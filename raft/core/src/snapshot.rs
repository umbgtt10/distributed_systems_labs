// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! Snapshot types and abstractions for Raft log compaction
//!
//! This module provides technology-agnostic abstractions for:
//! - Creating snapshots of state machine state
//! - Transferring snapshots in chunks between nodes
//! - Assembling received chunks back into complete snapshots
//!
//! Implementations can use different storage strategies:
//! - Vec<u8> for heap allocation
//! - heapless::Vec<u8, N> for embedded systems
//! - bytes::Bytes for zero-copy networking
//! - Custom types for specific serialization formats

use crate::types::{LogIndex, Term};

// ============================================================
// CORE TYPES
// ============================================================

/// Snapshot metadata
///
/// Tracks which log entries are included in the snapshot,
/// allowing safe log compaction and consistency checks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SnapshotMetadata {
    /// Last log index included in snapshot
    pub last_included_index: LogIndex,
    /// Term of last included entry
    pub last_included_term: Term,
}

/// Complete snapshot with metadata and data
///
/// Generic over data type that implements SnapshotData trait.
/// This allows different implementations to choose their own
/// allocation and serialization strategies.
#[derive(Clone, Debug)]
pub struct Snapshot<D: SnapshotData> {
    pub metadata: SnapshotMetadata,
    pub data: D,
}

/// Snapshot chunk for InstallSnapshot RPC
///
/// Enforces type safety: chunk type MUST be D::Chunk from SnapshotData trait.
/// This prevents mixing chunks from incompatible snapshot implementations.
#[derive(Clone, Debug)]
pub struct SnapshotChunk<D: SnapshotData> {
    /// Offset of this chunk (implementation-defined units)
    pub offset: usize,
    /// The chunk data (type determined by SnapshotData implementation)
    pub data: D::Chunk,
    /// True if this is the final chunk
    pub done: bool,
}

// ============================================================
// TRAITS
// ============================================================

/// Trait for snapshot data that can be transferred in chunks
///
/// This abstraction allows core Raft to remain agnostic about:
/// - Allocation strategy (heap vs stack vs static)
/// - Transfer mechanism (all-at-once vs streaming)
/// - Serialization format (bincode, postcard, JSON, etc.)
/// - Underlying data representation (bytes, words, custom types)
///
/// Similar to LogEntryCollection, this enables the same Raft core
/// to work with different snapshot implementations.
pub trait SnapshotData: Clone {
    /// The type used to represent a chunk of snapshot data
    type Chunk: Clone;

    /// Get the total size (implementation-defined units)
    fn len(&self) -> usize;

    /// Check if empty
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Get a chunk of data starting at offset
    /// Returns None if offset is out of bounds
    fn chunk_at(&self, offset: usize, max_len: usize) -> Option<Self::Chunk>;
}

/// Trait for assembling snapshot chunks during transfer
///
/// This is used by followers to accumulate InstallSnapshot chunks.
/// Separate from SnapshotData to allow implementations that use
/// different types for sending vs receiving (e.g., streaming to disk).
pub trait SnapshotBuilder {
    type Output: SnapshotData;
    type ChunkInput: Clone;

    /// Create a new builder
    fn new() -> Self;

    /// Add a chunk at the specified offset
    /// Returns error if offset is invalid or overlaps
    fn add_chunk(
        &mut self,
        offset: usize,
        data: Self::ChunkInput,
    ) -> Result<(), SnapshotBuildError>;

    /// Check if snapshot is complete
    fn is_complete(&self, expected_size: usize) -> bool;

    /// Finalize and return the complete snapshot data
    fn build(self) -> Result<Self::Output, SnapshotBuildError>;
}

// ============================================================
// ERRORS
// ============================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapshotBuildError {
    InvalidOffset,
    Incomplete,
    OutOfBounds,
}

/// Error types for snapshot operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapshotError {
    CorruptData,
    IncompatibleVersion,
    DeserializationFailed,
    NoEntriesToSnapshot,
    EntryNotFound,
    NotLeader,
}
