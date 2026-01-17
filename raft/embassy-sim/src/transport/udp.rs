// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! UDP transport using embassy-net (no_std)

use crate::embassy_log_collection::EmbassyLogEntryCollection;
use crate::transport::async_transport::AsyncTransport;
use crate::transport::raft_msg_serde::WireRaftMsg;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use embassy_net::udp::{PacketMetadata, UdpSocket};
use embassy_net::{IpEndpoint, Ipv4Address};
use raft_core::raft_messages::RaftMsg;
use raft_core::types::NodeId;

const BASE_PORT: u16 = 9000;
const MAX_PACKET_SIZE: usize = 4096;

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
    stack: &'static embassy_net::Stack<'static>,
    peer_addrs: [IpEndpoint; 5],
}

impl UdpTransport {
    pub fn new(node_id: NodeId, stack: &'static embassy_net::Stack<'static>) -> Self {
        // Compute peer addresses (10.0.0.1:9001, 10.0.0.2:9002, etc.)
        let peer_addrs = [
            IpEndpoint::new(Ipv4Address::new(10, 0, 0, 1).into(), BASE_PORT + 1),
            IpEndpoint::new(Ipv4Address::new(10, 0, 0, 2).into(), BASE_PORT + 2),
            IpEndpoint::new(Ipv4Address::new(10, 0, 0, 3).into(), BASE_PORT + 3),
            IpEndpoint::new(Ipv4Address::new(10, 0, 0, 4).into(), BASE_PORT + 4),
            IpEndpoint::new(Ipv4Address::new(10, 0, 0, 5).into(), BASE_PORT + 5),
        ];

        Self {
            node_id,
            stack,
            peer_addrs,
        }
    }
}

impl AsyncTransport for UdpTransport {
    async fn send(&mut self, to: NodeId, message: RaftMsg<String, EmbassyLogEntryCollection>) {
        let node_id = self.node_id; // Cache for use in closures

        // Convert RaftMsg to wire format
        let wire_msg = WireRaftMsg::from(message);

        // Serialize the wire message
        let message_bytes = match postcard::to_allocvec(&wire_msg) {
            Ok(bytes) => bytes,
            Err(_) => {
                crate::info!("Node {} failed to serialize message", node_id);
                return;
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
                crate::info!("Node {} failed to serialize envelope", node_id);
                return;
            }
        };

        let target_addr = self.peer_addrs[(to - 1) as usize];

        // Create socket buffers
        let mut rx_meta = [PacketMetadata::EMPTY; 8];
        let mut tx_meta = [PacketMetadata::EMPTY; 8];
        let mut rx_buffer = [0u8; MAX_PACKET_SIZE * 4];
        let mut tx_buffer = [0u8; MAX_PACKET_SIZE * 4];

        let mut socket = UdpSocket::new(
            *self.stack,
            &mut rx_meta,
            &mut rx_buffer,
            &mut tx_meta,
            &mut tx_buffer,
        );

        let port = BASE_PORT + node_id as u16;
        if let Err(e) = socket.bind(port) {
            crate::info!("Node {} failed to bind UDP socket: {:?}", node_id, e);
            return;
        }

        crate::info!("Node {} sending to {} via UDP", node_id, to);

        if let Err(e) = socket.send_to(&bytes, target_addr).await {
            crate::info!("Node {} failed to send UDP packet: {:?}", node_id, e);
        } else {
            crate::info!("Node {} sent {} bytes to {}", node_id, bytes.len(), to);
        }
    }

    async fn recv(&mut self) -> (NodeId, RaftMsg<String, EmbassyLogEntryCollection>) {
        let node_id = self.node_id; // Cache for use in error messages

        // Create socket buffers
        let mut rx_meta = [PacketMetadata::EMPTY; 8];
        let mut tx_meta = [PacketMetadata::EMPTY; 8];
        let mut rx_buffer = [0u8; MAX_PACKET_SIZE * 4];
        let mut tx_buffer = [0u8; MAX_PACKET_SIZE * 4];

        let mut socket = UdpSocket::new(
            *self.stack,
            &mut rx_meta,
            &mut rx_buffer,
            &mut tx_meta,
            &mut tx_buffer,
        );

        let port = BASE_PORT + node_id as u16;
        if let Err(e) = socket.bind(port) {
            crate::info!(
                "Node {} failed to bind UDP socket for recv: {:?}",
                node_id,
                e
            );
            loop {
                embassy_time::Timer::after(embassy_time::Duration::from_secs(1)).await;
            }
        }

        crate::info!("Node {} listening on UDP port {}", node_id, port);

        loop {
            let mut buf = vec![0u8; MAX_PACKET_SIZE];

            crate::info!("Node {} awaiting recv_from...", node_id);
            match socket.recv_from(&mut buf).await {
                Ok((len, from_addr)) => {
                    crate::info!("Node {} received {} bytes from {}", node_id, len, from_addr);

                    // Deserialize envelope
                    let envelope: Envelope = match postcard::from_bytes(&buf[..len]) {
                        Ok(env) => env,
                        Err(_) => {
                            crate::info!("Node {} failed to deserialize envelope", node_id);
                            continue;
                        }
                    };

                    // Deserialize wire message
                    let wire_msg: WireRaftMsg = match postcard::from_bytes(&envelope.message_bytes)
                    {
                        Ok(msg) => msg,
                        Err(_) => {
                            crate::info!("Node {} failed to deserialize wire message", node_id);
                            continue;
                        }
                    };

                    // Convert to RaftMsg
                    let message: RaftMsg<String, EmbassyLogEntryCollection> =
                        match wire_msg.try_into() {
                            Ok(msg) => msg,
                            Err(_) => {
                                crate::info!("Node {} failed to convert wire message", node_id);
                                continue;
                            }
                        };

                    return (envelope.from, message);
                }
                Err(e) => {
                    crate::info!("Node {} failed to receive UDP packet: {:?}", node_id, e);
                    embassy_time::Timer::after(embassy_time::Duration::from_millis(10)).await;
                }
            }
        }
    }

    async fn broadcast(&mut self, message: RaftMsg<String, EmbassyLogEntryCollection>) {
        for peer_id in 1..=5 {
            if peer_id != self.node_id {
                self.send(peer_id, message.clone()).await;
            }
        }
    }
}
