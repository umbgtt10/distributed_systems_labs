// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CollectionError {
    Full,
}

/// Trait for collections of chunks used in snapshot transfer
///
/// This abstraction allows core Raft to remain agnostic about:
/// - Allocation strategy (heap vs stack vs static)
/// - Element size (bytes, words, etc.)
/// - Storage mechanism
///
/// Similar to LogEntryCollection and NodeCollection, this enables
/// the same Raft core to work with different chunk representations.
pub trait ChunkCollection {
    type Item: Copy;

    fn new(data: &[Self::Item]) -> Self;
    fn len(&self) -> usize;
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
    fn as_slice(&self) -> &[Self::Item];
    fn clear(&mut self);
    fn extend_from_slice(&mut self, data: &[Self::Item]) -> Result<(), CollectionError>;
}
