// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::{
    in_memory_log_entry_collection::InMemoryLogEntryCollection, in_memory_map_collection::InMemoryMapCollection, in_memory_state_machine::InMemoryStateMachine, in_memory_storage::InMemoryStorage, in_memory_transport::InMemoryTransport, message_broker::MessageBroker, vec_node_collection::VecNodeCollection
};
use indexmap::IndexMap;
use raft_core::{
    event::Event, node_collection::NodeCollection, raft_messages::RaftMsg, raft_node::RaftNode,
    types::NodeId,
};
use std::sync::{Arc, Mutex};

type InMemoryRaftNode = RaftNode<
    InMemoryTransport,
    InMemoryStorage,
    String,
    InMemoryStateMachine,
    VecNodeCollection,
    InMemoryLogEntryCollection,
    InMemoryMapCollection
>;

pub struct TestCluster {
    nodes: IndexMap<NodeId, InMemoryRaftNode>,
    broker: Arc<Mutex<MessageBroker<String, InMemoryLogEntryCollection>>>,
    message_log: Vec<(NodeId, NodeId, RaftMsg<String, InMemoryLogEntryCollection>)>,
}

impl TestCluster {
    pub fn new() -> Self {
        Self {
            nodes: IndexMap::new(),
            broker: Arc::new(Mutex::new(MessageBroker::new())),
            message_log: Vec::new(),
        }
    }

    pub fn get_node(&self, id: NodeId) -> &InMemoryRaftNode {
        &self.nodes[&id]
    }

    pub fn get_node_mut(&mut self, id: NodeId) -> &mut InMemoryRaftNode {
        self.nodes.get_mut(&id).unwrap()
    }

    pub fn get_node_ids(&self) -> Vec<NodeId> {
        self.nodes.keys().cloned().collect()
    }

    pub fn get_messages(
        &self,
        recipient: NodeId,
    ) -> Vec<RaftMsg<String, InMemoryLogEntryCollection>> {
        self.message_log
            .iter()
            .filter(|(_, to, _)| *to == recipient)
            .map(|(_, _, msg)| msg.clone())
            .collect()
    }

    pub fn get_messages_from(
        &self,
        sender: NodeId,
        recipient: NodeId,
    ) -> Vec<RaftMsg<String, InMemoryLogEntryCollection>> {
        self.message_log
            .iter()
            .filter(|(from, to, _)| *from == sender && *to == recipient)
            .map(|(_, _, msg)| msg.clone())
            .collect()
    }

    pub fn get_all_messages(
        &self,
    ) -> &[(NodeId, NodeId, RaftMsg<String, InMemoryLogEntryCollection>)] {
        &self.message_log
    }

    pub fn message_count(&self) -> usize {
        self.message_log.len()
    }

    pub fn clear_message_log(&mut self) {
        self.message_log.clear();
    }

    pub fn add_node(&mut self, id: NodeId) {
        let transport = InMemoryTransport::new(id, self.broker.clone());
        let mut node = RaftNode::new(id, InMemoryStorage::new(), InMemoryStateMachine::new());
        node.set_transport(transport);
        self.nodes.insert(id, node);
    }

    pub fn connect_peers(&mut self) {
        let peer_ids: Vec<NodeId> = self.nodes.keys().cloned().collect();
        for node in self.nodes.values_mut() {
            let mut peers = VecNodeCollection::new();
            for &pid in &peer_ids {
                if pid != node.id() {
                    peers.push(pid).ok();
                }
            }
            node.set_peers(peers);
        }
    }

    pub fn deliver_messages(&mut self) {
        loop {
            // Collect all messages first, then deliver them
            let mut messages_to_deliver = Vec::new();
            {
                let mut broker = self.broker.lock().unwrap();
                for &node_id in self.nodes.keys() {
                    while let Some((from, msg)) = broker.dequeue(node_id) {
                        messages_to_deliver.push((node_id, from, msg));
                    }
                }
            }

            if messages_to_deliver.is_empty() {
                break;
            }

            // Now deliver all collected messages
            for (node_id, from, msg) in messages_to_deliver {
                self.message_log.push((from, node_id, msg.clone()));

                let node = self.nodes.get_mut(&node_id).unwrap();
                node.on_event(Event::Message { from, msg });
            }
        }
    }

    pub fn deliver_message_from_to(&mut self, from: NodeId, to: NodeId) {
        let mut broker = self.broker.lock().unwrap();

        // Find and remove the specific message
        if let Some((sender, msg)) = broker.dequeue_from(to, from) {
            drop(broker); // Release lock before calling on_event

            // Record and deliver
            self.message_log.push((sender, to, msg.clone()));

            let node = self.nodes.get_mut(&to).unwrap();
            node.on_event(Event::Message { from: sender, msg });
        }
    }
}

impl Default for TestCluster {
    fn default() -> Self {
        Self::new()
    }
}
