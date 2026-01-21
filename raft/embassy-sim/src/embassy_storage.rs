// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::collections::embassy_log_collection::EmbassyLogEntryCollection;
use crate::collections::heapless_chunk_collection::HeaplessChunkVec;
use crate::embassy_state_machine::EmbassySnapshotData;
use alloc::string::String;
use heapless::Vec;
use raft_core::{
    collections::{chunk_collection::ChunkCollection, log_entry_collection::LogEntryCollection},
    log_entry::LogEntry,
    snapshot::{SnapshotBuilder, SnapshotData},
    storage::Storage,
    types::{LogIndex, NodeId, Term},
};

/// Simple in-memory storage for Embassy Raft nodes
/// In a real system, this would persist to flash
#[derive(Clone)]
pub struct EmbassyStorage {
    current_term: Term,
    voted_for: Option<NodeId>,
    log: Vec<LogEntry<String>, 256>, // Max 256 entries
    snapshot: Option<raft_core::snapshot::Snapshot<EmbassySnapshotData>>,
    first_log_index: LogIndex,
}

impl EmbassyStorage {
    pub fn new() -> Self {
        Self {
            current_term: 0,
            voted_for: None,
            log: Vec::new(),
            snapshot: None,
            first_log_index: 1,
        }
    }
}

/// Snapshot builder for incremental snapshot reception
pub struct EmbassySnapshotBuilder {
    data: Vec<u8, 512>,
}

impl raft_core::snapshot::SnapshotBuilder for EmbassySnapshotBuilder {
    type Output = EmbassySnapshotData;
    type ChunkInput = HeaplessChunkVec<512>;

    fn new() -> Self {
        EmbassySnapshotBuilder { data: Vec::new() }
    }

    fn add_chunk(
        &mut self,
        _offset: usize,
        chunk: Self::ChunkInput,
    ) -> Result<(), raft_core::snapshot::SnapshotBuildError> {
        // Append chunk data to our buffer
        for byte in chunk.as_slice() {
            self.data
                .push(*byte)
                .map_err(|_| raft_core::snapshot::SnapshotBuildError::OutOfBounds)?;
        }
        Ok(())
    }

    fn is_complete(&self, expected_size: usize) -> bool {
        self.data.len() >= expected_size
    }

    fn build(self) -> Result<Self::Output, raft_core::snapshot::SnapshotBuildError> {
        Ok(EmbassySnapshotData { data: self.data })
    }
}

