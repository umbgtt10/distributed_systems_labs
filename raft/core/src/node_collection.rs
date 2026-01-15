// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::types::NodeId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CollectionError {
    Full,
}

pub trait NodeCollection {
    type Iter<'a>: Iterator<Item = NodeId>
    where
        Self: 'a;

    fn new() -> Self;
    fn push(&mut self, node_id: NodeId) -> Result<(), CollectionError>;
    fn len(&self) -> usize;
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
    fn clear(&mut self);
    fn iter(&self) -> Self::Iter<'_>;
}
