// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use raft_core::chunk_collection::{ChunkCollection, CollectionError};

/// In-memory implementation of ChunkCollection using Vec<u8>
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InMemoryChunkCollection {
    data: Vec<u8>,
}

impl InMemoryChunkCollection {
    pub fn new() -> Self {
        Self { data: Vec::new() }
    }

    pub fn from_vec(data: Vec<u8>) -> Self {
        Self { data }
    }

    pub fn to_vec(&self) -> Vec<u8> {
        self.data.clone()
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.data
    }
}

impl Default for InMemoryChunkCollection {
    fn default() -> Self {
        Self::new()
    }
}

impl ChunkCollection for InMemoryChunkCollection {
    type Item = u8;

    fn new(data: &[Self::Item]) -> Self {
        Self {
            data: data.to_vec(),
        }
    }

    fn len(&self) -> usize {
        self.data.len()
    }

    fn as_slice(&self) -> &[Self::Item] {
        &self.data
    }

    fn clear(&mut self) {
        self.data.clear();
    }

    fn extend_from_slice(&mut self, data: &[Self::Item]) -> Result<(), CollectionError> {
        self.data.extend_from_slice(data);
        Ok(())
    }
}
