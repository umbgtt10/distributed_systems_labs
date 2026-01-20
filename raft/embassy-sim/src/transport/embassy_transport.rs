// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::embassy_log_collection::EmbassyLogEntryCollection;
use crate::heapless_chunk_collection::HeaplessChunkVec;
use alloc::vec::Vec;
use alloc::{string::String, sync::Arc};
use core::cell::RefCell;
use critical_section::Mutex;
use raft_core::{raft_messages::RaftMsg, transport::Transport, types::NodeId};

type Outbox = Vec<(
    NodeId,
    RaftMsg<String, EmbassyLogEntryCollection, HeaplessChunkVec<512>>,
)>;

/// Transport wrapper that queues messages for async sending
#[derive(Clone)]
pub struct EmbassyTransport {
    outbox: Arc<Mutex<RefCell<Outbox>>>,
}

impl EmbassyTransport {
    pub fn new() -> Self {
        Self {
            outbox: Arc::new(Mutex::new(RefCell::new(Vec::new()))),
        }
    }

    pub fn drain_outbox(
        &mut self,
    ) -> Vec<(
        NodeId,
        RaftMsg<String, EmbassyLogEntryCollection, HeaplessChunkVec<512>>,
    )> {
        critical_section::with(|cs| self.outbox.borrow(cs).borrow_mut().drain(..).collect())
    }
}

impl Transport for EmbassyTransport {
    type Payload = String;
    type LogEntries = EmbassyLogEntryCollection;
    type ChunkCollection = HeaplessChunkVec<512>;

    fn send(
        &mut self,
        target: NodeId,
        msg: RaftMsg<Self::Payload, Self::LogEntries, Self::ChunkCollection>,
    ) {
        // Queue message for async sending
        critical_section::with(|cs| {
            self.outbox.borrow(cs).borrow_mut().push((target, msg));
        });
    }
}

impl Default for EmbassyTransport {
    fn default() -> Self {
        Self::new()
    }
}
