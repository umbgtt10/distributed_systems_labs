use raft_core::{node_collection::NodeCollection, types::NodeId};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VecNodeCollection {
    nodes: Vec<NodeId>,
}

impl VecNodeCollection {
    pub fn new() -> Self {
        Self { nodes: Vec::new() }
    }
}

impl Default for VecNodeCollection {
    fn default() -> Self {
        Self::new()
    }
}

impl NodeCollection for VecNodeCollection {
    fn new() -> Self {
        Self { nodes: Vec::new() }
    }

    fn clear(&mut self) {
        self.nodes.clear();
    }

    fn push(&mut self, id: NodeId) -> Result<(), ()> {
        self.nodes.push(id);
        Ok(())
    }

    fn len(&self) -> usize {
        self.nodes.len()
    }

    fn iter(&self) -> Box<dyn Iterator<Item = &NodeId> + '_> {
        Box::new(self.nodes.as_slice().iter())
    }

    fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }
}
