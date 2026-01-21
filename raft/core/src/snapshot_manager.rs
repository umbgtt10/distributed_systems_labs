// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::{
    snapshot::{Snapshot, SnapshotError, SnapshotMetadata},
    state_machine::StateMachine,
    storage::Storage,
    types::LogIndex,
};

/// Manages snapshot creation and log compaction
pub struct SnapshotManager {
    threshold: LogIndex,
}

impl SnapshotManager {
    pub fn new(threshold: LogIndex) -> Self {
        Self { threshold }
    }

    /// Get the snapshot threshold
    pub fn threshold(&self) -> LogIndex {
        self.threshold
    }

    /// Check if we should create a snapshot based on log size
    pub fn should_create<S>(&self, commit_index: LogIndex, storage: &S) -> bool
    where
        S: Storage,
    {
        // Only create snapshot if we have enough entries committed
        if commit_index < self.threshold {
            return false;
        }

        // Check if we already have a snapshot at or beyond the threshold
        // We want to snapshot at threshold, not every time commit advances
        if let Some(snapshot) = storage.load_snapshot() {
            if snapshot.metadata.last_included_index >= self.threshold {
                return false;
            }
        }

        true
    }

    /// Create a snapshot of the state machine and save to storage
    ///
    /// Returns the index of the last included entry in the snapshot.
    pub fn create<S, SM>(
        &self,
        storage: &mut S,
        state_machine: &mut SM,
        last_applied: LogIndex,
    ) -> Result<LogIndex, SnapshotError>
    where
        S: Storage<SnapshotData = SM::SnapshotData>,
        SM: StateMachine,
    {
        if last_applied == 0 {
            return Err(SnapshotError::NoEntriesToSnapshot);
        }

        // Get term of last applied entry
        let last_included_term = storage
            .get_entry(last_applied)
            .map(|e| e.term)
            .ok_or(SnapshotError::EntryNotFound)?;

        // Create snapshot from state machine
        let snapshot_data = state_machine.create_snapshot();

        let snapshot = Snapshot {
            metadata: SnapshotMetadata {
                last_included_index: last_applied,
                last_included_term,
            },
            data: snapshot_data,
        };

        // Save to storage
        storage.save_snapshot(snapshot);

        // Compact the log by discarding entries up to the snapshot point
        self.compact(storage, last_applied);

        Ok(last_applied)
    }

    /// Compact the log by discarding entries before the snapshot point
    pub fn compact<S>(&self, storage: &mut S, last_included_index: LogIndex)
    where
        S: Storage,
    {
        storage.discard_entries_before(last_included_index + 1);
    }
}
