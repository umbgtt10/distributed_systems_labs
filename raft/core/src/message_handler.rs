// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::{
    chunk_collection::ChunkCollection,
    config_change_collection::ConfigChangeCollection,
    config_change_manager::ConfigChangeManager,
    election_manager::ElectionManager,
    log_entry_collection::LogEntryCollection,
    log_replication_manager::LogReplicationManager,
    map_collection::MapCollection,
    node_collection::NodeCollection,
    node_state::NodeState,
    observer::{Observer, Role},
    raft_messages::RaftMsg,
    role_transition_manager::RoleTransitionManager,
    snapshot_manager::SnapshotManager,
    state_machine::StateMachine,
    storage::Storage,
    timer_service::TimerService,
    transport::Transport,
    types::{LogIndex, NodeId, Term},
};

/// MessageHandler handles all Raft message processing with mutable access to node state.
/// This separates message handling logic from the RaftNode facade.
pub struct MessageHandler<'a, T, S, P, SM, C, L, CC, M, TS, O, CCC>
where
    P: Clone,
    T: Transport<Payload = P, LogEntries = L, ChunkCollection = CC>,
    S: Storage<Payload = P>,
    SM: StateMachine<Payload = P>,
    C: NodeCollection,
    L: LogEntryCollection<Payload = P>,
    CC: ChunkCollection + Clone,
    M: MapCollection,
    TS: TimerService,
    O: Observer<Payload = P, LogEntries = L, ChunkCollection = CC>,
    CCC: ConfigChangeCollection,
{
    pub id: &'a NodeId,
    pub role: &'a mut NodeState,
    pub current_term: &'a mut Term,
    pub transport: &'a mut T,
    pub storage: &'a mut S,
    pub state_machine: &'a mut SM,
    pub observer: &'a mut O,
    pub election: &'a mut ElectionManager<C, TS>,
    pub replication: &'a mut LogReplicationManager<M>,
    pub config_manager: &'a mut ConfigChangeManager<C, M>,
    pub snapshot_manager: &'a mut SnapshotManager,
    pub _phantom: core::marker::PhantomData<CCC>,
}

impl<'a, T, S, P, SM, C, L, CC, M, TS, O, CCC>
    MessageHandler<'a, T, S, P, SM, C, L, CC, M, TS, O, CCC>
