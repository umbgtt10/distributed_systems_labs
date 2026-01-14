use raft_core::{raft_messages::RaftMsg, types::NodeId};
use std::collections::{HashMap, VecDeque};

pub struct MessageBroker<P> {
    queues: HashMap<NodeId, VecDeque<(NodeId, RaftMsg<P>)>>,
}

impl<P> MessageBroker<P> {
    pub fn new() -> Self {
        MessageBroker {
            queues: HashMap::new(),
        }
    }

    pub fn peak(&self, node_id: NodeId) -> Option<&VecDeque<(NodeId, RaftMsg<P>)>> {
        self.queues.get(&node_id)
    }

    pub fn enqueue(&mut self, from: NodeId, to: NodeId, msg: RaftMsg<P>) {
        let queue = self.queues.entry(to).or_default();
        queue.push_back((from, msg));
    }

    pub fn dequeue(&mut self, node_id: NodeId) -> Option<(NodeId, RaftMsg<P>)> {
        if let Some(queue) = self.queues.get_mut(&node_id) {
            queue.pop_front()
        } else {
            None
        }
    }
}

impl<P> Default for MessageBroker<P> {
    fn default() -> Self {
        Self::new()
    }
}