impl Storage for EmbassyStorage {
    type Payload = String;
    type LogEntryCollection = EmbassyLogEntryCollection;
    type SnapshotData = EmbassySnapshotData;
    type SnapshotChunk = HeaplessChunkVec<512>;
    type SnapshotBuilder = EmbassySnapshotBuilder;

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
        if self.log.is_empty() {
            // If no entries, return the last compacted index
            if let Some(snapshot) = &self.snapshot {
                snapshot.metadata.last_included_index
            } else {
                0
            }
        } else {
            self.first_log_index + self.log.len() as LogIndex - 1
        }
    }

    fn last_log_term(&self) -> Term {
        if let Some(last_entry) = self.log.last() {
            last_entry.term
        } else if let Some(snapshot) = &self.snapshot {
            snapshot.metadata.last_included_term
        } else {
            0
        }
    }

    fn get_entry(&self, index: LogIndex) -> Option<LogEntry<Self::Payload>> {
        if index < self.first_log_index {
            None // Entry was compacted
        } else {
            let offset = (index - self.first_log_index) as usize;
            self.log.get(offset).cloned()
        }
    }

    fn get_entries(&self, start: LogIndex, end: LogIndex) -> Self::LogEntryCollection {
        if start >= end || start < self.first_log_index {
            return EmbassyLogEntryCollection::new(&[]);
        }

        let start_offset = (start - self.first_log_index) as usize;
        let end_offset = (end - self.first_log_index) as usize;

        if start_offset >= self.log.len() {
            return EmbassyLogEntryCollection::new(&[]);
        }

        // Clamp to available entries to prevent out of bounds
        let actual_end_offset = end_offset.min(self.log.len());

        let slice = &self.log[start_offset..actual_end_offset];
        EmbassyLogEntryCollection::new(slice)
    }

    fn append_entries(&mut self, entries: &[LogEntry<Self::Payload>]) {
        for entry in entries {
            let _ = self.log.push(entry.clone());
        }
    }

    fn truncate_after(&mut self, index: LogIndex) {
        if index < self.first_log_index {
            // Truncate all entries
            self.log.clear();
        } else {
            let offset = (index - self.first_log_index + 1) as usize;
            self.log.truncate(offset);
        }
    }

    // === Snapshot Methods ===

    fn save_snapshot(&mut self, snapshot: raft_core::snapshot::Snapshot<Self::SnapshotData>) {
        self.snapshot = Some(snapshot);
    }

    fn load_snapshot(&self) -> Option<raft_core::snapshot::Snapshot<Self::SnapshotData>> {
        self.snapshot.clone()
    }

    fn snapshot_metadata(&self) -> Option<raft_core::snapshot::SnapshotMetadata> {
        self.snapshot.as_ref().map(|s| s.metadata.clone())
    }

    fn get_snapshot_chunk(
        &self,
        offset: usize,
        max_size: usize,
    ) -> Option<raft_core::snapshot::SnapshotChunk<Self::SnapshotData>> {
        let snapshot = self.snapshot.as_ref()?;
        let chunk_data = snapshot.data.chunk_at(offset, max_size)?;

        let done = offset + chunk_data.len() >= snapshot.data.len();

        Some(raft_core::snapshot::SnapshotChunk {
            offset,
            data: chunk_data,
            done,
        })
    }

    fn begin_snapshot_transfer(&mut self) -> Self::SnapshotBuilder {
        EmbassySnapshotBuilder::new()
    }

    fn apply_snapshot_chunk(
        &mut self,
        offset: u64,
        chunk: Self::SnapshotChunk,
        done: bool,
        last_included_index: LogIndex,
        last_included_term: Term,
    ) -> Result<(), raft_core::snapshot::SnapshotError> {
        // For single-chunk transfers (embassy typical case)
        if offset == 0 && done {
            // Convert HeaplessChunkVec to Vec<u8, 512>
            let mut data = Vec::new();
            for byte in chunk.as_slice() {
                data.push(*byte)
                    .map_err(|_| raft_core::snapshot::SnapshotError::CorruptData)?;
            }
            let snapshot_data = EmbassySnapshotData { data };
            let snapshot = raft_core::snapshot::Snapshot {
                metadata: raft_core::snapshot::SnapshotMetadata {
                    last_included_index,
                    last_included_term,
                },
                data: snapshot_data,
            };
            self.snapshot = Some(snapshot);
            Ok(())
        } else {
            // Multi-chunk not supported in this simple implementation
            Err(raft_core::snapshot::SnapshotError::CorruptData)
        }
    }

    fn finalize_snapshot(
        &mut self,
        builder: Self::SnapshotBuilder,
        metadata: raft_core::snapshot::SnapshotMetadata,
    ) -> Result<(), raft_core::snapshot::SnapshotError> {
        let data = builder
            .build()
            .map_err(|_| raft_core::snapshot::SnapshotError::CorruptData)?;
        self.snapshot = Some(raft_core::snapshot::Snapshot { metadata, data });
        Ok(())
    }

    fn discard_entries_before(&mut self, index: LogIndex) {
        // Discard entries with log index < index
        if index <= self.first_log_index {
            return; // Nothing to discard
        }

        let num_to_discard = (index - self.first_log_index) as usize;

        if num_to_discard >= self.log.len() {
            // Discard all entries
            self.log.clear();
        } else {
            // Shift remaining entries to beginning
            let mut new_log = Vec::new();
            for i in num_to_discard..self.log.len() {
                let _ = new_log.push(self.log[i].clone());
            }
            self.log = new_log;
        }

        self.first_log_index = index;
    }

    fn first_log_index(&self) -> LogIndex {
        self.first_log_index
    }
}

impl Default for EmbassyStorage {
    fn default() -> Self {
        Self::new()
    }
}
