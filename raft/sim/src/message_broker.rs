// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use raft_core::{
    chunk_collection::ChunkCollection, log_entry_collection::LogEntryCollection,
    raft_messages::RaftMsg, types::NodeId,
};
use std::collections::{HashMap, VecDeque};

type Queue<P, L, C> = VecDeque<(NodeId, RaftMsg<P, L, C>)>;

pub struct MessageBroker<
    P: Clone,
    L: LogEntryCollection<Payload = P> + Clone,
    C: ChunkCollection + Clone,
> {
    queues: HashMap<NodeId, Queue<P, L, C>>,
}

impl<P: Clone, L: LogEntryCollection<Payload = P> + Clone, C: ChunkCollection + Clone>
    MessageBroker<P, L, C>
{
    pub fn new() -> Self {
        MessageBroker {
            queues: HashMap::new(),
        }
    }

    pub fn peak(&self, node_id: NodeId) -> Option<&Queue<P, L, C>> {
        self.queues.get(&node_id)
    }

    pub fn enqueue(&mut self, from: NodeId, to: NodeId, msg: RaftMsg<P, L, C>) {
        let queue = self.queues.entry(to).or_default();
        queue.push_back((from, msg));
    }

    pub fn dequeue(&mut self, node_id: NodeId) -> Option<(NodeId, RaftMsg<P, L, C>)> {
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
    ) -> Option<(NodeId, RaftMsg<P, L, C>)> {
        self.queues.get_mut(&node_id).and_then(|queue| {
            queue
                .iter()
                .position(|(sender, _)| *sender == from)
                .and_then(|pos| queue.remove(pos))
        })
    }
}

impl<P: Clone, L: LogEntryCollection<Payload = P> + Clone, C: ChunkCollection + Clone> Default
    for MessageBroker<P, L, C>
{
    fn default() -> Self {
        Self::new()
    }
}
