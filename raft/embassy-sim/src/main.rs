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
mod embassy_node;
mod embassy_storage;
mod embassy_timer;
mod heap;
mod led_state;
mod time_driver;
mod transport;

use cancellation_token::CancellationToken;
use embassy_node::raft_node_task;
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

    // Create transport hub (manages all inter-node channels)
    let transport_hub = ChannelTransportHub::new();

    // Spawn 5 Raft node tasks
    for node_id in 1..=5 {
        let transport = transport_hub.create_transport(node_id);
        spawner
            .spawn(raft_node_task(node_id as u8, transport, cancel.clone()))
            .unwrap();
        info!("✅ Spawned node {}", node_id);
    }

    info!("All nodes started. Observing consensus...");

    spawner.spawn(heartbeat_task(cancel.clone())).unwrap();

    // Run for 30 seconds
    embassy_time::Timer::after(Duration::from_secs(30)).await;

    info!("30 seconds elapsed. Initiating graceful shutdown...");
    cancel.cancel();

    // Give tasks time to finish
    embassy_time::Timer::after(Duration::from_millis(500)).await;

    info!("Shutdown complete. Exiting.");
    cortex_m_semihosting::debug::exit(cortex_m_semihosting::debug::EXIT_SUCCESS);
}

#[embassy_executor::task]
async fn heartbeat_task(cancel: CancellationToken) {
    loop {
        embassy_time::Timer::after(Duration::from_secs(10)).await;

        if cancel.is_cancelled() {
            info!("💓 Heartbeat task shutting down");
            break;
        }

        info!("💓 Cluster heartbeat");
    }
}
