// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::{
    chunk_collection::ChunkCollection,
    config_change_collection::ConfigChangeCollection,
    config_change_manager::{ConfigChangeManager, ConfigError},
    election_manager::ElectionManager,
    event::Event,
    log_entry::{ConfigurationChange, EntryType, LogEntry},
    log_entry_collection::LogEntryCollection,
    log_replication_manager::LogReplicationManager,
    map_collection::MapCollection,
    message_handler::MessageHandler,
    node_collection::NodeCollection,
    node_state::NodeState,
    observer::{Observer, TimerKind as ObserverTimerKind},
    raft_messages::RaftMsg,
    snapshot_manager::SnapshotManager,
    state_machine::StateMachine,
    storage::Storage,
    timer_service::{TimerKind, TimerService},
    transport::Transport,
    types::{LogIndex, NodeId, Term},
};

pub struct RaftNode<T, S, P, SM, C, L, CC, M, TS, O, CCC>
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
    id: NodeId,
    role: NodeState,
    current_term: Term,
    transport: T,
    storage: S,
    state_machine: SM,
    observer: O,

    // Delegated responsibilities
    election: ElectionManager<C, TS>,
    replication: LogReplicationManager<M>,
    config_manager: ConfigChangeManager<C, M>,
    snapshot_manager: SnapshotManager,

    _phantom: core::marker::PhantomData<CCC>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientError {
    NotLeader,
}

