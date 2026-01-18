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
///
/// # Note on `async fn` in traits
/// We intentionally use native `async fn` in traits instead of the `async-trait` crate or explicit
/// `impl Future` return types for the following reasons:
/// 1. **Zero-Cost**: Avoids the heap allocation and dynamic dispatch (Boxing) overhead of `async-trait`,
///    which is crucial for embedded/Embassy contexts.
/// 2. **Performance**: Compiles down to efficient state machines.
/// 3. **Usage**: This is an internal application trait, so the auto-trait bound limitations (Send)
///    warned by the compiler are acceptable in this specific `no_std` context.
#[allow(async_fn_in_trait)]
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
