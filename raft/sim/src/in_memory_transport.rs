use crate::message_broker::MessageBroker;
use raft_core::{raft_messages::RaftMsg, transport::Transport, types::NodeId};
use std::sync::{Arc, Mutex};

pub struct InMemoryTransport {
    node_id: NodeId,
    broker: Arc<Mutex<MessageBroker<String>>>,
}

impl InMemoryTransport {
    pub fn new(node_id: NodeId, broker: Arc<Mutex<MessageBroker<String>>>) -> Self {
        InMemoryTransport { node_id, broker }
    }
}

impl Transport for InMemoryTransport {
    type Payload = String;

    fn send(&mut self, target: NodeId, msg: RaftMsg<Self::Payload>) {
        let mut broker = self.broker.lock().unwrap();
        broker.enqueue(self.node_id, target, msg);
    }
}
