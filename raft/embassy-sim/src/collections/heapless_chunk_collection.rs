// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! ChunkCollection implementation for heapless::Vec
//!
//! We use a newtype wrapper to satisfy Rust's orphan rules

use heapless::Vec;
use raft_core::collections::chunk_collection::{ChunkCollection, CollectionError};

/// Newtype wrapper for heapless Vec to implement ChunkCollection
#[derive(Clone, Debug, PartialEq)]
pub struct HeaplessChunkVec<const N: usize>(pub Vec<u8, N>);

impl<const N: usize> HeaplessChunkVec<N> {
    pub fn inner(&self) -> &Vec<u8, N> {
        &self.0
    }

    pub fn into_inner(self) -> Vec<u8, N> {
        self.0
    }
}

impl<const N: usize> ChunkCollection for HeaplessChunkVec<N> {
    type Item = u8;

    fn new(data: &[Self::Item]) -> Self {
        let mut vec = Vec::new();
        for &byte in data {
            vec.push(byte).ok();
        }
        HeaplessChunkVec(vec)
    }

    fn len(&self) -> usize {
        self.0.len()
    }

    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    fn as_slice(&self) -> &[Self::Item] {
        self.0.as_slice()
    }

    fn clear(&mut self) {
        self.0.clear();
    }

    fn extend_from_slice(&mut self, data: &[Self::Item]) -> Result<(), CollectionError> {
        for &byte in data {
            self.0.push(byte).map_err(|_| CollectionError::Full)?;
        }
        Ok(())
    }
}
