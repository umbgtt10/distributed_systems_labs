// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! Null observer implementation for testing

use raft_core::{
    log_entry_collection::LogEntryCollection,
    observer::{EventLevel, Observer, Role, TimerKind},
    raft_messages::RaftMsg,
    types::{LogIndex, NodeId, Term},
};

/// Null observer that does nothing (zero-cost default for tests)
pub struct NullObserver<P, L>
where
    P: Clone,
    L: LogEntryCollection<Payload = P> + Clone,
{
    _phantom: core::marker::PhantomData<(P, L)>,
}

impl<P, L> NullObserver<P, L>
where
    P: Clone,
    L: LogEntryCollection<Payload = P> + Clone,
{
    pub const fn new() -> Self {
        Self {
            _phantom: core::marker::PhantomData,
        }
    }
}

impl<P, L> Observer for NullObserver<P, L>
where
    P: Clone,
    L: LogEntryCollection<Payload = P> + Clone,
{
    type Payload = P;
    type LogEntries = L;
    type ChunkCollection = crate::in_memory_chunk_collection::InMemoryChunkCollection;

    #[inline]
    fn min_level(&self) -> EventLevel {
        EventLevel::None
    }

    // All methods are no-ops and will be optimized away
    #[inline]
    fn leader_elected(&mut self, _: NodeId, _: Term) {}
    #[inline]
    fn role_changed(&mut self, _: NodeId, _: Role, _: Role, _: Term) {}
    #[inline]
    fn leader_lost(&mut self, _: NodeId, _: Term, _: Term) {}
    #[inline]
    fn election_started(&mut self, _: NodeId, _: Term) {}
    #[inline]
    fn election_timeout(&mut self, _: NodeId, _: Term) {}
    #[inline]
    fn pre_vote_started(&mut self, _: NodeId, _: Term) {}
    #[inline]
    fn pre_vote_requested(&mut self, _: NodeId, _: NodeId, _: Term, _: LogIndex, _: Term) {}
    #[inline]
    fn pre_vote_granted(&mut self, _: NodeId, _: NodeId, _: bool, _: Term) {}
    #[inline]
    fn pre_vote_succeeded(&mut self, _: NodeId, _: Term) {}
    #[inline]
    fn commit_advanced(&mut self, _: NodeId, _: LogIndex, _: LogIndex) {}
    #[inline]
    fn log_appended(&mut self, _: NodeId, _: usize, _: LogIndex, _: Term) {}
    #[inline]
    fn log_truncated(&mut self, _: NodeId, _: LogIndex) {}
    #[inline]
    fn vote_requested(&mut self, _: NodeId, _: NodeId, _: Term, _: LogIndex, _: Term) {}
    #[inline]
    fn vote_granted(&mut self, _: NodeId, _: NodeId, _: bool, _: Term) {}
    #[inline]
    fn voted_for(&mut self, _: NodeId, _: NodeId, _: Term) {}
    #[inline]
    fn heartbeat_sent(&mut self, _: NodeId, _: NodeId, _: Term) {}
    #[inline]
    fn append_entries_sent(&mut self, _: NodeId, _: NodeId, _: usize, _: Term) {}
    #[inline]
    fn append_entries_response(&mut self, _: NodeId, _: NodeId, _: bool, _: LogIndex, _: Term) {}
    #[inline]
    fn timer_fired(&mut self, _: NodeId, _: TimerKind, _: Term) {}
    #[inline]
    fn timer_reset(&mut self, _: NodeId, _: TimerKind, _: Term) {}
    #[inline]
    fn message_sent(&mut self, _: NodeId, _: NodeId, _: &RaftMsg<P, L, Self::ChunkCollection>) {}
    #[inline]
    fn message_received(&mut self, _: NodeId, _: NodeId, _: &RaftMsg<P, L, Self::ChunkCollection>) {
    }
    #[inline]
    fn higher_term_discovered(&mut self, _: NodeId, _: Term, _: Term, _: NodeId) {}
}

impl<P, L> Default for NullObserver<P, L>
where
    P: Clone,
    L: LogEntryCollection<Payload = P> + Clone,
{
    fn default() -> Self {
        Self::new()
    }
}
