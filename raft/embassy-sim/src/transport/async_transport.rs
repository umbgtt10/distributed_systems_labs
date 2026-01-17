// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! Async transport trait for different communication backends

use crate::embassy_log_collection::EmbassyLogEntryCollection;
use alloc::string::String;
use raft_core::raft_messages::RaftMsg;
use raft_core::types::NodeId;

/// Trait for async transport layers
///
/// Implementations can use channels, UDP, UART, CAN bus, etc.
pub trait AsyncTransport {
    /// Send a message to a specific peer
    async fn send(&mut self, to: NodeId, message: RaftMsg<String, EmbassyLogEntryCollection>);

    /// Receive a message from any peer
    /// Returns (sender_node_id, message)
    async fn recv(&mut self) -> (NodeId, RaftMsg<String, EmbassyLogEntryCollection>);

    /// Optional: Broadcast to all peers
    /// Default implementation sends individually
    async fn broadcast(&mut self, message: RaftMsg<String, EmbassyLogEntryCollection>) {
        // Default: not implemented, let sender handle it
        let _ = message;
    }
}