impl<T, S, P, SM, C, L, CC, M, TS, O, CCC> RaftNode<T, S, P, SM, C, L, CC, M, TS, O, CCC>
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
    /// Internal constructor - use RaftNodeBuilder instead
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new_from_builder(
        id: NodeId,
        storage: S,
        mut state_machine: SM,
        mut election: ElectionManager<C, TS>,
        mut replication: LogReplicationManager<M>,
        transport: T,
        peers: C,
        observer: O,
        snapshot_threshold: LogIndex,
    ) -> Self
    where
        SM: StateMachine<Payload = P, SnapshotData = S::SnapshotData>,
    {
        let current_term = storage.current_term();

        // CRASH RECOVERY: Restore state machine from snapshot if one exists
        let mut last_applied = 0;
        if let Some(snapshot) = storage.load_snapshot() {
            // Restore state machine to snapshot state
            let _ = state_machine.restore_from_snapshot(&snapshot.data);
            last_applied = snapshot.metadata.last_included_index;

            // Note: Storage indices are already adjusted by load_snapshot
            // The storage implementation handles first_log_index internally
        }

        // NOTE: We do NOT replay uncommitted log entries on restart.
        // Raft safety requires that only committed entries are applied to the state machine.
        // After a crash, we don't know which entries were committed, so we only restore
        // from the snapshot. Uncommitted entries will be re-replicated by the leader.

        // Update replication manager's last_applied index
        replication.set_last_applied(last_applied);

        // Start election timer for initial Follower state
        election.timer_service_mut().reset_election_timer();

        let config_manager =
            ConfigChangeManager::new(crate::configuration::Configuration::new(peers));
        let snapshot_manager = SnapshotManager::new(snapshot_threshold);

        RaftNode {
            id,
            role: NodeState::Follower,
            current_term,
            transport,
            storage,
            state_machine,
            observer,
            election,
            replication,
            config_manager,
            snapshot_manager,
            _phantom: core::marker::PhantomData,
        }
    }

    pub fn role(&self) -> &NodeState {
        &self.role
    }

    pub fn storage(&self) -> &S {
        &self.storage
    }

    pub fn state_machine(&self) -> &SM {
        &self.state_machine
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
        if self.config_manager.config().members.is_empty() {
            None
        } else {
            Some(&self.config_manager.config().members)
        }
    }

    pub fn timer_service(&self) -> &TS {
        self.election.timer_service()
    }

    pub fn config(&self) -> &crate::configuration::Configuration<C> {
        self.config_manager.config()
    }

    pub fn is_committed(&self, index: LogIndex) -> bool {
        index <= self.replication.commit_index()
    }

    pub fn observer(&mut self) -> &mut O {
        &mut self.observer
    }

    pub fn on_event(&mut self, event: Event<P, L, CC>)
    where
        SM: StateMachine<Payload = P, SnapshotData = S::SnapshotData>,
        S: Storage<Payload = P, LogEntryCollection = L>,
    {
        match event {
            Event::TimerFired(kind) => self.handle_timer(kind),
            Event::Message { from, msg } => self.handle_message(from, msg),
            Event::ClientCommand(payload) => {
                let _ = self.submit_client_command(payload);
            }
            Event::ConfigChange(change) => {
                let _ = self.submit_config_change(change);
            }
        }
    }

    /// Add a server to the cluster configuration
    ///
    /// This initiates a configuration change to add a new server to the cluster.
    /// The change will be replicated through the Raft log and applied when committed.
    ///
    /// # Safety Constraints
    /// - Only the leader can add servers
    /// - Only one configuration change can be in progress at a time
    /// - The server must not already be in the cluster
    pub fn add_server(&mut self, node_id: NodeId) -> Result<LogIndex, ConfigError>
    where
        SM: StateMachine<Payload = P, SnapshotData = S::SnapshotData>,
        S: Storage<Payload = P, LogEntryCollection = L>,
        C: NodeCollection,
    {
        let is_leader = self.role == NodeState::Leader;
        let commit_index = self.replication.commit_index();

        // Validate and get the configuration change
        let change = self
            .config_manager
            .add_server(node_id, self.id, is_leader, commit_index)?;

        // Submit the change
        let index = self
            .submit_config_change(change)
            .map_err(|_| ConfigError::NotLeader)?;

        // Track the pending change
        self.config_manager.track_pending_change(index);

        Ok(index)
    }

    /// Remove a server from the cluster configuration
    ///
    /// This initiates a configuration change to remove a server from the cluster.
    /// The change will be replicated through the Raft log and applied when committed.
    ///
    /// # Safety Constraints
    /// - Only the leader can remove servers
    /// - Only one configuration change can be in progress at a time
    /// - The server must be in the cluster
    /// - Cannot remove the last server (cluster would be empty)
    pub fn remove_server(&mut self, node_id: NodeId) -> Result<LogIndex, ConfigError>
    where
        SM: StateMachine<Payload = P, SnapshotData = S::SnapshotData>,
        S: Storage<Payload = P, LogEntryCollection = L>,
        C: NodeCollection,
    {
        let is_leader = self.role == NodeState::Leader;
        let commit_index = self.replication.commit_index();

        // Validate and get the configuration change
        let change =
            self.config_manager
                .remove_server(node_id, self.id, is_leader, commit_index)?;

        // Submit the change
        let index = self
            .submit_config_change(change)
            .map_err(|_| ConfigError::NotLeader)?;

        // Track the pending change
        self.config_manager.track_pending_change(index);

        Ok(index)
    }

    pub fn submit_client_command(&mut self, payload: P) -> Result<LogIndex, ClientError>
    where
        SM: StateMachine<Payload = P, SnapshotData = S::SnapshotData>,
        S: Storage<Payload = P, LogEntryCollection = L>,
    {
        if self.role != NodeState::Leader {
            return Err(ClientError::NotLeader);
        }

        let entry = LogEntry {
            term: self.current_term,
            entry_type: EntryType::Command(payload),
        };
        self.storage.append_entries(&[entry]);
        let index = self.storage.last_log_index();

        // If we are a single node cluster, we can advance commit index immediately
        if self.config_manager.config().members.len() == 0 {
            let config_changes: CCC = self.replication.advance_commit_index(
                &self.storage,
                &mut self.state_machine,
                self.config_manager.config(),
            );
            self.apply_config_changes(config_changes);
        }

        self.send_append_entries_to_followers();

        Ok(index)
    }

    fn submit_config_change(&mut self, change: ConfigurationChange) -> Result<LogIndex, ClientError>
    where
        SM: StateMachine<Payload = P, SnapshotData = S::SnapshotData>,
        S: Storage<Payload = P, LogEntryCollection = L>,
    {
        if self.role != NodeState::Leader {
            return Err(ClientError::NotLeader);
        }

        let entry = LogEntry {
            term: self.current_term,
            entry_type: EntryType::ConfigChange(change),
        };
        self.storage.append_entries(&[entry]);
        let index = self.storage.last_log_index();

        // If we are a single node cluster, we can advance commit index immediately
        if self.config_manager.config().members.len() == 0 {
            let config_changes: CCC = self.replication.advance_commit_index(
                &self.storage,
                &mut self.state_machine,
                self.config_manager.config(),
            );
            self.apply_config_changes(config_changes);
        }

        self.send_append_entries_to_followers();

        Ok(index)
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
                    self.start_pre_vote();

                    // If we have no peers, immediately start real election (we're the only node)
                    if self.config_manager.config().members.len() == 0 {
                        self.start_election();
                    }
                }
            }
            TimerKind::Heartbeat => {
                if self.role == NodeState::Leader {
                    self.send_heartbeats();
                }
            }
        }
    }

    fn handle_message(&mut self, from: NodeId, msg: RaftMsg<P, L, CC>)
    where
        SM: StateMachine<Payload = P, SnapshotData = S::SnapshotData>,
        S: Storage<Payload = P, LogEntryCollection = L>,
    {
        let mut handler = MessageHandler {
            id: &self.id,
            role: &mut self.role,
            current_term: &mut self.current_term,
            transport: &mut self.transport,
            storage: &mut self.storage,
            state_machine: &mut self.state_machine,
            observer: &mut self.observer,
            election: &mut self.election,
            replication: &mut self.replication,
            config_manager: &mut self.config_manager,
            snapshot_manager: &mut self.snapshot_manager,
            _phantom: core::marker::PhantomData::<CCC>,
        };

        handler.handle_message(from, msg);
    }

    // ============================================================
    // WRAPPER METHODS FOR INTERNAL USE
    // ============================================================

    fn apply_config_changes(&mut self, changes: CCC)
    where
        C: NodeCollection,
        M: MapCollection,
        CCC: ConfigChangeCollection,
    {
        let mut handler = MessageHandler {
            id: &self.id,
            role: &mut self.role,
            current_term: &mut self.current_term,
            transport: &mut self.transport,
            storage: &mut self.storage,
            state_machine: &mut self.state_machine,
            observer: &mut self.observer,
            election: &mut self.election,
            replication: &mut self.replication,
            config_manager: &mut self.config_manager,
            snapshot_manager: &mut self.snapshot_manager,
            _phantom: core::marker::PhantomData::<CCC>,
        };
        handler.apply_config_changes(changes);
    }

    fn start_pre_vote(&mut self) {
        let mut handler = MessageHandler {
            id: &self.id,
            role: &mut self.role,
            current_term: &mut self.current_term,
            transport: &mut self.transport,
            storage: &mut self.storage,
            state_machine: &mut self.state_machine,
            observer: &mut self.observer,
            election: &mut self.election,
            replication: &mut self.replication,
            config_manager: &mut self.config_manager,
            snapshot_manager: &mut self.snapshot_manager,
            _phantom: core::marker::PhantomData::<CCC>,
        };
        handler.start_pre_vote();
    }

    fn start_election(&mut self) {
        let mut handler = MessageHandler {
            id: &self.id,
            role: &mut self.role,
            current_term: &mut self.current_term,
            transport: &mut self.transport,
            storage: &mut self.storage,
            state_machine: &mut self.state_machine,
            observer: &mut self.observer,
            election: &mut self.election,
            replication: &mut self.replication,
            config_manager: &mut self.config_manager,
            snapshot_manager: &mut self.snapshot_manager,
            _phantom: core::marker::PhantomData::<CCC>,
        };
        handler.start_election();
    }

    fn send_heartbeats(&mut self) {
        let mut handler = MessageHandler {
            id: &self.id,
            role: &mut self.role,
            current_term: &mut self.current_term,
            transport: &mut self.transport,
            storage: &mut self.storage,
            state_machine: &mut self.state_machine,
            observer: &mut self.observer,
            election: &mut self.election,
            replication: &mut self.replication,
            config_manager: &mut self.config_manager,
            snapshot_manager: &mut self.snapshot_manager,
            _phantom: core::marker::PhantomData::<CCC>,
        };
        handler.send_heartbeats();
    }

    fn send_append_entries_to_followers(&mut self) {
        let mut handler = MessageHandler {
            id: &self.id,
            role: &mut self.role,
            current_term: &mut self.current_term,
            transport: &mut self.transport,
            storage: &mut self.storage,
            state_machine: &mut self.state_machine,
            observer: &mut self.observer,
            election: &mut self.election,
            replication: &mut self.replication,
            config_manager: &mut self.config_manager,
            snapshot_manager: &mut self.snapshot_manager,
            _phantom: core::marker::PhantomData::<CCC>,
        };
        handler.send_append_entries_to_followers();
    }
}
