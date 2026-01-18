// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::cancellation_token::CancellationToken;
use crate::info;
use embassy_executor::Spawner;

use crate::transport::channel::transport::ChannelTransportHub;

// --- In-Memory Channel Initialization ---

pub async fn initialize_cluster(spawner: Spawner, cancel: CancellationToken) {
    info!("Using Channel transport (In-Memory)");

    // Get the singleton hub
    let hub = ChannelTransportHub::new();

    for node_id in 1..=5 {
        let node_id_u64 = node_id as u64;

        // Create transport for this node from the hub
        let transport = hub.create_transport(node_id_u64);

        // Spawn Node Task
        spawner
            .spawn(channel_raft_node_task(
                node_id_u64,
                transport,
                cancel.clone(),
            ))
            .unwrap();
    }
}

// Channel Raft Wrapper
#[embassy_executor::task(pool_size = 5)]
async fn channel_raft_node_task(
    node_id: u64,
    transport: crate::transport::channel::ChannelTransport,
    cancel: CancellationToken,
) {
    crate::embassy_node::raft_node_task_impl(node_id, transport, cancel).await
}
