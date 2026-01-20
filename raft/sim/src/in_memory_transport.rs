// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::{
    in_memory_chunk_collection::InMemoryChunkCollection,
    in_memory_log_entry_collection::InMemoryLogEntryCollection, message_broker::MessageBroker,
};
use raft_core::{raft_messages::RaftMsg, transport::Transport, types::NodeId};
use std::sync::{Arc, Mutex};

pub struct InMemoryTransport {
    node_id: NodeId,
    broker: Arc<Mutex<MessageBroker<String, InMemoryLogEntryCollection, InMemoryChunkCollection>>>,
}

impl InMemoryTransport {
    pub fn new(
        node_id: NodeId,
        broker: Arc<
            Mutex<MessageBroker<String, InMemoryLogEntryCollection, InMemoryChunkCollection>>,
        >,
    ) -> Self {
        InMemoryTransport { node_id, broker }
    }
}

impl Transport for InMemoryTransport {
    type Payload = String;
    type LogEntries = InMemoryLogEntryCollection;
    type ChunkCollection = crate::in_memory_chunk_collection::InMemoryChunkCollection;

    fn send(
        &mut self,
        target: NodeId,
        msg: RaftMsg<Self::Payload, Self::LogEntries, Self::ChunkCollection>,
    ) {
        let mut broker = self.broker.lock().unwrap();
        broker.enqueue(self.node_id, target, msg);
    }
}
