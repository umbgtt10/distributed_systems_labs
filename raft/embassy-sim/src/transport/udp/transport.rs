// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! UDP transport using embassy-net (no_std)

use crate::embassy_log_collection::EmbassyLogEntryCollection;
use crate::transport::async_transport::AsyncTransport;
use crate::transport::udp::serde::WireRaftMsg;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use embassy_net::udp::{PacketMetadata, UdpSocket};
use embassy_net::{IpEndpoint, Ipv4Address};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::{Channel, Receiver, Sender};
use raft_core::raft_messages::RaftMsg;
use raft_core::types::NodeId;

const BASE_PORT: u16 = 9000;
const MAX_PACKET_SIZE: usize = 4096;
const CHANNEL_SIZE: usize = 16;

pub type RaftChannel = Channel<
    CriticalSectionRawMutex,
    (NodeId, RaftMsg<String, EmbassyLogEntryCollection>),
    CHANNEL_SIZE,
>;
pub type RaftReceiver = Receiver<
    'static,
    CriticalSectionRawMutex,
    (NodeId, RaftMsg<String, EmbassyLogEntryCollection>),
    CHANNEL_SIZE,
>;
pub type RaftSender = Sender<
    'static,
    CriticalSectionRawMutex,
    (NodeId, RaftMsg<String, EmbassyLogEntryCollection>),
    CHANNEL_SIZE,
>;

/// Message envelope for serialization
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct Envelope {
    from: NodeId,
    to: NodeId,
    // Store serialized RaftMsg bytes
    message_bytes: Vec<u8>,
}

/// UDP transport using embassy-net
pub struct UdpTransport {
    node_id: NodeId,
    sender: RaftSender,
    receiver: RaftReceiver,
}

impl UdpTransport {
    pub fn new(node_id: NodeId, sender: RaftSender, receiver: RaftReceiver) -> Self {
        Self {
            node_id,
            sender,
            receiver,
        }
    }
}

impl AsyncTransport for UdpTransport {
    async fn send(&mut self, to: NodeId, message: RaftMsg<String, EmbassyLogEntryCollection>) {
        // Send to the sender task via channel
        if self.sender.try_send((to, message)).is_err() {
            info!("Node {} outgoing packet dropped (queue full)", self.node_id);
        }
    }

    async fn recv(&mut self) -> (NodeId, RaftMsg<String, EmbassyLogEntryCollection>) {
        // Simply read from the channel (listener task handles the socket)
        self.receiver.receive().await
    }

    async fn broadcast(&mut self, message: RaftMsg<String, EmbassyLogEntryCollection>) {
        for peer_id in 1..=5 {
            if peer_id != self.node_id {
                self.send(peer_id, message.clone()).await;
            }
        }
    }
}

