// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use raft_core::{log_entry_collection::LogEntryCollection, raft_messages::RaftMsg, types::NodeId};
use std::collections::{HashMap, VecDeque};

type Queue<P, L> = VecDeque<(NodeId, RaftMsg<P, L>)>;

pub struct MessageBroker<P: Clone, L: LogEntryCollection<Payload = P> + Clone> {
    queues: HashMap<NodeId, Queue<P, L>>,
}

impl<P: Clone, L: LogEntryCollection<Payload = P> + Clone> MessageBroker<P, L> {
    pub fn new() -> Self {
        MessageBroker {
            queues: HashMap::new(),
        }
    }

    pub fn peak(&self, node_id: NodeId) -> Option<&VecDeque<(NodeId, RaftMsg<P, L>)>> {
        self.queues.get(&node_id)
    }

    pub fn enqueue(&mut self, from: NodeId, to: NodeId, msg: RaftMsg<P, L>) {
        let queue = self.queues.entry(to).or_default();
        queue.push_back((from, msg));
    }

    pub fn dequeue(&mut self, node_id: NodeId) -> Option<(NodeId, RaftMsg<P, L>)> {
        if let Some(queue) = self.queues.get_mut(&node_id) {
            queue.pop_front()
        } else {
            None
        }
    }

    pub fn dequeue_from(
        &mut self,
        node_id: NodeId,
        from: NodeId,
    ) -> Option<(NodeId, RaftMsg<P, L>)> {
        self.queues.get_mut(&node_id).and_then(|queue| {
            queue
                .iter()
                .position(|(sender, _)| *sender == from)
                .and_then(|pos| queue.remove(pos))
        })
    }
}

impl<P: Clone, L: LogEntryCollection<Payload = P> + Clone> Default for MessageBroker<P, L> {
    fn default() -> Self {
        Self::new()
    }
}
