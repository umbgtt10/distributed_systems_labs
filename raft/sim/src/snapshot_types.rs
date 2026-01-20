// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! Snapshot types for sim - standard Vec-based implementation

use raft_core::snapshot::{SnapshotBuildError, SnapshotBuilder, SnapshotData};

/// Newtype wrapper around Vec<u8> for sim snapshots
/// (Needed to satisfy orphan rules for trait implementation)
#[derive(Clone, Debug, PartialEq)]
pub struct SimSnapshotData(pub Vec<u8>);

impl SimSnapshotData {
    pub fn new() -> Self {
        Self(Vec::new())
    }

    pub fn from_vec(vec: Vec<u8>) -> Self {
        Self(vec)
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.0
    }
}

impl Default for SimSnapshotData {
    fn default() -> Self {
        Self::new()
    }
}

impl SnapshotData for SimSnapshotData {
    type Chunk = Vec<u8>;

    fn len(&self) -> usize {
        self.0.len()
    }

    fn chunk_at(&self, offset: usize, max_len: usize) -> Option<Self::Chunk> {
        if offset >= self.len() {
            return None;
        }

        let end = (offset + max_len).min(self.len());
        Some(self.0[offset..end].to_vec())
    }
}

/// Builder for sim - accumulates chunks into Vec
pub struct SimSnapshotBuilder {
    data: Vec<u8>,
    expected_offset: usize,
}

impl SnapshotBuilder for SimSnapshotBuilder {
    type Output = SimSnapshotData;
    type ChunkInput = Vec<u8>;

    fn new() -> Self {
        Self {
            data: Vec::new(),
            expected_offset: 0,
        }
    }

    fn add_chunk(
        &mut self,
        offset: usize,
        data: Self::ChunkInput,
    ) -> Result<(), SnapshotBuildError> {
        if offset != self.expected_offset {
            return Err(SnapshotBuildError::InvalidOffset);
        }

        self.data.extend_from_slice(&data);
        self.expected_offset += data.len();
        Ok(())
    }

    fn is_complete(&self, expected_size: usize) -> bool {
        self.expected_offset == expected_size
    }

    fn build(self) -> Result<Self::Output, SnapshotBuildError> {
        Ok(SimSnapshotData(self.data))
    }
}
