use indexmap::IndexMap;
use raft_core::{
    map_collection::MapCollection,
    types::{LogIndex, NodeId},
};

pub struct InMemoryMapCollection {
    map: IndexMap<NodeId, LogIndex>,
}

impl MapCollection for InMemoryMapCollection {
    fn new() -> Self {
        InMemoryMapCollection {
            map: IndexMap::new(),
        }
    }

    fn insert(&mut self, key: NodeId, value: LogIndex) {
        self.map.insert(key, value);
    }

    fn get(&self, key: NodeId) -> Option<LogIndex> {
        self.map.get(&key).cloned()
    }

    fn len(&self) -> usize {
        self.map.len()
    }

    fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    fn values(&self) -> impl Iterator<Item = LogIndex> + '_ {
        self.map.values().cloned()
    }

    fn clear(&mut self) {
        self.map.clear();
    }

    fn compute_median(&self, leader_last_index: LogIndex, total_peers: usize) -> Option<LogIndex> {
        let total_nodes = total_peers + 1; // peers + leader
        let mut indices: Vec<LogIndex> = Vec::new();

        // Add leader's own index
        indices.push(leader_last_index);

        // Add all peer match_index values
        for (_, &index) in self.map.iter() {
            indices.push(index);
        }

        // Pad with zeros for peers we haven't heard from
        while indices.len() < total_nodes {
            indices.push(0);
        }

        // Sort to find median
        indices.sort_unstable();

        // Median is at position that represents majority
        // For N nodes, we need ceil(N/2) = (N+1)/2 nodes
        // The commit index is the value at position: N - (N+1)/2 = (N-1)/2
        let median_idx = (total_nodes - 1) / 2;
        Some(indices[median_idx])
    }
}
