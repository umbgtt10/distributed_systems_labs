// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::in_memory_chunk_collection::InMemoryChunkCollection;
use crate::in_memory_log_entry_collection::InMemoryLogEntryCollection;
use crate::snapshot_types::{SimSnapshotBuilder, SimSnapshotData};
use raft_core::{
    log_entry::LogEntry,
    log_entry_collection::LogEntryCollection,
    snapshot::{
        Snapshot, SnapshotBuilder, SnapshotChunk, SnapshotData, SnapshotError, SnapshotMetadata,
    },
    storage::Storage,
    types::{LogIndex, NodeId, Term},
};

#[derive(Clone, Debug)]
pub struct InMemoryStorage {
    current_term: Term,
    voted_for: Option<NodeId>,
    log: InMemoryLogEntryCollection,
    // Snapshot support
    snapshot: Option<Snapshot<SimSnapshotData>>,
    pending_snapshot_data: Vec<u8>,
    first_index: LogIndex,
}

impl InMemoryStorage {
    pub fn new() -> Self {
        Self {
            current_term: 0,
            voted_for: None,
            log: InMemoryLogEntryCollection::new(&[]),
            snapshot: None,
            pending_snapshot_data: Vec::new(),
            first_index: 1,
        }
    }
}

impl Default for InMemoryStorage {
    fn default() -> Self {
        Self::new()
    }
}

impl Storage for InMemoryStorage {
    type Payload = String;
    type LogEntryCollection = InMemoryLogEntryCollection;
    type SnapshotData = SimSnapshotData;
    type SnapshotChunk = InMemoryChunkCollection;
    type SnapshotBuilder = SimSnapshotBuilder;

    fn current_term(&self) -> Term {
        self.current_term
    }

    fn set_current_term(&mut self, term: Term) {
        self.current_term = term;
    }

    fn voted_for(&self) -> Option<NodeId> {
        self.voted_for
    }

    fn set_voted_for(&mut self, voted_for: Option<NodeId>) {
        self.voted_for = voted_for;
    }

    fn last_log_index(&self) -> LogIndex {
        if self.log.is_empty() {
            self.first_index.saturating_sub(1)
        } else {
            self.first_index + self.log.len() as LogIndex - 1
        }
    }

    fn last_log_term(&self) -> Term {
        if let Some(last_entry) = self.log.as_slice().last() {
            last_entry.term
        } else if let Some(snapshot) = &self.snapshot {
            snapshot.metadata.last_included_term
        } else {
            0
        }
    }

    fn get_entry(&self, index: LogIndex) -> Option<LogEntry<String>> {
        if index < self.first_index {
            return None; // Entry is in snapshot
        }

        let adjusted_index = (index - self.first_index) as usize;
        self.log.as_slice().get(adjusted_index).cloned()
    }

    fn get_entries(&self, start: LogIndex, end: LogIndex) -> InMemoryLogEntryCollection {
        if start < self.first_index || start >= end {
            return InMemoryLogEntryCollection::new(&[]);
        }

        let adjusted_start = (start - self.first_index) as usize;
        let adjusted_end = (end - self.first_index) as usize;

        let entries = self
            .log
            .as_slice()
            .get(adjusted_start..adjusted_end)
            .unwrap_or(&[])
            .to_vec();
        InMemoryLogEntryCollection::new(&entries)
    }

    fn append_entries(&mut self, entries: &[LogEntry<String>]) {
        for entry in entries {
            self.log.push(entry.clone()).unwrap();
        }
    }

    fn truncate_after(&mut self, index: LogIndex) {
        // Convert 1-based log index to 0-based Vec index (adjusted by first_index)
        if index < self.first_index {
            self.log = InMemoryLogEntryCollection::new(&[]);
            return;
        }

        let truncate_at = (index - self.first_index + 1) as usize;
        self.log.truncate(truncate_at);
    }

    fn save_snapshot(&mut self, snapshot: Snapshot<Self::SnapshotData>) {
        self.snapshot = Some(snapshot);
    }

    fn load_snapshot(&self) -> Option<Snapshot<Self::SnapshotData>> {
        self.snapshot.clone()
    }

    fn snapshot_metadata(&self) -> Option<SnapshotMetadata> {
        self.snapshot.as_ref().map(|s| s.metadata.clone())
    }

    fn get_snapshot_chunk(
        &self,
        offset: usize,
        max_size: usize,
    ) -> Option<SnapshotChunk<Self::SnapshotData>> {
        let snapshot = self.snapshot.as_ref()?;
        let chunk_data = snapshot.data.chunk_at(offset, max_size)?;

        let total_size = snapshot.data.len();
        let done = offset + chunk_data.len() >= total_size;

        Some(SnapshotChunk {
            offset,
            data: chunk_data,
            done,
        })
    }

    fn apply_snapshot_chunk(
        &mut self,
        offset: u64,
        chunk: Self::SnapshotChunk,
        done: bool,
        last_included_index: LogIndex,
        last_included_term: Term,
    ) -> Result<(), SnapshotError> {
        if offset == 0 {
            self.pending_snapshot_data.clear();
        }

        if offset as usize != self.pending_snapshot_data.len() {
            return Err(SnapshotError::CorruptData);
        }

        self.pending_snapshot_data
            .extend_from_slice(&chunk.to_vec());

        if done {
            let snapshot_data = SimSnapshotData::from_vec(self.pending_snapshot_data.clone());

            let snapshot = Snapshot {
                metadata: SnapshotMetadata {
                    last_included_index,
                    last_included_term,
                },
                data: snapshot_data,
            };
            self.save_snapshot(snapshot);
            self.pending_snapshot_data.clear();
        }
        Ok(())
    }

    fn begin_snapshot_transfer(&mut self) -> Self::SnapshotBuilder {
        SimSnapshotBuilder::new()
    }

    fn finalize_snapshot(
        &mut self,
        builder: Self::SnapshotBuilder,
        metadata: SnapshotMetadata,
    ) -> Result<(), SnapshotError> {
        let data = builder.build().map_err(|_| SnapshotError::CorruptData)?;

        self.first_index = metadata.last_included_index + 1;
        self.snapshot = Some(Snapshot { metadata, data });
        Ok(())
    }

    fn discard_entries_before(&mut self, index: LogIndex) {
        if index <= self.first_index {
            return;
        }

        let num_to_discard = (index - self.first_index) as usize;

        if num_to_discard >= self.log.len() {
            // Discarding all entries
            self.log = InMemoryLogEntryCollection::new(&[]);
            // New first index is the one we're discarding up to
            self.first_index = index;
        } else {
            let remaining = self.log.as_slice()[num_to_discard..].to_vec();
            self.log = InMemoryLogEntryCollection::new(&remaining);
            self.first_index = index;
        }
    }

    fn first_log_index(&self) -> LogIndex {
        self.first_index
    }
}
