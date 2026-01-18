// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::cancellation_token::CancellationToken;
use crate::cluster::RaftCluster;
use crate::info;
use embassy_executor::Spawner;
use raft_core::observer::EventLevel;

use crate::transport::channel::transport::ChannelTransportHub;

// --- In-Memory Channel Initialization ---

pub async fn initialize_cluster(
    spawner: Spawner,
    cancel: CancellationToken,
    observer_level: EventLevel,
) -> RaftCluster {
    info!("Using Channel transport (In-Memory)");

    // Get the singleton hub
    let hub = ChannelTransportHub::new();

    for node_id in 1..=5 {
        let node_id_u64 = node_id as u64;

        // Create transport for this node from the hub
        let transport = hub.create_transport(node_id_u64);

        // Create the node
        let client_rx = crate::cluster::CLIENT_CHANNELS[(node_id_u64 - 1) as usize].receiver();
        let node = crate::embassy_node::EmbassyNode::new(
            node_id_u64,
            transport,
            client_rx,
            observer_level,
        );

        // Spawn Node Task
        spawner
            .spawn(channel_raft_node_task(node, cancel.clone()))
            .unwrap();
    }

    // Return cluster handle for client interaction
    RaftCluster::new(cancel)
}

// Channel Raft Wrapper
#[embassy_executor::task(pool_size = 5)]
async fn channel_raft_node_task(
    mut node: crate::embassy_node::EmbassyNode<crate::transport::channel::ChannelTransport>,
    cancel: CancellationToken,
) {
    node.run(cancel).await
}
