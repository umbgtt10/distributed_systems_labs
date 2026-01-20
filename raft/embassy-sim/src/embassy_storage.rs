// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use alloc::string::String;
use heapless::Vec;
use raft_core::log_entry::LogEntry;
use raft_core::log_entry_collection::LogEntryCollection;
use raft_core::storage::Storage;
use raft_core::types::{LogIndex, NodeId, Term};

use crate::embassy_log_collection::EmbassyLogEntryCollection;

/// Simple in-memory storage for Embassy Raft nodes
/// In a real system, this would persist to flash
#[derive(Clone)]
pub struct EmbassyStorage {
    current_term: Term,
    voted_for: Option<NodeId>,
    log: Vec<LogEntry<String>, 256>, // Max 256 entries
}

impl EmbassyStorage {
    pub fn new() -> Self {
        Self {
            current_term: 0,
            voted_for: None,
            log: Vec::new(),
        }
    }
}

// Dummy types for snapshot support (not yet implemented)
pub struct DummySnapshotData;
pub struct DummySnapshotBuilder;

impl raft_core::snapshot::SnapshotData for DummySnapshotData {
    type Chunk = ();
    fn len(&self) -> usize { 0 }
    fn chunk_at(&self, _: usize, _: usize) -> Option<Self::Chunk> { None }
}

impl raft_core::snapshot::SnapshotBuilder for DummySnapshotBuilder {
    type Output = DummySnapshotData;
    type ChunkInput = ();
    fn new() -> Self { DummySnapshotBuilder }
    fn add_chunk(&mut self, _: usize, _: Self::ChunkInput) -> Result<(), raft_core::snapshot::SnapshotBuildError> { Ok(()) }
    fn is_complete(&self, _: usize) -> bool { true }
    fn build(self) -> Result<Self::Output, raft_core::snapshot::SnapshotBuildError> { Ok(DummySnapshotData) }
}

impl Clone for DummySnapshotData {
    fn clone(&self) -> Self { DummySnapshotData }
}

impl Storage for EmbassyStorage {
    type Payload = String;
    type LogEntryCollection = EmbassyLogEntryCollection;
    type SnapshotData = DummySnapshotData;
    type SnapshotChunk = ();
    type SnapshotBuilder = DummySnapshotBuilder;

    fn current_term(&self) -> Term {
        self.current_term
    }

    fn set_current_term(&mut self, term: Term) {
        self.current_term = term;
    }

    fn voted_for(&self) -> Option<NodeId> {
        self.voted_for
    }

    fn set_voted_for(&mut self, vote: Option<NodeId>) {
        self.voted_for = vote;
    }

    fn last_log_index(&self) -> LogIndex {
        self.log.len() as LogIndex
    }

    fn last_log_term(&self) -> Term {
        self.log.last().map(|entry| entry.term).unwrap_or(0)
    }

    fn get_entry(&self, index: LogIndex) -> Option<LogEntry<Self::Payload>> {
        if index == 0 || index > self.log.len() as LogIndex {
            None
        } else {
            self.log.get((index - 1) as usize).cloned()
        }
    }

    fn get_entries(&self, start: LogIndex, end: LogIndex) -> Self::LogEntryCollection {
        if start == 0 || start >= end {
            return EmbassyLogEntryCollection::new(&[]);
        }

        let start_idx = (start - 1) as usize;
        let end_idx = (end - 1) as usize;

        if start_idx >= self.log.len() {
            return EmbassyLogEntryCollection::new(&[]);
        }

        // Clamp to available entries to prevent out of bounds
        let actual_end_idx = end_idx.min(self.log.len());

        let slice = &self.log[start_idx..actual_end_idx];
        EmbassyLogEntryCollection::new(slice)
    }

    fn append_entries(&mut self, entries: &[LogEntry<Self::Payload>]) {
        for entry in entries {
            let _ = self.log.push(entry.clone());
        }
    }

    fn truncate_after(&mut self, index: LogIndex) {
        self.log.truncate(index as usize);
    }

    // === Snapshot Methods (Stubs) ===

    fn save_snapshot(&mut self, _snapshot: raft_core::snapshot::Snapshot<Self::SnapshotData>) {
        todo!("Snapshot support not yet implemented")
    }

    fn load_snapshot(&self) -> Option<raft_core::snapshot::Snapshot<Self::SnapshotData>> {
        None // No snapshots yet
    }

    fn snapshot_metadata(&self) -> Option<raft_core::snapshot::SnapshotMetadata> {
        None // No snapshots yet
    }

    fn get_snapshot_chunk(
        &self,
        _offset: usize,
        _max_size: usize,
    ) -> Option<raft_core::snapshot::SnapshotChunk<Self::SnapshotData>> {
        None // No snapshots yet
    }

    fn begin_snapshot_transfer(&mut self) -> Self::SnapshotBuilder {
        DummySnapshotBuilder
    }

    fn finalize_snapshot(
        &mut self,
        _builder: Self::SnapshotBuilder,
        _metadata: raft_core::snapshot::SnapshotMetadata,
    ) -> Result<(), raft_core::snapshot::SnapshotError> {
        todo!("Snapshot support not yet implemented")
    }

    fn discard_entries_before(&mut self, _index: LogIndex) {
        // No-op: compaction not yet implemented
    }

    fn first_log_index(&self) -> LogIndex {
        1 // Always start at 1 (no compaction yet)
    }
}

impl Default for EmbassyStorage {
    fn default() -> Self {
        Self::new()
    }
}
