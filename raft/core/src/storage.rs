// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::{
    log_entry::LogEntry,
    log_entry_collection::LogEntryCollection,
    snapshot::{
        Snapshot, SnapshotBuilder, SnapshotChunk, SnapshotData, SnapshotError, SnapshotMetadata,
    },
    types::{LogIndex, NodeId, Term},
};

pub trait Storage {
    type Payload: Clone;
    type LogEntryCollection: LogEntryCollection<Payload = Self::Payload>;
    type SnapshotData: SnapshotData;
    type SnapshotChunk: Clone;
    type SnapshotBuilder: SnapshotBuilder<
        Output = Self::SnapshotData,
        ChunkInput = Self::SnapshotChunk,
    >;

    fn current_term(&self) -> Term;
    fn set_current_term(&mut self, term: Term);

    fn voted_for(&self) -> Option<NodeId>;
    fn set_voted_for(&mut self, vote: Option<NodeId>);

    fn last_log_index(&self) -> LogIndex;
    fn last_log_term(&self) -> Term;

    fn get_entry(&self, index: LogIndex) -> Option<LogEntry<Self::Payload>>;
    fn get_entries(&self, start: LogIndex, end: LogIndex) -> Self::LogEntryCollection;
    fn append_entries(&mut self, entries: &[LogEntry<Self::Payload>]);

    fn truncate_after(&mut self, index: LogIndex);

    // === Snapshot Methods ===

    fn save_snapshot(&mut self, snapshot: Snapshot<Self::SnapshotData>);
    fn load_snapshot(&self) -> Option<Snapshot<Self::SnapshotData>>;
    fn snapshot_metadata(&self) -> Option<SnapshotMetadata>;
    fn get_snapshot_chunk(
        &self,
        offset: usize,
        max_size: usize,
    ) -> Option<SnapshotChunk<Self::SnapshotData>>;
    fn begin_snapshot_transfer(&mut self) -> Self::SnapshotBuilder;
    fn finalize_snapshot(
        &mut self,
        builder: Self::SnapshotBuilder,
        metadata: SnapshotMetadata,
    ) -> Result<(), SnapshotError>;
    fn discard_entries_before(&mut self, index: LogIndex);
    fn first_log_index(&self) -> LogIndex;
}
