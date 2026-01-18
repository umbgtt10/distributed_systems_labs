// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::cancellation_token::CancellationToken;
use crate::info;
use embassy_executor::Spawner; // Macros

#[cfg(feature = "channel-transport")]
use crate::embassy_node::raft_node_task;
#[cfg(feature = "channel-transport")]
use crate::transport::channel::ChannelTransportHub;

#[cfg(feature = "udp-transport")]
use crate::transport::net_config::{self, get_node_config};
#[cfg(feature = "udp-transport")]
use crate::transport::net_driver::{self, MockNetDriver, NetworkBus};
#[cfg(feature = "udp-transport")]
use crate::transport::udp::{self, UdpTransport};

pub async fn initialize_cluster(spawner: Spawner, cancel: CancellationToken) {
    #[cfg(feature = "udp-transport")]
    {
        info!("Using UDP transport (simulated Ethernet)");
        info!("WireRaftMsg serialization layer: COMPLETE ✓");

        // Create shared network bus for all nodes
        static NETWORK_BUS: NetworkBus = NetworkBus::new();

        // Storage for network stacks (one per node)
        static mut STACK_1: Option<embassy_net::Stack<'static>> = None;
        static mut STACK_2: Option<embassy_net::Stack<'static>> = None;
        static mut STACK_3: Option<embassy_net::Stack<'static>> = None;
        static mut STACK_4: Option<embassy_net::Stack<'static>> = None;
        static mut STACK_5: Option<embassy_net::Stack<'static>> = None;

        // Create network stacks for all 5 nodes
        for node_id in 1..=5 {
            unsafe {
                let driver = MockNetDriver::new(node_id, &NETWORK_BUS);
                let config = get_node_config(node_id);
                let resources = net_config::get_node_resources(node_id);
                let seed = 0x0123_4567_89AB_CDEF_u64 + node_id as u64;

                let (stack, runner) =
                    embassy_net::new(driver, config, &mut resources.resources, seed);

                // Store stack in static storage
                match node_id {
                    1 => STACK_1 = Some(stack),
                    2 => STACK_2 = Some(stack),
                    3 => STACK_3 = Some(stack),
                    4 => STACK_4 = Some(stack),
                    5 => STACK_5 = Some(stack),
                    _ => unreachable!(),
                }

                // Spawn network stack runner task
                spawner.spawn(net_stack_task(node_id, runner)).unwrap();
            }
        }

        info!("Network stacks created, waiting for configuration...");

        // Wait for all stacks to be configured
        unsafe {
            for node_id in 1..=5 {
                let stack = match node_id {
                    1 => STACK_1.as_ref().unwrap(),
                    2 => STACK_2.as_ref().unwrap(),
                    3 => STACK_3.as_ref().unwrap(),
                    4 => STACK_4.as_ref().unwrap(),
                    5 => STACK_5.as_ref().unwrap(),
                    _ => unreachable!(),
                };

                // Wait for link up and configuration
                stack.wait_link_up().await;
                stack.wait_config_up().await;

                info!(
                    "Node {} network configured: {:?}",
                    node_id,
                    stack.config_v4()
                );
            }
        }

        info!("All network stacks ready!");

        // Receiver channels for UDP listeners (Incoming packets)
        static CHANNEL_1: udp::RaftChannel = udp::RaftChannel::new();
        static CHANNEL_2: udp::RaftChannel = udp::RaftChannel::new();
        static CHANNEL_3: udp::RaftChannel = udp::RaftChannel::new();
        static CHANNEL_4: udp::RaftChannel = udp::RaftChannel::new();
        static CHANNEL_5: udp::RaftChannel = udp::RaftChannel::new();

        // Sender channels for UDP senders (Outgoing packets)
        static OUT_CHANNEL_1: udp::RaftChannel = udp::RaftChannel::new();
        static OUT_CHANNEL_2: udp::RaftChannel = udp::RaftChannel::new();
        static OUT_CHANNEL_3: udp::RaftChannel = udp::RaftChannel::new();
        static OUT_CHANNEL_4: udp::RaftChannel = udp::RaftChannel::new();
        static OUT_CHANNEL_5: udp::RaftChannel = udp::RaftChannel::new();

        // Create UDP transports and spawn Raft nodes
        unsafe {
            for node_id in 1..=5 {
                let stack = match node_id {
                    1 => STACK_1.as_ref().unwrap(),
                    2 => STACK_2.as_ref().unwrap(),
                    3 => STACK_3.as_ref().unwrap(),
                    4 => STACK_4.as_ref().unwrap(),
                    5 => STACK_5.as_ref().unwrap(),
                    _ => unreachable!(),
                };

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
                    .spawn(udp_listener_task(node_id, *stack, inbox_sender))
                    .unwrap();

                // Spawn persistent UDP sender (Consumes Outbox)
                spawner
                    .spawn(udp_sender_task(node_id, *stack, outbox_receiver))
                    .unwrap();

                // Stack is Copy, so we can pass it directly
                // UdpTransport now takes (node_id, outbound_sender, inbound_receiver)
                let transport = UdpTransport::new(node_id, outbox_sender, inbox_receiver);

                spawner
                    .spawn(udp_raft_node_task(node_id, transport, cancel.clone()))
                    .unwrap();

                info!("Spawned UDP node {}", node_id);
            }
        }

        info!("All UDP nodes started!");
    }

    #[cfg(not(feature = "udp-transport"))]
    {
        info!("Using channel-based transport (in-memory)");

        // Create transport hub (manages all inter-node channels)
        let transport_hub = ChannelTransportHub::new();

        // Spawn 5 Raft node tasks
        for node_id in 1..=5 {
            let transport = transport_hub.create_transport(node_id);
            spawner
                .spawn(raft_node_task(node_id, transport, cancel.clone()))
                .unwrap();
            info!("Spawned node {}", node_id);
        }

        info!("All Raft nodes spawned with channel transport!");
    }
}

// Network stack task (runs embassy-net Runner)
#[cfg(feature = "udp-transport")]
#[embassy_executor::task(pool_size = 5)]
async fn net_stack_task(
    _node_id: u8,
    mut runner: embassy_net::Runner<'static, crate::transport::net_driver::MockNetDriver>,
) {
    runner.run().await
}

// UDP Raft node task wrapper
#[cfg(feature = "udp-transport")]
#[embassy_executor::task(pool_size = 5)]
async fn udp_raft_node_task(
    node_id: u64,
    transport: crate::transport::udp::UdpTransport,
    cancel: CancellationToken,
) {
    crate::embassy_node::raft_node_task_impl(node_id, transport, cancel).await
}

// UDP Listener Task Wrapper
#[cfg(feature = "udp-transport")]
#[embassy_executor::task(pool_size = 5)]
async fn udp_listener_task(
    node_id: u64,
    stack: embassy_net::Stack<'static>,
    sender: crate::transport::udp::RaftSender,
) {
    if node_id > 255 {
        info!("Invalid node_id for listener: {}", node_id);
        return;
    }
    crate::transport::udp::run_udp_listener(node_id, stack, sender).await
}

// UDP Sender Task Wrapper
#[cfg(feature = "udp-transport")]
#[embassy_executor::task(pool_size = 5)]
async fn udp_sender_task(
    node_id: u64,
    stack: embassy_net::Stack<'static>,
    receiver: crate::transport::udp::RaftReceiver,
) {
    if node_id > 255 {
        info!("Invalid node_id for sender: {}", node_id);
        return;
    }
    crate::transport::udp::run_udp_sender(node_id, stack, receiver).await
}
