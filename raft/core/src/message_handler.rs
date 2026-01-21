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

type PhantomData<T, S, P, SM, C, L, CC, M, TS, O, CCC> =
    core::marker::PhantomData<(T, S, P, SM, C, L, CC, M, TS, O, CCC)>;

/// MessageHandler handles all Raft message processing.
/// This is a zero-sized stateless type that separates message handling logic from RaftNode.
pub struct MessageHandler<T, S, P, SM, C, L, CC, M, TS, O, CCC>
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
    _phantom: PhantomData<T, S, P, SM, C, L, CC, M, TS, O, CCC>,
}

/// Context struct that bundles all mutable references needed by MessageHandler
pub struct MessageHandlerContext<'a, T, S, P, SM, C, L, CC, M, TS, O, CCC>
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

impl<T, S, P, SM, C, L, CC, M, TS, O, CCC> MessageHandler<T, S, P, SM, C, L, CC, M, TS, O, CCC>
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
    pub fn new() -> Self {
        Self {
            _phantom: core::marker::PhantomData,
        }
    }

    /// Main entry point for handling messages
    pub fn handle_message(&self,
        ctx: &mut MessageHandlerContext<T, S, P, SM, C, L, CC, M, TS, O, CCC>,
        from: NodeId,
        msg: RaftMsg<P, L, CC>,
    ) where
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
                ctx,
                from,
                term,
                candidate_id,
                last_log_index,
                last_log_term,
            ),

            RaftMsg::PreVoteResponse { term, vote_granted } => {
                self.handle_pre_vote_response(ctx, from, term, vote_granted)
            }

            RaftMsg::RequestVote {
                term,
                candidate_id,
                last_log_index,
                last_log_term,
            } => self.handle_vote_request(
                ctx,
                from,
                term,
                candidate_id,
                last_log_index,
                last_log_term,
            ),

            RaftMsg::RequestVoteResponse { term, vote_granted } => {
                self.handle_vote_response(ctx, from, term, vote_granted)
            }

            RaftMsg::AppendEntries {
                term,
                prev_log_index,
                prev_log_term,
                entries,
                leader_commit,
            } => self.handle_append_entries(
                ctx,
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
            } => self.handle_append_entries_response(ctx, from, term, success, match_index),

            RaftMsg::InstallSnapshot {
                term,
                leader_id,
                last_included_index,
                last_included_term,
                offset,
                data,
                done,
            } => self.handle_install_snapshot(
                ctx,
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
                self.handle_install_snapshot_response(ctx, from, term, success)
            }
        }
    }

    // Handler methods
    fn handle_pre_vote_request(
        &self,
        ctx: &mut MessageHandlerContext<T, S, P, SM, C, L, CC, M, TS, O, CCC>,
        from: NodeId,
        term: Term,
        candidate_id: NodeId,
        last_log_index: LogIndex,
        last_log_term: Term,
    ) {
        ctx.observer
            .pre_vote_requested(candidate_id, *ctx.id, term, last_log_index, last_log_term);

        let response = ctx.election.handle_pre_vote_request(
            term,
            candidate_id,
            last_log_index,
            last_log_term,
            *ctx.current_term,
            ctx.storage,
        );

        // Log the response
        if let RaftMsg::PreVoteResponse { vote_granted, .. } = &response {
            ctx.observer
                .pre_vote_granted(candidate_id, *ctx.id, *vote_granted, term);
        }

        self.send(ctx, from, response);
    }

    fn handle_pre_vote_response(
        &self,
        ctx: &mut MessageHandlerContext<T, S, P, SM, C, L, CC, M, TS, O, CCC>,
        from: NodeId,
        term: Term,
        vote_granted: bool,
    ) {
        // Ignore pre-vote responses from higher terms
        if term > *ctx.current_term {
            return;
        }

        let should_start_election =
            ctx.election
                .handle_pre_vote_response(from, vote_granted, ctx.config_manager.config());

        if should_start_election {
            ctx.observer.pre_vote_succeeded(*ctx.id, *ctx.current_term);
            self.start_election(ctx);
        }
    }

    fn handle_vote_request(
        &self,
        ctx: &mut MessageHandlerContext<T, S, P, SM, C, L, CC, M, TS, O, CCC>,
        from: NodeId,
        term: Term,
        candidate_id: NodeId,
        last_log_index: LogIndex,
        last_log_term: Term,
    ) {
        let response = ctx.election.handle_vote_request(
            term,
            candidate_id,
            last_log_index,
            last_log_term,
            ctx.current_term,
            ctx.storage,
            ctx.role,
        );
        self.send(ctx, from, response);
    }

    fn handle_vote_response(
        &self,
        ctx: &mut MessageHandlerContext<T, S, P, SM, C, L, CC, M, TS, O, CCC>,
        from: NodeId,
        term: Term,
        vote_granted: bool,
    ) {
        if term > *ctx.current_term {
            self.step_down(ctx, term);
            return;
        }

        let should_become_leader = ctx.election.handle_vote_response(
            from,
            term,
            vote_granted,
            ctx.current_term,
            ctx.role,
            ctx.config_manager.config(),
        );

        if should_become_leader {
            self.become_leader(ctx);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn handle_append_entries(
        &self,
        ctx: &mut MessageHandlerContext<T, S, P, SM, C, L, CC, M, TS, O, CCC>,
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
        if term >= *ctx.current_term {
            ctx.election.timer_service_mut().reset_election_timer();
        }

        let (response, config_changes) = ctx.replication.handle_append_entries(
            term,
            prev_log_index,
            prev_log_term,
            entries,
            leader_commit,
            ctx.current_term,
            ctx.storage,
            ctx.state_machine,
            ctx.role,
        );
        self.apply_config_changes(ctx, config_changes);
        self.send(ctx, from, response);
    }

    fn handle_append_entries_response(
        & self,
        ctx: &mut MessageHandlerContext<T, S, P, SM, C, L, CC, M, TS, O, CCC>,
        from: NodeId,
        term: Term,
        success: bool,
        match_index: LogIndex,
    ) where
        SM: StateMachine<Payload = P, SnapshotData = S::SnapshotData>,
    {
        if term > *ctx.current_term {
            self.step_down(ctx, term);
            return;
        }

        if *ctx.role == NodeState::Leader && term == *ctx.current_term {
            let old_commit_index = ctx.replication.commit_index();
            let config_changes: CCC = ctx.replication.handle_append_entries_response(
                from,
                success,
                match_index,
                ctx.storage,
                ctx.state_machine,
                ctx.config_manager.config(),
            );
            let new_commit_index = ctx.replication.commit_index();

            if new_commit_index > old_commit_index {
                ctx.observer
                    .commit_advanced(*ctx.id, old_commit_index, new_commit_index);

                // Apply any configuration changes
                self.apply_config_changes(ctx, config_changes);

                // Check if we should create a snapshot after commit advances
                if self.should_create_snapshot(ctx) {
                    let _ = self.create_snapshot_internal(ctx);
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn handle_install_snapshot(
        &self,
        ctx: &mut MessageHandlerContext<T, S, P, SM, C, L, CC, M, TS, O, CCC>,
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
        if term >= *ctx.current_term {
            ctx.election.timer_service_mut().reset_election_timer();
        }

        let response = ctx.replication.handle_install_snapshot(
            term,
            leader_id,
            last_included_index,
            last_included_term,
            offset,
            data,
            done,
            ctx.current_term,
            ctx.storage,
            ctx.state_machine,
            ctx.role,
        );
        self.send(ctx, from, response);
    }

    fn handle_install_snapshot_response(
        &self,
        ctx: &mut MessageHandlerContext<T, S, P, SM, C, L, CC, M, TS, O, CCC>,
        from: NodeId,
        term: Term,
        success: bool,
    ) {
        if term > *ctx.current_term {
            self.step_down(ctx, term);
            return;
        }

        if *ctx.role == NodeState::Leader && term == *ctx.current_term {
            // Get the snapshot metadata to know last_included_index
            if let Some(snapshot_metadata) = ctx.storage.snapshot_metadata() {
                ctx.replication.handle_install_snapshot_response(
                    from,
                    term,
                    success,
                    snapshot_metadata.last_included_index,
                );
            }
        }
    }

    // Support methods
    pub fn send(
        &self,
        ctx: &mut MessageHandlerContext<T, S, P, SM, C, L, CC, M, TS, O, CCC>,
        to: NodeId,
        msg: RaftMsg<P, L, CC>,
    ) {
        ctx.transport.send(to, msg);
    }

    pub fn broadcast(
        &self,
        ctx: &mut MessageHandlerContext<T, S, P, SM, C, L, CC, M, TS, O, CCC>,
        msg: RaftMsg<P, L, CC>,
    ) {
        // Collect peer IDs first to avoid borrowing issues
        let mut ids = C::new();
        for peer in ctx.config_manager.config().members.iter() {
            ids.push(peer).ok();
        }

        // Now send to each peer
        for peer in ids.iter() {
            self.send(ctx, peer, msg.clone());
        }
    }

    pub fn apply_config_changes(
        &self,
        ctx: &mut MessageHandlerContext<T, S, P, SM, C, L, CC, M, TS, O, CCC>,
        changes: CCC,
    ) where
        C: NodeCollection,
        M: MapCollection,
        CCC: ConfigChangeCollection,
    {
        // Check if we need to notify observer about role change (if we removed ourselves)
        let old_role = self.node_state_to_role(ctx);
        let last_log_index = ctx.storage.last_log_index();

        ctx.config_manager.apply_changes(
            changes,
            *ctx.id,
            last_log_index,
            ctx.replication,
            ctx.observer,
            ctx.role,
        );

        // If we stepped down due to self-removal, notify observer
        let new_role = self.node_state_to_role(ctx);
        if old_role != new_role {
            ctx.observer
                .role_changed(*ctx.id, old_role, new_role, *ctx.current_term);
        }
    }

    pub fn start_pre_vote(
        &self,
        ctx: &mut MessageHandlerContext<T, S, P, SM, C, L, CC, M, TS, O, CCC>,
    ) {
        let pre_vote_request = RoleTransitionManager::start_pre_vote(
            *ctx.id,
            *ctx.current_term,
            ctx.storage,
            ctx.election,
            ctx.observer,
        );

        self.broadcast(ctx, pre_vote_request);
    }

    pub fn start_election(
        &self,
        ctx: &mut MessageHandlerContext<T, S, P, SM, C, L, CC, M, TS, O, CCC>,
    ) {
        let old_role = RoleTransitionManager::node_state_to_role(ctx.role);

        let vote_request = RoleTransitionManager::start_election(
            *ctx.id,
            ctx.current_term,
            ctx.storage,
            ctx.role,
            ctx.election,
            ctx.observer,
            old_role,
        );

        self.broadcast(ctx, vote_request);

        // If we have no peers, we already have majority (1 of 1) - become leader immediately
        if ctx.config_manager.config().members.len() == 0 {
            self.become_leader(ctx);
        }
    }

    fn become_leader(&self, ctx: &mut MessageHandlerContext<T, S, P, SM, C, L, CC, M, TS, O, CCC>) {
        let old_role = RoleTransitionManager::node_state_to_role(ctx.role);

        RoleTransitionManager::become_leader(
            *ctx.id,
            *ctx.current_term,
            ctx.role,
            ctx.storage,
            ctx.config_manager.config().members.iter(),
            ctx.election,
            ctx.replication,
            ctx.observer,
            old_role,
        );

        // Send initial heartbeat
        self.send_append_entries_to_followers(ctx);
    }

    fn step_down(
        &self,
        ctx: &mut MessageHandlerContext<T, S, P, SM, C, L, CC, M, TS, O, CCC>,
        new_term: Term,
    ) {
        let old_role = RoleTransitionManager::node_state_to_role(ctx.role);
        let old_term = *ctx.current_term;

        RoleTransitionManager::step_down(
            *ctx.id,
            old_term,
            new_term,
            ctx.current_term,
            ctx.storage,
            ctx.role,
            ctx.election,
            ctx.observer,
            old_role,
        );
    }

    pub fn send_heartbeats(
        &self,
        ctx: &mut MessageHandlerContext<T, S, P, SM, C, L, CC, M, TS, O, CCC>,
    ) {
        self.send_append_entries_to_followers(ctx);
        ctx.election.timer_service_mut().reset_heartbeat_timer();
    }

    pub fn send_append_entries_to_followers(
        &self,
        ctx: &mut MessageHandlerContext<T, S, P, SM, C, L, CC, M, TS, O, CCC>,
    ) {
        // Collect peer IDs first to avoid borrowing issues
        let mut ids = C::new();
        for peer in ctx.config_manager.config().members.iter() {
            ids.push(peer).ok();
        }

        // Now send to each peer
        for peer in ids.iter() {
            let msg = ctx
                .replication
                .get_append_entries_for_peer(peer, *ctx.id, ctx.storage);
            self.send(ctx, peer, msg);
        }
    }

    fn should_create_snapshot(
        &self,
        ctx: &MessageHandlerContext<T, S, P, SM, C, L, CC, M, TS, O, CCC>,
    ) -> bool {
        ctx.snapshot_manager
            .should_create(ctx.replication.commit_index(), ctx.storage)
    }

    fn create_snapshot_internal(
        &self,
        ctx: &mut MessageHandlerContext<T, S, P, SM, C, L, CC, M, TS, O, CCC>,
    ) -> Result<(), ()>
    where
        SM: StateMachine<Payload = P, SnapshotData = S::SnapshotData>,
    {
        let commit_index = ctx.replication.commit_index();
        ctx.snapshot_manager
            .create(ctx.storage, ctx.state_machine, commit_index)
            .map(|_| ())
            .map_err(|_| ())
    }

    /// Convert NodeState to Observer Role
    fn node_state_to_role(
        &self,
        ctx: &MessageHandlerContext<T, S, P, SM, C, L, CC, M, TS, O, CCC>,
    ) -> Role {
        RoleTransitionManager::node_state_to_role(ctx.role)
    }
}

impl<T, S, P, SM, C, L, CC, M, TS, O, CCC> Default for MessageHandler<T, S, P, SM, C, L, CC, M, TS, O, CCC>
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
    fn default() -> Self {
        Self::new()
    }
}