/// Continuous UDP sender task
pub async fn run_udp_sender(
    node_id: NodeId,
    stack: embassy_net::Stack<'static>,
    receiver: RaftReceiver, // Receives messages to send
) {
    let mut rx_meta = [PacketMetadata::EMPTY; 8];
    let mut tx_meta = [PacketMetadata::EMPTY; 8];
    let mut rx_buffer = [0u8; MAX_PACKET_SIZE * 4];
    let mut tx_buffer = [0u8; MAX_PACKET_SIZE * 4];

    let mut socket = UdpSocket::new(
        stack,
        &mut rx_meta,
        &mut rx_buffer,
        &mut tx_meta,
        &mut tx_buffer,
    );

    // Bind to ephemeral port (0) for sending
    if let Err(e) = socket.bind(0) {
        info!("Node {} failed to bind UDP sender: {:?}", node_id, e);
        return;
    }

    // Compute peer addresses
    let peer_addrs = [
        IpEndpoint::new(Ipv4Address::new(10, 0, 0, 1).into(), BASE_PORT + 1),
        IpEndpoint::new(Ipv4Address::new(10, 0, 0, 2).into(), BASE_PORT + 2),
        IpEndpoint::new(Ipv4Address::new(10, 0, 0, 3).into(), BASE_PORT + 3),
        IpEndpoint::new(Ipv4Address::new(10, 0, 0, 4).into(), BASE_PORT + 4),
        IpEndpoint::new(Ipv4Address::new(10, 0, 0, 5).into(), BASE_PORT + 5),
    ];

    info!("Node {} UDP sender task active", node_id);

    loop {
        let (to, message) = receiver.receive().await;

        // Convert RaftMsg to wire format
        let wire_msg = WireRaftMsg::from(message);

        // Serialize the wire message
        let message_bytes = match postcard::to_allocvec(&wire_msg) {
            Ok(bytes) => bytes,
            Err(_) => {
                info!("Node {} failed to serialize message", node_id);
                continue;
            }
        };

        let envelope = Envelope {
            from: node_id,
            to,
            message_bytes,
        };

        // Serialize the envelope
        let bytes = match postcard::to_allocvec(&envelope) {
            Ok(bytes) => bytes,
            Err(_) => {
                info!("Node {} failed to serialize envelope", node_id);
                continue;
            }
        };

        let target_addr = peer_addrs[(to - 1) as usize];

        // crate::info!("Node {} sending to {} via UDP", node_id, to);

        if let Err(e) = socket.send_to(&bytes, target_addr).await {
            info!("Node {} failed to send UDP packet: {:?}", node_id, e);
        } else {
            // crate::info!("Node {} sent {} bytes to {}", node_id, bytes.len(), to);
        }
    }
}

/// Continuous UDP listener task
pub async fn run_udp_listener(
    node_id: NodeId,
    stack: embassy_net::Stack<'static>,
    sender: RaftSender,
) {
    let mut rx_meta = [PacketMetadata::EMPTY; 8];
    let mut tx_meta = [PacketMetadata::EMPTY; 8];
    let mut rx_buffer = [0u8; MAX_PACKET_SIZE * 4];
    let mut tx_buffer = [0u8; MAX_PACKET_SIZE * 4];

    let mut socket = UdpSocket::new(
        stack,
        &mut rx_meta,
        &mut rx_buffer,
        &mut tx_meta,
        &mut tx_buffer,
    );

    let port = BASE_PORT + node_id as u16;
    if let Err(e) = socket.bind(port) {
        info!("Node {} failed to bind UDP listener: {:?}", node_id, e);
        return;
    }

    info!("Node {} listening on UDP port {}", node_id, port);

    loop {
        let mut buf = vec![0u8; MAX_PACKET_SIZE];

        match socket.recv_from(&mut buf).await {
            Ok((len, _from_addr)) => {
                // Deserialize envelope
                let envelope: Envelope = match postcard::from_bytes(&buf[..len]) {
                    Ok(env) => env,
                    Err(_) => {
                        info!("Node {} failed to deserialize envelope", node_id);
                        continue;
                    }
                };

                // Deserialize wire message
                let wire_msg: WireRaftMsg = match postcard::from_bytes(&envelope.message_bytes) {
                    Ok(msg) => msg,
                    Err(_) => {
                        info!("Node {} failed to deserialize wire message", node_id);
                        continue;
                    }
                };

                // Convert to RaftMsg
                let message: RaftMsg<String, EmbassyLogEntryCollection> = match wire_msg.try_into()
                {
                    Ok(msg) => msg,
                    Err(_) => {
                        info!("Node {} failed to convert wire message", node_id);
                        continue;
                    }
                };

                // Push to channel (drop if full)
                if sender.try_send((envelope.from, message)).is_err() {
                    info!("Node {} incoming packet dropped (channel full)", node_id);
                }
            }
            Err(e) => {
                info!("Node {} UDP receive error: {:?}", node_id, e);
                embassy_time::Timer::after(embassy_time::Duration::from_millis(100)).await;
            }
        }
    }
}
