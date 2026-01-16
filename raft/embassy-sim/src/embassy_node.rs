// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use embassy_time::{Duration, Timer};

use crate::cancellation_token::CancellationToken;
use crate::embassy_storage::EmbassyStorage;
use crate::embassy_timer::EmbassyTimer;
use crate::led_state::LedState;
use crate::transport::channel::ChannelTransport;

use raft_core::{node_state::NodeState, raft_node::RaftNode};

#[embassy_executor::task(pool_size = 5)]
pub async fn raft_node_task(node_id: u8, transport: ChannelTransport, cancel: CancellationToken) {
    crate::info!("Node {} starting...", node_id);

    let storage = EmbassyStorage::new();
    let timer = EmbassyTimer::new();
    let mut led = LedState::new(node_id);

    // TODO: Build RaftNode using builder pattern
    // let mut node = RaftNode::builder()
    //     .with_node_id(node_id)
    //     .with_storage(storage)
    //     .with_transport(transport)
    //     .with_timer(timer)
    //     .build();

    crate::info!("Node {} initialized", node_id);

    loop {
        // Check for cancellation
        if cancel.is_cancelled() {
            crate::info!("Node {} shutting down gracefully", node_id);
            break;
        }

        // TODO: Main node loop
        // 1. Check timer for election timeout
        // 2. Process incoming messages
        // 3. Send heartbeats if leader
        // 4. Update LED based on role

        // Placeholder: Update LED based on current role
        // let role = node.role();
        // led.update(role);

        // Simulate some work
        info!("Node {} doing some work...", node_id);
        Timer::after(Duration::from_millis(100)).await;
    }

    crate::info!("Node {} shutdown complete", node_id);
}
