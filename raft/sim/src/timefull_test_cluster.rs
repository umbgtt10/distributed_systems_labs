// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::mocked_timer_service::{MockClock, MockTimerService};
use crate::{
    in_memory_log_entry_collection::InMemoryLogEntryCollection,
    in_memory_map_collection::InMemoryMapCollection, in_memory_state_machine::InMemoryStateMachine,
    in_memory_storage::InMemoryStorage, in_memory_transport::InMemoryTransport,
    message_broker::MessageBroker, vec_node_collection::VecNodeCollection,
};
use indexmap::IndexMap;
use raft_core::timer_service::TimerService;
use raft_core::{
    event::Event, node_collection::NodeCollection, raft_messages::RaftMsg, raft_node::RaftNode,
    types::NodeId,
};
use std::sync::{Arc, Mutex};
type InMemoryTimefullRaftNode = RaftNode<
    InMemoryTransport,
    InMemoryStorage,
    String,
    InMemoryStateMachine,
    VecNodeCollection,
    InMemoryLogEntryCollection,
    InMemoryMapCollection,
    MockTimerService,
>;

pub struct TimefullTestCluster {
    nodes: IndexMap<NodeId, InMemoryTimefullRaftNode>,
    broker: Arc<Mutex<MessageBroker<String, InMemoryLogEntryCollection>>>,
    message_log: Vec<(NodeId, NodeId, RaftMsg<String, InMemoryLogEntryCollection>)>,
    clock: MockClock,
}

impl TimefullTestCluster {
    pub fn new() -> Self {
        Self {
            nodes: IndexMap::new(),
            broker: Arc::new(Mutex::new(MessageBroker::new())),
            message_log: Vec::new(),
            clock: MockClock::new(),
        }
    }

    pub fn get_node(&self, id: NodeId) -> &InMemoryTimefullRaftNode {
        &self.nodes[&id]
    }

    pub fn get_node_mut(&mut self, id: NodeId) -> &mut InMemoryTimefullRaftNode {
        self.nodes.get_mut(&id).unwrap()
    }

    pub fn add_node_with_timeout(
        &mut self,
        id: NodeId,
        election_timeout_min: u64,
        election_timeout_max: u64,
    ) {
        let transport = InMemoryTransport::new(id, self.broker.clone());

        let timer = MockTimerService::new(
            election_timeout_min,
            election_timeout_max,
            50, // heartbeat interval (fixed)
            self.clock.clone(),
            id,
        );

        let mut node = RaftNode::new(
            id,
            InMemoryStorage::new(),
            InMemoryStateMachine::new(),
            timer,
        );
        node.set_transport(transport);
        self.nodes.insert(id, node);
    }

    /// Add node with default timeouts (convenience method)
    pub fn add_node(&mut self, id: NodeId) {
        self.add_node_with_timeout(id, 150, 300);
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

    /// Check all timers and fire events for expired ones
    pub fn process_timer_events(&mut self) {
        let node_ids: Vec<NodeId> = self.nodes.keys().cloned().collect();

        for node_id in node_ids {
            let fired_timers = {
                let node = self.nodes.get(&node_id).unwrap();
                node.timer_service().check_expired()
            };

            for timer_kind in fired_timers.iter() {
                let node = self.nodes.get_mut(&node_id).unwrap();
                node.on_event(Event::TimerFired(timer_kind));
            }
        }
    }

    pub fn deliver_messages(&mut self) {
        loop {
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

            for (node_id, from, msg) in messages_to_deliver {
                self.message_log.push((from, node_id, msg.clone()));

                let node = self.nodes.get_mut(&node_id).unwrap();
                node.on_event(Event::Message { from, msg });
            }
        }
    }

    pub fn advance_time(&mut self, millis: u64) {
        self.clock.advance(millis);
        self.process_timer_events();
    }
}

impl Default for TimefullTestCluster {
    fn default() -> Self {
        Self::new()
    }
}
