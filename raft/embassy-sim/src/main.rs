// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

#![no_std]
#![no_main]

extern crate alloc;

use embassy_executor::Spawner;
use embassy_time::Duration;
use panic_semihosting as _;

#[macro_use]
mod logging;
mod cancellation_token;
mod embassy_log_collection;
mod embassy_map_collection;
mod embassy_node;
mod embassy_node_collection;
mod embassy_state_machine;
mod embassy_storage;
mod embassy_timer;
mod heap;
mod led_state;
mod time_driver;
mod transport;

#[cfg(feature = "udp-transport")]
mod net_config;
#[cfg(feature = "udp-transport")]
mod net_driver;

use cancellation_token::CancellationToken;

#[cfg(feature = "channel-transport")]
use embassy_node::raft_node_task;

#[cfg(feature = "channel-transport")]
use transport::channel::ChannelTransportHub;

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    heap::init_heap();

    // Initialize Time Driver (SysTick) for QEMU
    let mut p = cortex_m::Peripherals::take().unwrap();
    time_driver::init(&mut p.SYST);

    info!("Starting 5-node Raft cluster in Embassy");
    info!("Runtime: 30 seconds");

    let cancel = CancellationToken::new();

    #[cfg(feature = "udp-transport")]
    {
        use net_config::get_node_config;
        use net_driver::{MockNetDriver, NetworkBus};
        use transport::udp::UdpTransport;

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

                // Stack is Copy, so we can pass it directly
                let transport = UdpTransport::new(node_id, stack);

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

    info!("All nodes started. Observing consensus...");

    // Run for 30 seconds
    embassy_time::Timer::after(Duration::from_secs(30)).await;

    info!("30 seconds elapsed. Initiating graceful shutdown...");
    cancel.cancel();

    // Give tasks time to finish
    embassy_time::Timer::after(Duration::from_millis(500)).await;

    info!("Shutdown complete. Exiting.");
    cortex_m_semihosting::debug::exit(cortex_m_semihosting::debug::EXIT_SUCCESS);
}

// Network stack task (runs embassy-net Runner)
#[cfg(feature = "udp-transport")]
#[embassy_executor::task(pool_size = 5)]
async fn net_stack_task(
    _node_id: u8,
    mut runner: embassy_net::Runner<'static, net_driver::MockNetDriver>,
) {
    runner.run().await
}

// UDP Raft node task wrapper
#[cfg(feature = "udp-transport")]
#[embassy_executor::task(pool_size = 5)]
async fn udp_raft_node_task(
    node_id: u64,
    transport: transport::udp::UdpTransport,
    cancel: CancellationToken,
) {
    embassy_node::raft_node_task_impl(node_id, transport, cancel).await
}
