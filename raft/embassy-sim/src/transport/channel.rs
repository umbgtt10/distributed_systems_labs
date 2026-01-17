// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::embassy_log_collection::EmbassyLogEntryCollection;
use crate::transport::async_transport::AsyncTransport;
use alloc::string::String;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::{Channel, Receiver, Sender};
use raft_core::raft_messages::RaftMsg;
use raft_core::types::NodeId;

/// Message envelope with sender info
#[derive(Debug, Clone)]
pub struct Envelope {
    pub from: NodeId,
    pub to: NodeId,
    pub message: RaftMsg<String, EmbassyLogEntryCollection>,
}

const CHANNEL_SIZE: usize = 32;

/// Hub that manages all inter-node channels
pub struct ChannelTransportHub {
    // One channel per node (5 nodes)
    channels: [Channel<CriticalSectionRawMutex, Envelope, CHANNEL_SIZE>; 5],
}

impl ChannelTransportHub {
    pub fn new() -> &'static Self {
        // Allocate hub statically (required for embassy channels)
        static HUB: ChannelTransportHub = ChannelTransportHub {
            channels: [
                Channel::new(),
                Channel::new(),
                Channel::new(),
                Channel::new(),
                Channel::new(),
            ],
        };
        &HUB
    }

    pub fn create_transport(&'static self, node_id: NodeId) -> ChannelTransport {
        let rx = self.channels[(node_id - 1) as usize].receiver();
        let tx_list = self.get_all_senders();

        ChannelTransport {
            node_id,
            rx,
            tx_list,
        }
    }

    fn get_all_senders(
        &'static self,
    ) -> [Sender<'static, CriticalSectionRawMutex, Envelope, CHANNEL_SIZE>; 5] {
        [
            self.channels[0].sender(),
            self.channels[1].sender(),
            self.channels[2].sender(),
            self.channels[3].sender(),
            self.channels[4].sender(),
        ]
    }
}

/// Channel-based transport for a single node
pub struct ChannelTransport {
    node_id: NodeId,
    rx: Receiver<'static, CriticalSectionRawMutex, Envelope, CHANNEL_SIZE>,
    tx_list: [Sender<'static, CriticalSectionRawMutex, Envelope, CHANNEL_SIZE>; 5],
}

impl ChannelTransport {
    /// Send a message to a peer
    pub async fn send(&self, to: NodeId, message: RaftMsg<String, EmbassyLogEntryCollection>) {
        let envelope = Envelope {
            from: self.node_id,
            to,
            message,
        };

        let sender = &self.tx_list[(to - 1) as usize];
        sender.send(envelope).await;
    }

    /// Receive a message from any peer
    pub async fn recv(&mut self) -> (NodeId, RaftMsg<String, EmbassyLogEntryCollection>) {
        let envelope = self.rx.receive().await;
        (envelope.from, envelope.message)
    }

    /// Broadcast a message to all peers
    pub async fn broadcast(&self, message: RaftMsg<String, EmbassyLogEntryCollection>) {
        for peer_id in 1..=5 {
            if peer_id != self.node_id {
                self.send(peer_id, message.clone()).await;
            }
        }
    }
}

// Implement AsyncTransport trait for ChannelTransport
impl AsyncTransport for ChannelTransport {
    async fn send(&mut self, to: NodeId, message: RaftMsg<String, EmbassyLogEntryCollection>) {
        ChannelTransport::send(self, to, message).await
    }

    async fn recv(&mut self) -> (NodeId, RaftMsg<String, EmbassyLogEntryCollection>) {
        ChannelTransport::recv(self).await
    }

    async fn broadcast(&mut self, message: RaftMsg<String, EmbassyLogEntryCollection>) {
        ChannelTransport::broadcast(self, message).await
    }
}