where
    P: Clone,
    T: Transport<Payload = P, LogEntries = L, ChunkCollection = CC>,
    S: Storage<Payload = P, LogEntryCollection = L, SnapshotChunk = CC> + Clone,
    SM: StateMachine<Payload = P>,
    C: NodeCollection,
    L: LogEntryCollection<Payload = P> + Clone,
    CC: ChunkCollection + Clone,
    M: MapCollection,
    TS: TimerService,
    O: Observer<Payload = P, LogEntries = L, ChunkCollection = CC>,
    CCC: ConfigChangeCollection,
{
    /// Main entry point for handling messages
    pub fn handle_message(&mut self, from: NodeId, msg: RaftMsg<P, L, CC>)
    where
        SM: StateMachine<Payload = P, SnapshotData = S::SnapshotData>,
        S: Storage<Payload = P, LogEntryCollection = L>,
    {
        match msg {
            RaftMsg::PreVoteRequest {
                term,
                candidate_id,
                last_log_index,
                last_log_term,
            } => self.handle_pre_vote_request(
                from,
                term,
                candidate_id,
                last_log_index,
                last_log_term,
            ),

            RaftMsg::PreVoteResponse { term, vote_granted } => {
                self.handle_pre_vote_response(from, term, vote_granted)
            }

            RaftMsg::RequestVote {
                term,
                candidate_id,
                last_log_index,
                last_log_term,
            } => self.handle_vote_request(from, term, candidate_id, last_log_index, last_log_term),

            RaftMsg::RequestVoteResponse { term, vote_granted } => {
                self.handle_vote_response(from, term, vote_granted)
            }

            RaftMsg::AppendEntries {
                term,
                prev_log_index,
                prev_log_term,
                entries,
                leader_commit,
            } => self.handle_append_entries(
                from,
                term,
                prev_log_index,
                prev_log_term,
                entries,
                leader_commit,
            ),

            RaftMsg::AppendEntriesResponse {
                term,
                success,
                match_index,
            } => self.handle_append_entries_response(from, term, success, match_index),

            RaftMsg::InstallSnapshot {
                term,
                leader_id,
                last_included_index,
                last_included_term,
                offset,
                data,
                done,
            } => self.handle_install_snapshot(
                from,
                term,
                leader_id,
                last_included_index,
                last_included_term,
                offset,
                data,
                done,
            ),

            RaftMsg::InstallSnapshotResponse { term, success } => {
                self.handle_install_snapshot_response(from, term, success)
            }
        }
    }

    // Handler methods
    fn handle_pre_vote_request(
        &mut self,
        from: NodeId,
        term: Term,
        candidate_id: NodeId,
        last_log_index: LogIndex,
        last_log_term: Term,
    ) {
        self.observer.pre_vote_requested(
            candidate_id,
            *self.id,
            term,
            last_log_index,
            last_log_term,
        );

        let response = self.election.handle_pre_vote_request(
            term,
            candidate_id,
            last_log_index,
            last_log_term,
            *self.current_term,
            self.storage,
        );

        // Log the response
        if let RaftMsg::PreVoteResponse { vote_granted, .. } = &response {
            self.observer
                .pre_vote_granted(candidate_id, *self.id, *vote_granted, term);
        }

        self.send(from, response);
    }

    fn handle_pre_vote_response(&mut self, from: NodeId, term: Term, vote_granted: bool) {
        // Ignore pre-vote responses from higher terms
        if term > *self.current_term {
            return;
        }

        let should_start_election = self.election.handle_pre_vote_response(
            from,
            vote_granted,
            self.config_manager.config(),
        );

        if should_start_election {
            self.observer
                .pre_vote_succeeded(*self.id, *self.current_term);
            self.start_election();
        }
    }

    fn handle_vote_request(
        &mut self,
        from: NodeId,
        term: Term,
        candidate_id: NodeId,
        last_log_index: LogIndex,
        last_log_term: Term,
    ) {
        let response = self.election.handle_vote_request(
            term,
            candidate_id,
            last_log_index,
            last_log_term,
            self.current_term,
            self.storage,
            self.role,
        );
        self.send(from, response);
    }

    fn handle_vote_response(&mut self, from: NodeId, term: Term, vote_granted: bool) {
        if term > *self.current_term {
            self.step_down(term);
            return;
        }

        let should_become_leader = self.election.handle_vote_response(
            from,
            term,
            vote_granted,
            self.current_term,
            self.role,
            self.config_manager.config(),
        );

        if should_become_leader {
            self.become_leader();
        }
    }

    fn handle_append_entries(
        &mut self,
        from: NodeId,
        term: Term,
        prev_log_index: LogIndex,
        prev_log_term: Term,
        entries: L,
        leader_commit: LogIndex,
    ) where
        SM: StateMachine<Payload = P, SnapshotData = S::SnapshotData>,
        S: Storage<Payload = P, LogEntryCollection = L>,
    {
        // Reset election timer on valid heartbeat
        if term >= *self.current_term {
            self.election.timer_service_mut().reset_election_timer();
        }

        let (response, config_changes) = self.replication.handle_append_entries(
            term,
            prev_log_index,
            prev_log_term,
            entries,
            leader_commit,
            self.current_term,
            self.storage,
            self.state_machine,
            self.role,
        );
        self.apply_config_changes(config_changes);
        self.send(from, response);
    }

    fn handle_append_entries_response(
        &mut self,
        from: NodeId,
        term: Term,
        success: bool,
        match_index: LogIndex,
    ) where
        SM: StateMachine<Payload = P, SnapshotData = S::SnapshotData>,
    {
        if term > *self.current_term {
            self.step_down(term);
            return;
        }

        if *self.role == NodeState::Leader && term == *self.current_term {
            let old_commit_index = self.replication.commit_index();
            let config_changes: CCC = self.replication.handle_append_entries_response(
                from,
                success,
                match_index,
                self.storage,
                self.state_machine,
                self.config_manager.config(),
            );
            let new_commit_index = self.replication.commit_index();

            if new_commit_index > old_commit_index {
                self.observer
                    .commit_advanced(*self.id, old_commit_index, new_commit_index);

                // Apply any configuration changes
                self.apply_config_changes(config_changes);

                // Check if we should create a snapshot after commit advances
                if self.should_create_snapshot() {
                    let _ = self.create_snapshot_internal();
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn handle_install_snapshot(
        &mut self,
        from: NodeId,
        term: Term,
        leader_id: NodeId,
        last_included_index: LogIndex,
        last_included_term: Term,
        offset: u64,
        data: CC,
        done: bool,
    ) where
        SM: StateMachine<Payload = P, SnapshotData = S::SnapshotData>,
    {
        // Reset election timer on valid snapshot
        if term >= *self.current_term {
            self.election.timer_service_mut().reset_election_timer();
        }

        let response = self.replication.handle_install_snapshot(
            term,
            leader_id,
            last_included_index,
            last_included_term,
            offset,
            data,
            done,
            self.current_term,
            self.storage,
            self.state_machine,
            self.role,
        );
        self.send(from, response);
    }

    fn handle_install_snapshot_response(&mut self, from: NodeId, term: Term, success: bool) {
        if term > *self.current_term {
            self.step_down(term);
            return;
        }

        if *self.role == NodeState::Leader && term == *self.current_term {
            // Get the snapshot metadata to know last_included_index
            if let Some(snapshot_metadata) = self.storage.snapshot_metadata() {
                self.replication.handle_install_snapshot_response(
                    from,
                    term,
                    success,
                    snapshot_metadata.last_included_index,
                );
            }
        }
    }

    // Support methods
    pub fn send(&mut self, to: NodeId, msg: RaftMsg<P, L, CC>) {
        self.transport.send(to, msg);
    }

    pub fn broadcast(&mut self, msg: RaftMsg<P, L, CC>) {
        // Collect peer IDs first to avoid borrowing issues
        let mut ids = C::new();
        for peer in self.config_manager.config().members.iter() {
            ids.push(peer).ok();
        }

        // Now send to each peer
        for peer in ids.iter() {
            self.send(peer, msg.clone());
        }
    }

    pub fn apply_config_changes(&mut self, changes: CCC)
    where
        C: NodeCollection,
        M: MapCollection,
        CCC: ConfigChangeCollection,
    {
        // Check if we need to notify observer about role change (if we removed ourselves)
        let old_role = self.node_state_to_role();
        let last_log_index = self.storage.last_log_index();

        self.config_manager.apply_changes(
            changes,
            *self.id,
            last_log_index,
            self.replication,
            self.observer,
            self.role,
        );

        // If we stepped down due to self-removal, notify observer
        let new_role = self.node_state_to_role();
        if old_role != new_role {
            self.observer
                .role_changed(*self.id, old_role, new_role, *self.current_term);
        }
    }

    pub fn start_pre_vote(&mut self) {
        let pre_vote_request = RoleTransitionManager::start_pre_vote(
            *self.id,
            *self.current_term,
            self.storage,
            self.election,
            self.observer,
        );

        self.broadcast(pre_vote_request);
    }

    pub fn start_election(&mut self) {
        let old_role = RoleTransitionManager::node_state_to_role(self.role);

        let vote_request = RoleTransitionManager::start_election(
            *self.id,
            self.current_term,
            self.storage,
            self.role,
            self.election,
            self.observer,
            old_role,
        );

        self.broadcast(vote_request);

        // If we have no peers, we already have majority (1 of 1) - become leader immediately
        if self.config_manager.config().members.len() == 0 {
            self.become_leader();
        }
    }

    fn become_leader(&mut self) {
        let old_role = RoleTransitionManager::node_state_to_role(self.role);

        RoleTransitionManager::become_leader(
            *self.id,
            *self.current_term,
            self.role,
            self.storage,
            self.config_manager.config().members.iter(),
            self.election,
            self.replication,
            self.observer,
            old_role,
        );

        // Send initial heartbeat
        self.send_append_entries_to_followers();
    }

    fn step_down(&mut self, new_term: Term) {
        let old_role = RoleTransitionManager::node_state_to_role(self.role);
        let old_term = *self.current_term;

        RoleTransitionManager::step_down(
            *self.id,
            old_term,
            new_term,
            self.current_term,
            self.storage,
            self.role,
            self.election,
            self.observer,
            old_role,
        );
    }

    pub fn send_heartbeats(&mut self) {
        self.send_append_entries_to_followers();
        self.election.timer_service_mut().reset_heartbeat_timer();
    }

    pub fn send_append_entries_to_followers(&mut self) {
        // Collect peer IDs first to avoid borrowing issues
        let mut ids = C::new();
        for peer in self.config_manager.config().members.iter() {
            ids.push(peer).ok();
        }

        // Now send to each peer
        for peer in ids.iter() {
            let msg = self
                .replication
                .get_append_entries_for_peer(peer, *self.id, self.storage);
            self.send(peer, msg);
        }
    }

    fn should_create_snapshot(&self) -> bool {
        self.snapshot_manager
            .should_create(self.replication.commit_index(), self.storage)
    }

    fn create_snapshot_internal(&mut self) -> Result<(), ()>
    where
        SM: StateMachine<Payload = P, SnapshotData = S::SnapshotData>,
    {
        let commit_index = self.replication.commit_index();
        self.snapshot_manager
            .create(self.storage, self.state_machine, commit_index)
            .map(|_| ())
            .map_err(|_| ())
    }

    /// Convert NodeState to Observer Role
    fn node_state_to_role(&self) -> Role {
        RoleTransitionManager::node_state_to_role(self.role)
    }
}
