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

    fn values(&self) -> impl Iterator<Item = LogIndex> + '_ {
        self.map.values().cloned()
    }

    fn clear(&mut self) {
        self.map.clear();
    }

    fn compute_median(&self, additional_value: LogIndex) -> Option<LogIndex> {
        if self.map.is_empty() {
            return Some(additional_value);
        }

        let mut values: Vec<LogIndex> = self.map.iter().map(|(_, v)| *v).collect();
        values.push(additional_value);
        values.sort_unstable();

        let majority_index = values.len() / 2;
        Some(values[majority_index])
    }
}
