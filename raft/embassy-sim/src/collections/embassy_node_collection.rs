// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use heapless::Vec;
use raft_core::{
    collections::node_collection::{CollectionError, NodeCollection},
    types::NodeId,
};

#[derive(Debug, Clone)]
pub struct EmbassyNodeCollection {
    nodes: Vec<NodeId, 10>,
}

impl NodeCollection for EmbassyNodeCollection {
    type Iter<'a> = core::iter::Copied<core::slice::Iter<'a, NodeId>>;

    fn new() -> Self {
        Self { nodes: Vec::new() }
    }

    fn push(&mut self, node_id: NodeId) -> Result<(), CollectionError> {
        self.nodes.push(node_id).map_err(|_| CollectionError::Full)
    }

    fn remove(&mut self, node_id: NodeId) {
        self.nodes.retain(|&id| id != node_id);
    }

    fn len(&self) -> usize {
        self.nodes.len()
    }

    fn clear(&mut self) {
        self.nodes.clear();
    }

    fn iter(&self) -> Self::Iter<'_> {
        self.nodes.iter().copied()
    }
}
