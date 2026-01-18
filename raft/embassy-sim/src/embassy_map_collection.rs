// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use heapless::index_map::FnvIndexMap;
use raft_core::map_collection::MapCollection;
use raft_core::types::{LogIndex, NodeId};

#[derive(Debug, Clone)]
pub struct EmbassyMapCollection {
    map: FnvIndexMap<NodeId, LogIndex, 8>,
}

impl MapCollection for EmbassyMapCollection {
    fn new() -> Self {
        Self {
            map: FnvIndexMap::new(),
        }
    }

    fn insert(&mut self, key: NodeId, value: LogIndex) {
        let _ = self.map.insert(key, value);
    }

    fn get(&self, key: NodeId) -> Option<LogIndex> {
        self.map.get(&key).copied()
    }

    fn len(&self) -> usize {
        self.map.len()
    }

    fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    fn values(&self) -> impl Iterator<Item = LogIndex> + '_ {
        self.map.values().copied()
    }

    fn clear(&mut self) {
        self.map.clear();
    }

    fn compute_median(&self, leader_last_index: LogIndex, total_peers: usize) -> Option<LogIndex> {
        if self.is_empty() {
            return None;
        }

        let mut indices: heapless::Vec<LogIndex, 10> = heapless::Vec::new();
        let _ = indices.push(leader_last_index);

        for index in self.values() {
            let _ = indices.push(index);
        }

        if indices.is_empty() {
            return None;
        }

        indices.sort_unstable();

        let quorum_size = (total_peers / 2) + 1;
        if indices.len() >= quorum_size {
            Some(indices[indices.len() - quorum_size])
        } else {
            None
        }
    }
}
