// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::{
    election_manager::ElectionManager,
    event::Event,
    log_entry::LogEntry,
    log_entry_collection::LogEntryCollection,
    log_replication_manager::LogReplicationManager,
    map_collection::MapCollection,
    node_collection::NodeCollection,
    node_state::NodeState,
    observer::{Observer, Role, TimerKind as ObserverTimerKind},
    raft_messages::RaftMsg,
    state_machine::StateMachine,
    storage::Storage,
    timer_service::{TimerKind, TimerService},
    transport::Transport,
    types::{LogIndex, NodeId, Term},
};

pub struct RaftNode<T, S, P, SM, C, L, M, TS, O>
where
    P: Clone,
    T: Transport<Payload = P, LogEntries = L>,
    S: Storage<Payload = P>,
    SM: StateMachine<Payload = P>,
    C: NodeCollection,
    L: LogEntryCollection<Payload = P>,
    M: MapCollection,
    TS: TimerService,
    O: Observer<Payload = P, LogEntries = L>,
{
    id: NodeId,
    peers: C,
    role: NodeState,
    current_term: Term,
    transport: T,
    storage: S,
    state_machine: SM,
    observer: O,

    // Delegated responsibilities
    election: ElectionManager<C, TS>,
    replication: LogReplicationManager<M>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientError {
    NotLeader,
}

impl<T, S, P, SM, C, L, M, TS, O> RaftNode<T, S, P, SM, C, L, M, TS, O>
where
    P: Clone,
    T: Transport<Payload = P, LogEntries = L>,
    S: Storage<Payload = P, LogEntryCollection = L> + Clone,
    SM: StateMachine<Payload = P>,
    C: NodeCollection,
    L: LogEntryCollection<Payload = P> + Clone,
    M: MapCollection,
    TS: TimerService,
    O: Observer<Payload = P, LogEntries = L>,
{
    /// Internal constructor - use RaftNodeBuilder instead
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new_from_builder(
        id: NodeId,
        storage: S,
        state_machine: SM,
        mut election: ElectionManager<C, TS>,
        replication: LogReplicationManager<M>,
        transport: T,
        peers: C,
        observer: O,
    ) -> Self {
        let current_term = storage.current_term();

        // Start election timer for initial Follower state
        election.timer_service_mut().reset_election_timer();

        RaftNode {
            id,
            peers,
            role: NodeState::Follower,
            current_term,
            transport,
            storage,
            state_machine,
            observer,
            election,
            replication,
        }
    }

    // ============================================================
    // PUBLIC GETTERS
    // ============================================================

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
        self.replication.commit_index()
    }

    pub fn peers(&self) -> Option<&C> {
        if self.peers.is_empty() {
            None
        } else {
            Some(&self.peers)
        }
    }

    pub fn state_machine(&self) -> &SM {
        &self.state_machine
    }

    pub fn timer_service(&self) -> &TS {
        self.election.timer_service()
    }

    // ============================================================
    // EVENT HANDLING
    // ============================================================

    pub fn on_event(&mut self, event: Event<P, L>) {
        match event {
            Event::TimerFired(kind) => self.handle_timer(kind),
            Event::Message { from, msg } => self.handle_message(from, msg),
            Event::ClientCommand(payload) => {
                let _ = self.submit_client_command(payload);
            }
        }
    }

    fn handle_timer(&mut self, kind: TimerKind) {
        let observer_kind = match kind {
            TimerKind::Election => ObserverTimerKind::Election,
            TimerKind::Heartbeat => ObserverTimerKind::Heartbeat,
        };
        self.observer
            .timer_fired(self.id, observer_kind, self.current_term);

        match kind {
            TimerKind::Election => {
                if self.role != NodeState::Leader {
                    self.observer.election_timeout(self.id, self.current_term);
                    self.start_election();
                }
            }
            TimerKind::Heartbeat => {
                if self.role == NodeState::Leader {
                    self.send_heartbeats();
                }
            }
        }
    }

    fn handle_message(&mut self, from: NodeId, msg: RaftMsg<P, L>) {
        match msg {
            RaftMsg::RequestVote {
                term,
                candidate_id,
                last_log_index,
                last_log_term,
            } => {
                let response = self.election.handle_vote_request(
                    term,
                    candidate_id,
                    last_log_index,
                    last_log_term,
                    &mut self.current_term,
                    &mut self.storage,
                    &mut self.role,
                );
                self.send(from, response);
            }

            RaftMsg::RequestVoteResponse { term, vote_granted } => {
                if term > self.current_term {
                    self.step_down(term);
                    return;
                }

                let total_peers = self.peers.len();
                let should_become_leader = self.election.handle_vote_response(
                    from,
                    term,
                    vote_granted,
                    &self.current_term,
                    &self.role,
                    total_peers,
                );

                if should_become_leader {
                    self.become_leader();
                }
            }

            RaftMsg::AppendEntries {
                term,
                prev_log_index,
                prev_log_term,
                entries,
                leader_commit,
            } => {
                // Reset election timer on valid heartbeat
                if term >= self.current_term {
                    self.election.timer_service_mut().reset_election_timer();
                }

                let response = self.replication.handle_append_entries(
                    term,
                    prev_log_index,
                    prev_log_term,
                    entries,
                    leader_commit,
                    &mut self.current_term,
                    &mut self.storage,
                    &mut self.state_machine,
                    &mut self.role,
                );
                self.send(from, response);
            }

            RaftMsg::AppendEntriesResponse {
                term,
                success,
                match_index,
            } => {
                if term > self.current_term {
                    self.step_down(term);
                    return;
                }

                if self.role == NodeState::Leader && term == self.current_term {
                    let old_commit_index = self.replication.commit_index();
                    self.replication.handle_append_entries_response(
                        from,
                        success,
                        match_index,
                        &self.storage,
                        &mut self.state_machine,
                    );
                    let new_commit_index = self.replication.commit_index();
                    if new_commit_index > old_commit_index {
                        self.observer
                            .commit_advanced(self.id, old_commit_index, new_commit_index);
                    }
                }
            }
        }
    }

    pub fn submit_client_command(&mut self, payload: P) -> Result<LogIndex, ClientError> {
        if self.role != NodeState::Leader {
            return Err(ClientError::NotLeader);
        }

        let entry = LogEntry {
            term: self.current_term,
            payload,
        };
        self.storage.append_entries(&[entry]);
        let index = self.storage.last_log_index();

        self.send_append_entries_to_followers();

        Ok(index)
    }

    pub fn is_committed(&self, index: LogIndex) -> bool {
        index <= self.replication.commit_index()
    }

    // ============================================================
    // STATE TRANSITIONS
    // ============================================================

    fn start_election(&mut self) {
        let old_role = self.node_state_to_role();

        let vote_request = self.election.start_election(
            self.id,
            &mut self.current_term,
            &mut self.storage,
            &mut self.role,
        );

        self.observer.election_started(self.id, self.current_term);
        self.observer
            .role_changed(self.id, old_role, Role::Candidate, self.current_term);
        self.observer.voted_for(self.id, self.id, self.current_term);

        self.broadcast(vote_request);
    }

    fn become_leader(&mut self) {
        let old_role = self.node_state_to_role();
        self.role = NodeState::Leader;

        self.observer
            .role_changed(self.id, old_role, Role::Leader, self.current_term);
        self.observer.leader_elected(self.id, self.current_term);

        // Initialize replication state
        self.replication
            .initialize_leader_state(self.peers.iter(), &self.storage);

        self.election.timer_service_mut().stop_timers();
        self.election.timer_service_mut().reset_heartbeat_timer();

        // Send initial heartbeat
        self.send_append_entries_to_followers();
    }

    fn step_down(&mut self, new_term: Term) {
        let old_role = self.node_state_to_role();
        let old_term = self.current_term;

        if old_role == Role::Leader {
            self.observer.leader_lost(self.id, old_term, new_term);
        }

        self.current_term = new_term;
        self.storage.set_current_term(new_term);
        self.role = NodeState::Follower;
        self.storage.set_voted_for(None);
        self.election.timer_service_mut().reset_election_timer();

        self.observer
            .role_changed(self.id, old_role, Role::Follower, new_term);
    }

    // ============================================================
    // MESSAGE SENDING
    // ============================================================

    fn send(&mut self, to: NodeId, msg: RaftMsg<P, L>) {
        self.transport.send(to, msg);
    }

    fn broadcast(&mut self, msg: RaftMsg<P, L>) {
        // Collect peer IDs first to avoid borrowing issues
        let mut ids = C::new();
        for peer in self.peers.iter() {
            ids.push(peer).ok();
        }

        // Now send to each peer
        for peer in ids.iter() {
            self.send(peer, msg.clone());
        }
    }

    fn send_heartbeats(&mut self) {
        self.send_append_entries_to_followers();
        self.election.timer_service_mut().reset_heartbeat_timer();
    }

    fn send_append_entries_to_followers(&mut self) {
        // Collect peer IDs first to avoid borrowing issues
        let mut ids = C::new();
        for peer in self.peers.iter() {
            ids.push(peer).ok();
        }

        // Now send to each peer
        for peer in ids.iter() {
            let msg = self.replication.get_append_entries_for_follower(
                peer,
                self.current_term,
                &self.storage,
            );
            self.send(peer, msg);
        }
    }

    // ============================================================
    // OBSERVER ACCESS
    // ============================================================

    pub fn observer(&mut self) -> &mut O {
        &mut self.observer
    }

    /// Convert NodeState to Observer Role
    fn node_state_to_role(&self) -> Role {
        match self.role {
            NodeState::Follower => Role::Follower,
            NodeState::Candidate => Role::Candidate,
            NodeState::Leader => Role::Leader,
        }
    }
}
