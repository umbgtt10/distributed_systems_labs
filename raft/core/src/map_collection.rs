// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::types::{LogIndex, NodeId};

pub trait MapCollection {
    fn new() -> Self;
    fn insert(&mut self, key: NodeId, value: LogIndex);
    fn get(&self, key: NodeId) -> Option<LogIndex>;
    fn values(&self) -> impl Iterator<Item = LogIndex> + '_;
    fn clear(&mut self);
    fn compute_median(&self, leader_last_index: LogIndex, total_peers: usize) -> Option<LogIndex>;
}
