// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::{
    chunk_collection::ChunkCollection, election_manager::ElectionManager, log_entry_collection::LogEntryCollection, log_replication_manager::LogReplicationManager, map_collection::MapCollection, node_collection::NodeCollection, observer::Observer, raft_node::RaftNode, state_machine::StateMachine, storage::Storage, timer_service::TimerService, transport::Transport, types::{LogIndex, NodeId}
};

/// Builder for constructing a RaftNode with proper initialization order
pub struct RaftNodeBuilder<S, SM, P> {
    id: NodeId,
    storage: S,
    state_machine: SM,
    snapshot_threshold: LogIndex,
    _phantom: core::marker::PhantomData<P>,
}

impl<S, SM, P> RaftNodeBuilder<S, SM, P>
where
    S: Storage<Payload = P>,
    SM: StateMachine<Payload = P>,
{
    /// Create a new builder with id, storage, and state machine
    pub fn new(id: NodeId, storage: S, state_machine: SM) -> Self {
        Self {
            id,
            storage,
            state_machine,
            snapshot_threshold: 10, // Default threshold
            _phantom: core::marker::PhantomData,
        }
    }

    /// Configure snapshot threshold (default: 10)
    pub fn with_snapshot_threshold(mut self, threshold: LogIndex) -> Self {
        self.snapshot_threshold = threshold;
        self
    }

    /// Add election manager (with embedded timer service)
    pub fn with_election<C, TS>(
        self,
        election: ElectionManager<C, TS>,
    ) -> RaftNodeBuilderWithElection<S, SM, P, C, TS>
    where
        S: Storage<Payload = P>,
        SM: StateMachine<Payload = P>,
        P: Clone,
        C: NodeCollection,
        TS: TimerService,
    {
        RaftNodeBuilderWithElection {
            id: self.id,
            storage: self.storage,
            state_machine: self.state_machine,
            snapshot_threshold: self.snapshot_threshold,
            election,
        }
    }
}

/// Builder after election manager is added
pub struct RaftNodeBuilderWithElection<S, SM, P, C, TS>
where
    C: NodeCollection,
    SM: StateMachine<Payload = P>,
    P: Clone,
    C: NodeCollection,
    TS: TimerService,
{
    id: NodeId,
    storage: S,
    state_machine: SM,
    snapshot_threshold: LogIndex,
    election: ElectionManager<C, TS>,
}

impl<S, SM, P, C, TS> RaftNodeBuilderWithElection<S, SM, P, C, TS>
where
    S: Storage<Payload = P>,
    SM: StateMachine<Payload = P>,
    P: Clone,
    C: NodeCollection,
    TS: TimerService,
{
    /// Add log replication manager
    pub fn with_replication<M>(
        self,
        replication: LogReplicationManager<M>,
    ) -> RaftNodeBuilderWithReplication<S, SM, P, C, TS, M>
    where
        S: Storage<Payload = P>,
        SM: StateMachine<Payload = P>,
        P: Clone,
        C: NodeCollection,
        TS: TimerService,
        M: MapCollection,
    {
        RaftNodeBuilderWithReplication {
            id: self.id,
            storage: self.storage,
            state_machine: self.state_machine,
            snapshot_threshold: self.snapshot_threshold,
            election: self.election,
            replication,
        }
    }
}

/// Builder after replication manager is added
pub struct RaftNodeBuilderWithReplication<S, SM, P, C, TS, M>
where
    C: NodeCollection,
    SM: StateMachine<Payload = P>,
    P: Clone,
    C: NodeCollection,
    TS: TimerService,
    M: MapCollection,
{
    id: NodeId,
    storage: S,
    state_machine: SM,
    snapshot_threshold: LogIndex,
    election: ElectionManager<C, TS>,
    replication: LogReplicationManager<M>,
}

impl<S, SM, P, C, TS, M> RaftNodeBuilderWithReplication<S, SM, P, C, TS, M>
where
    S: Storage<Payload = P>,
    SM: StateMachine<Payload = P>,
    P: Clone,
    C: NodeCollection,
    TS: TimerService,
    M: MapCollection,
{
    /// Add transport, peers, and observer to complete construction
    pub fn with_transport<T, L, CC, O>(
        self,
        transport: T,
        peers: C,
        observer: O,
    ) -> RaftNode<T, S, P, SM, C, L, CC, M, TS, O>
    where
        P: Clone,
        T: Transport<Payload = P, LogEntries = L, ChunkCollection = CC>,
        L: LogEntryCollection<Payload = P> + Clone,
        CC: ChunkCollection + Clone,
        S: Storage<Payload = P, LogEntryCollection = L, SnapshotChunk = CC> + Clone,
        O: Observer<Payload = P, LogEntries = L, ChunkCollection = CC>,
    {
        RaftNode::new_from_builder(
            self.id,
            self.storage,
            self.state_machine,
            self.election,
            self.replication,
            transport,
            peers,
            observer,
            self.snapshot_threshold,
        )
    }
}
