// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::{
    event::Event,
    node_collection::NodeCollection,
    node_state::NodeState,
    raft_messages::RaftMsg,
    state_machine::StateMachine,
    storage::Storage,
    timer::TimerKind,
    transport::Transport,
    types::{LogIndex, NodeId, Term},
};

pub struct RaftNode<T, S, P, SM, C>
where
    T: Transport<Payload = P>,
    S: Storage<Payload = P>,
    SM: StateMachine<Payload = P>,
    C: NodeCollection,
{
    id: NodeId,
    peers: Option<C>,
    role: NodeState,
    current_term: Term,
    commit_index: LogIndex,
    votes_received: C,
    transport: Option<T>,
    storage: S,
    state_machine: SM,
}

impl<T, S, P, SM, C> RaftNode<T, S, P, SM, C>
where
    T: Transport<Payload = P>,
    S: Storage<Payload = P>,
    SM: StateMachine<Payload = P>,
    C: NodeCollection,
{
    pub fn new(id: NodeId, storage: S, state_machine: SM) -> Self {
        RaftNode {
            id,
            peers: None,
            role: NodeState::Follower,
            current_term: storage.current_term(),
            commit_index: 0,
            votes_received: C::new(),
            transport: None,
            storage,
            state_machine,
        }
    }

    pub fn role(&self) -> &NodeState {
        &self.role
    }

    pub fn current_term(&self) -> Term {
        self.current_term
    }

    pub fn id(&self) -> NodeId {
        self.id
    }

    pub fn peers(&self) -> Option<&C> {
        self.peers.as_ref()
    }

    pub fn set_peers(&mut self, peers: C) {
        self.peers = Some(peers);
    }

    pub fn set_transport(&mut self, transport: T) {
        self.transport = Some(transport);
    }

    pub fn on_event(&mut self, event: Event<P>) {
        match event {
            Event::TimerFired(TimerKind::Election) => {
                if self.role != NodeState::Leader {
                    self.current_term += 1;
                    self.storage.set_current_term(self.current_term);
                    self.storage.set_voted_for(Some(self.id));
                    self.votes_received.clear();
                    self.votes_received.push(self.id).unwrap(); // vote for self
                    self.role = NodeState::Candidate;
                    // reset election timer
                    // self.reset_timer(TimerKind::Election);
                    // send request vote to all peers
                    let last_log_index = self.storage.last_log_index();
                    let last_log_term = self.storage.last_log_term();
                    for &peer in self.peers.as_ref().unwrap().iter() {
                        self.transport.as_mut().unwrap().send(
                            peer,
                            RaftMsg::RequestVote {
                                term: self.current_term,
                                candidate_id: self.id,
                                last_log_index,
                                last_log_term,
                            },
                        );
                    }
                }
            }
            Event::TimerFired(TimerKind::Heartbeat) => {
                if self.role == NodeState::Leader {
                    // send heartbeats
                    let last_log_index = self.storage.last_log_index();
                    let last_log_term = self.storage.last_log_term();
                    for &peer in self.peers.as_ref().unwrap().iter() {
                        self.transport.as_mut().unwrap().send(
                            peer,
                            RaftMsg::AppendEntries {
                                term: self.current_term,
                                leader_id: self.id,
                                prev_log_index: last_log_index,
                                prev_log_term: last_log_term,
                                entries: heapless::Vec::new(), // empty for heartbeat
                                leader_commit: self.commit_index,
                            },
                        );
                    }
                    // reset heartbeat timer
                }
            }
            Event::Message { from, msg } => {
                match msg {
                    RaftMsg::RequestVote {
                        term,
                        candidate_id,
                        last_log_index,
                        last_log_term,
                    } => {
                        if term > self.current_term {
                            self.current_term = term;
                            self.storage.set_current_term(term);
                            self.role = NodeState::Follower;
                            self.storage.set_voted_for(None);
                        }
                        let vote_granted = if term < self.current_term {
                            false
                        } else if self.storage.voted_for().is_some()
                            && self.storage.voted_for() != Some(candidate_id)
                        {
                            false
                        } else {
                            // Check log is at least as up-to-date
                            let our_last_log_term = self.storage.last_log_term();
                            let our_last_log_index = self.storage.last_log_index();

                            let log_ok = last_log_term > our_last_log_term
                                || (last_log_term == our_last_log_term
                                    && last_log_index >= our_last_log_index);

                            if log_ok {
                                self.storage.set_voted_for(Some(candidate_id));
                                true
                            } else {
                                false
                            }
                        };

                        self.transport.as_mut().unwrap().send(
                            from,
                            RaftMsg::RequestVoteResponse {
                                term: self.current_term,
                                vote_granted,
                            },
                        );
                        // reset election timer
                    }
                    RaftMsg::RequestVoteResponse { term, vote_granted } => {
                        if term > self.current_term {
                            self.current_term = term;
                            self.storage.set_current_term(term);
                            self.role = NodeState::Follower;
                            return;
                        }

                        if self.role == NodeState::Candidate
                            && term == self.current_term
                            && vote_granted
                        {
                            self.votes_received.push(from).ok();

                            // Check if we have majority
                            let total_nodes = self.peers.as_ref().map(|p| p.len()).unwrap_or(0) + 1;
                            let votes = self.votes_received.len();

                            if votes > total_nodes / 2 {
                                self.role = NodeState::Leader;
                            }
                        }
                    }
                    RaftMsg::AppendEntries {
                        term,
                        leader_id,
                        prev_log_index,
                        prev_log_term,
                        entries,
                        leader_commit,
                    } => {
                        if term >= self.current_term {
                            self.current_term = term;
                            self.storage.set_current_term(term);
                            self.role = NodeState::Follower;
                            self.storage.set_voted_for(None);
                            // reset election timer
                            // handle log replication, but for election, just acknowledge
                            let success = true; // assume for now
                            self.transport.as_mut().unwrap().send(
                                from,
                                RaftMsg::AppendEntriesResponse {
                                    term: self.current_term,
                                    success,
                                    match_index: prev_log_index + entries.len() as LogIndex,
                                },
                            );
                        } else {
                            self.transport.as_mut().unwrap().send(
                                from,
                                RaftMsg::AppendEntriesResponse {
                                    term: self.current_term,
                                    success: false,
                                    match_index: 0,
                                },
                            );
                        }
                    }
                    RaftMsg::AppendEntriesResponse { .. } => {
                        // for leader, handle replication, but for election, perhaps later
                    }
                }
            }
            Event::ClientCommand(_) => {
                // handle client commands later
            }
        }
    }
}
