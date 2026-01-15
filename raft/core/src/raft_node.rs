// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::{
    event::Event,
    log_entry::LogEntry,
    log_entry_collection::LogEntryCollection,
    map_collection::MapCollection,
    node_collection::NodeCollection,
    node_state::NodeState,
    raft_messages::RaftMsg,
    state_machine::StateMachine,
    storage::Storage,
    timer_service::{TimerKind, TimerService},
    transport::Transport,
    types::{LogIndex, NodeId, Term},
};

pub struct RaftNode<T, S, P, SM, C, L, M, TS>
where
    T: Transport<Payload = P, LogEntries = L>,
    S: Storage<Payload = P>,
    SM: StateMachine<Payload = P>,
    C: NodeCollection,
    L: LogEntryCollection<Payload = P>,
    M: MapCollection,
    TS: TimerService,
{
    id: NodeId,
    peers: Option<C>,
    role: NodeState,
    current_term: Term,
    commit_index: LogIndex,
    last_applied: LogIndex,
    votes_received: C,
    transport: Option<T>,
    storage: S,
    state_machine: SM,
    match_index: M,
    next_index: M,
    timer_service: TS,
}

impl<T, S, P, SM, C, L, M, TS> RaftNode<T, S, P, SM, C, L, M, TS>
where
    T: Transport<Payload = P, LogEntries = L>,
    S: Storage<Payload = P, LogEntryCollection = L>,
    SM: StateMachine<Payload = P>,
    C: NodeCollection,
    L: LogEntryCollection<Payload = P>,
    M: MapCollection,
    TS: TimerService,
{
    pub fn new(id: NodeId, storage: S, state_machine: SM, timer_service: TS) -> Self {
        RaftNode {
            id,
            peers: None,
            role: NodeState::Follower,
            current_term: storage.current_term(),
            commit_index: 0,
            last_applied: 0,
            votes_received: C::new(),
            transport: None,
            storage,
            state_machine,
            match_index: M::new(),
            next_index: M::new(),
            timer_service,
        }
    }

    pub fn role(&self) -> &NodeState {
        &self.role
    }

    pub fn storage(&self) -> &S {
        &self.storage
    }

    pub fn current_term(&self) -> Term {
        self.current_term
    }

    pub fn id(&self) -> NodeId {
        self.id
    }

    pub fn commit_index(&self) -> LogIndex {
        self.commit_index
    }

    pub fn peers(&self) -> Option<&C> {
        self.peers.as_ref()
    }

    pub fn state_machine(&self) -> &SM {
        &self.state_machine
    }

    pub fn timer_service(&self) -> &TS {
        &self.timer_service
    }

    pub fn set_peers(&mut self, peers: C) {
        self.peers = Some(peers);
    }

    pub fn set_transport(&mut self, transport: T) {
        self.transport = Some(transport);
    }

    pub fn on_event(&mut self, event: Event<P, L>) {
        match event {
            Event::TimerFired(TimerKind::Election) => {
                if self.role != NodeState::Leader {
                    self.current_term += 1;
                    self.storage.set_current_term(self.current_term);
                    self.storage.set_voted_for(Some(self.id));
                    self.votes_received.clear();
                    self.votes_received.push(self.id).unwrap(); // vote for self
                    self.role = NodeState::Candidate;

                    self.timer_service.reset_election_timer();

                    for &peer in self.peers.as_ref().unwrap().iter() {
                        self.transport.as_mut().unwrap().send(
                            peer,
                            RaftMsg::RequestVote {
                                term: self.current_term,
                                candidate_id: self.id,
                                last_log_index: self.storage.last_log_index(),
                                last_log_term: self.storage.last_log_term(),
                            },
                        );
                    }
                }
                if self.role == NodeState::Leader {
                    // Initialize next_index for all followers to last_log_index + 1
                    let next_log_index = self.storage.last_log_index() + 1;
                    if let Some(peers) = &self.peers {
                        for peer in peers.iter() {
                            self.next_index.insert(*peer, next_log_index);
                            self.match_index.insert(*peer, 0);
                        }
                    }

                    self.send_append_entries_to_followers();
                    self.timer_service.reset_heartbeat_timer();
                }
            }
            Event::TimerFired(TimerKind::Heartbeat) => {
                if self.role == NodeState::Leader {
                    // This serves as both heartbeat AND log replication
                    self.send_append_entries_to_followers();
                    self.timer_service.reset_heartbeat_timer();
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
                        let vote_granted = if term < self.current_term
                            || self
                                .storage
                                .voted_for()
                                .is_some_and(|voted| voted != candidate_id)
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

                        self.timer_service.reset_election_timer();
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

                                // Initialize next_index and match_index for all followers
                                let next_log_index = self.storage.last_log_index() + 1;
                                if let Some(peers) = &self.peers {
                                    for peer in peers.iter() {
                                        self.next_index.insert(*peer, next_log_index);
                                        self.match_index.insert(*peer, 0);
                                    }
                                }

                                self.timer_service.stop_timers();
                                self.timer_service.reset_heartbeat_timer();
                            }
                        }
                    }
                    RaftMsg::AppendEntries {
                        term,
                        prev_log_index,
                        prev_log_term,
                        entries,
                        leader_commit,
                    } => {
                        // Update term if necessary
                        if term > self.current_term {
                            self.current_term = term;
                            self.storage.set_current_term(term);
                            self.role = NodeState::Follower;
                            self.storage.set_voted_for(None);
                            self.timer_service.reset_election_timer();
                        }

                        let success = if term < self.current_term {
                            false
                        } else {
                            // Check log consistency
                            let log_ok = self.check_log_consistency(prev_log_index, prev_log_term);

                            if log_ok {
                                let last_index = self.storage.last_log_index();

                                if !entries.is_empty() {
                                    // If we have entries beyond prev_log_index, they might conflict
                                    if last_index > prev_log_index {
                                        // Truncate conflicting entries
                                        self.storage.truncate_after(prev_log_index);
                                    }

                                    // Append new entries
                                    self.storage.append_entries(entries.as_slice());
                                }

                                // Update commit index
                                if leader_commit > self.commit_index {
                                    self.commit_index =
                                        leader_commit.min(self.storage.last_log_index());
                                    self.apply_committed_entries();
                                }

                                true
                            } else {
                                false
                            }
                        };

                        // Send response
                        if let Some(transport) = self.transport.as_mut() {
                            transport.send(
                                from,
                                RaftMsg::AppendEntriesResponse {
                                    term: self.current_term,
                                    success,
                                    match_index: self.storage.last_log_index(),
                                },
                            );
                        }
                    }
                    RaftMsg::AppendEntriesResponse {
                        term,
                        success,
                        match_index,
                    } => {
                        // Step down if higher term
                        if term > self.current_term {
                            self.current_term = term;
                            self.storage.set_current_term(term);
                            self.role = NodeState::Follower;
                            self.storage.set_voted_for(None);
                            return;
                        }

                        if self.role == NodeState::Leader && term == self.current_term {
                            if success {
                                // Update match_index and next_index
                                self.match_index.insert(from, match_index);
                                self.next_index.insert(from, match_index + 1);
                                self.advance_commit_index();
                            } else {
                                // Decrement next_index on failure
                                let next = self.next_index.get(from).unwrap_or(1);
                                if next > 1 {
                                    self.next_index.insert(from, next - 1);
                                }
                            }
                        }
                    }
                }
            }
            Event::ClientCommand(payload) => {
                if self.role == NodeState::Leader {
                    // Append to leader's log
                    let entry = LogEntry {
                        term: self.current_term,
                        payload,
                    };
                    self.storage.append_entries(&[entry]);

                    self.send_append_entries_to_followers();
                }
            }
        }
    }

    // ...existing code...

    fn send_append_entries_to_followers(&mut self) {
        for peer in self.peers.as_ref().unwrap().iter() {
            if let Some(transport) = self.transport.as_mut() {
                // Get next_index (should already be initialized when node became leader)
                let next_idx = self.next_index.get(*peer).unwrap_or(1);

                let prev_log_index = next_idx.saturating_sub(1);
                let prev_log_term = if prev_log_index == 0 {
                    0
                } else {
                    self.storage
                        .get_entry(prev_log_index)
                        .map(|e| e.term)
                        .unwrap_or(0)
                };

                let entries = self
                    .storage
                    .get_entries(next_idx, self.storage.last_log_index() + 1);

                transport.send(
                    *peer,
                    RaftMsg::AppendEntries {
                        term: self.current_term,
                        prev_log_index,
                        prev_log_term,
                        entries,
                        leader_commit: self.commit_index,
                    },
                );
            }
        }
    }

    // ...existing code...

    fn check_log_consistency(&self, prev_log_index: LogIndex, prev_log_term: Term) -> bool {
        if prev_log_index == 0 {
            true // Empty log is always consistent
        } else {
            self.storage
                .get_entry(prev_log_index)
                .map(|entry| entry.term)
                == Some(prev_log_term)
        }
    }

    fn apply_committed_entries(&mut self) {
        while self.last_applied < self.commit_index {
            self.last_applied += 1;
            if let Some(entry) = self.storage.get_entry(self.last_applied) {
                self.state_machine.apply(&entry.payload);
            }
        }
    }

    fn advance_commit_index(&mut self) {
        let leader_index = self.storage.last_log_index();
        let total_peers = self.peers.as_ref().map(|p| p.len()).unwrap_or(0);

        if let Some(new_commit) = self.match_index.compute_median(leader_index, total_peers) {
            if new_commit > self.commit_index {
                if let Some(entry) = self.storage.get_entry(new_commit) {
                    if entry.term == self.current_term {
                        self.commit_index = new_commit;
                        self.apply_committed_entries();
                    }
                }
            }
        }
    }
}
