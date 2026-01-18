// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::cancellation_token::CancellationToken;
use crate::info;
use alloc::vec::Vec;
use embassy_executor::Spawner; // Macros

use crate::transport::udp::config::{self, get_node_config};
use crate::transport::udp::driver::{MockNetDriver, NetworkBus};
use crate::transport::udp::transport::{self, UdpTransport};

pub async fn initialize_cluster(spawner: Spawner, cancel: CancellationToken) {
    info!("Using UDP transport (simulated Ethernet)");
    info!("WireRaftMsg serialization layer: COMPLETE ✓");

    // Create shared network bus for all nodes
    static NETWORK_BUS: NetworkBus = NetworkBus::new();

    // Local storage for network stack handles
    let mut stacks = Vec::with_capacity(5);

    // Create network stacks for all 5 nodes
    for node_id in 1..=5 {
        // SAFETY: Each node gets a unique NodeNetworkResources instance from
        // get_node_resources(node_id), which returns a mutable reference to a
        // distinct static resource. The resources array has 5 separate entries,
        // ensuring no aliasing occurs. The mutable reference is consumed by
        // embassy_net::new() and not accessed elsewhere during this initialization.
        let (stack, runner) = unsafe {
            let driver = MockNetDriver::new(node_id, &NETWORK_BUS);
            let config = get_node_config(node_id);
            let resources = config::get_node_resources(node_id);
            let seed = 0x0123_4567_89AB_CDEF_u64 + node_id as u64;

            embassy_net::new(driver, config, &mut resources.resources, seed)
        };

        stacks.push(stack);

        // Spawn network stack runner task
        spawner.spawn(net_stack_task(node_id, runner)).unwrap();
    }

    info!("Network stacks created, waiting for configuration...");

    // Wait for all stacks to be configured
    for (i, stack) in stacks.iter().enumerate() {
        let node_id = (i + 1) as u8;

        // Wait for link up and configuration
        stack.wait_link_up().await;
        stack.wait_config_up().await;

        info!(
            "Node {} network configured: {:?}",
            node_id,
            stack.config_v4()
        );
    }

    info!("All network stacks ready!");

    // Receiver channels for UDP listeners (Incoming packets)
    static CHANNEL_1: transport::RaftChannel = transport::RaftChannel::new();
    static CHANNEL_2: transport::RaftChannel = transport::RaftChannel::new();
    static CHANNEL_3: transport::RaftChannel = transport::RaftChannel::new();
    static CHANNEL_4: transport::RaftChannel = transport::RaftChannel::new();
    static CHANNEL_5: transport::RaftChannel = transport::RaftChannel::new();

    // Sender channels for UDP senders (Outgoing packets)
    static OUT_CHANNEL_1: transport::RaftChannel = transport::RaftChannel::new();
    static OUT_CHANNEL_2: transport::RaftChannel = transport::RaftChannel::new();
    static OUT_CHANNEL_3: transport::RaftChannel = transport::RaftChannel::new();
    static OUT_CHANNEL_4: transport::RaftChannel = transport::RaftChannel::new();
    static OUT_CHANNEL_5: transport::RaftChannel = transport::RaftChannel::new();

    // Create UDP transports and spawn Raft nodes
    for (i, stack) in stacks.iter().enumerate() {
        let node_id = (i + 1) as u8;
        let node_id_u64 = node_id as u64;

        // Inbox (Listener -> Raft)
        let (inbox_sender, inbox_receiver) = match node_id {
            1 => (CHANNEL_1.sender(), CHANNEL_1.receiver()),
            2 => (CHANNEL_2.sender(), CHANNEL_2.receiver()),
            3 => (CHANNEL_3.sender(), CHANNEL_3.receiver()),
            4 => (CHANNEL_4.sender(), CHANNEL_4.receiver()),
            5 => (CHANNEL_5.sender(), CHANNEL_5.receiver()),
            _ => unreachable!(),
        };

        // Outbox (Raft -> Sender)
        let (outbox_sender, outbox_receiver) = match node_id {
            1 => (OUT_CHANNEL_1.sender(), OUT_CHANNEL_1.receiver()),
            2 => (OUT_CHANNEL_2.sender(), OUT_CHANNEL_2.receiver()),
            3 => (OUT_CHANNEL_3.sender(), OUT_CHANNEL_3.receiver()),
            4 => (OUT_CHANNEL_4.sender(), OUT_CHANNEL_4.receiver()),
            5 => (OUT_CHANNEL_5.sender(), OUT_CHANNEL_5.receiver()),
            _ => unreachable!(),
        };

        // Spawn persistent UDP listener (Feeds Inbox)
        spawner
            .spawn(udp_listener_task(node_id_u64, *stack, inbox_sender))
            .unwrap();

        // Spawn persistent UDP sender (Consumes Outbox)
        spawner
            .spawn(udp_sender_task(node_id_u64, *stack, outbox_receiver))
            .unwrap();

        // Stack is Copy, so we can pass it directly
        // UdpTransport now takes (node_id, outbound_sender, inbound_receiver)
        let transport_impl = UdpTransport::new(node_id_u64, outbox_sender, inbox_receiver);

        spawner
            .spawn(udp_raft_node_task(
                node_id_u64,
                transport_impl,
                cancel.clone(),
            ))
            .unwrap();

        info!("Spawned UDP node {}", node_id);
    }

    info!("All UDP nodes started!");
}

// Network stack task (runs embassy-net Runner)
#[embassy_executor::task(pool_size = 5)]
async fn net_stack_task(
    _node_id: u8,
    mut runner: embassy_net::Runner<'static, crate::transport::udp::driver::MockNetDriver>,
) {
    runner.run().await
}

// UDP Raft node task wrapper
#[embassy_executor::task(pool_size = 5)]
async fn udp_raft_node_task(
    node_id: u64,
    transport: crate::transport::udp::transport::UdpTransport,
    cancel: CancellationToken,
) {
    crate::embassy_node::raft_node_task_impl(node_id, transport, cancel).await
}

// UDP Listener Task Wrapper
#[embassy_executor::task(pool_size = 5)]
async fn udp_listener_task(
    node_id: u64,
    stack: embassy_net::Stack<'static>,
    sender: crate::transport::udp::transport::RaftSender,
) {
    if node_id > 255 {
        info!("Invalid node_id for listener: {}", node_id);
        return;
    }
    crate::transport::udp::transport::run_udp_listener(node_id, stack, sender).await
}

// UDP Sender Task Wrapper
#[embassy_executor::task(pool_size = 5)]
async fn udp_sender_task(
    node_id: u64,
    stack: embassy_net::Stack<'static>,
    receiver: crate::transport::udp::transport::RaftReceiver,
) {
    if node_id > 255 {
        info!("Invalid node_id for sender: {}", node_id);
        return;
    }
    crate::transport::udp::transport::run_udp_sender(node_id, stack, receiver).await
}
