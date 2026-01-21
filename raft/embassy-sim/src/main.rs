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
pub mod logging;
pub mod cancellation_token;
pub mod cluster;
pub mod collections;
pub mod config;
pub mod embassy_node;
pub mod embassy_observer;
pub mod embassy_state_machine;
pub mod embassy_storage;
pub mod embassy_timer;
pub mod heap;
pub mod led_state;
pub mod time_driver;
pub mod transport;

use cancellation_token::CancellationToken;

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    heap::init_heap();

    // Initialize Time Driver (SysTick) for QEMU
    let mut p = cortex_m::Peripherals::take().unwrap();
    time_driver::init(&mut p.SYST);

    info!("Starting 5-node Raft cluster in Embassy");
    info!("Runtime: 30 seconds");

    let cancel = CancellationToken::default();

    // Read observer level from config
    let observer_level = config::get_observer_level();
    info!("Observer level: {:?}", observer_level);

    // Initialize cluster (handles all network/channel setup internally)
    let cluster =
        transport::setup::initialize_cluster(spawner, cancel.clone(), observer_level).await;

    info!("All nodes started. Waiting for leader election...");

    // Wait for consensus (leader election)
    match cluster
        .wait_for_leader(embassy_time::Duration::from_secs(10))
        .await
    {
        Ok(leader_id) => {
            info!("Consensus achieved! Leader elected: Node {}", leader_id);
        }
        Err(_) => {
            panic!("Leader election timed out!");
        }
    }

    // Submit test commands
    info!("Submitting test commands...");
    for i in 1..=3 {
        let command = alloc::format!("SET key{} value{}", i, i);
        info!("Submitting: {}", command);
        match cluster.submit_command(command).await {
            Ok(_) => info!("Command {} committed successfully!", i),
            Err(e) => info!("Command {} failed: {:?}", i, e),
        }
    }
    info!("All commands processed!");

    // Run for additional time to observe replication
    embassy_time::Timer::after(Duration::from_secs(1)).await;

    info!("1 seconds elapsed. Initiating graceful shutdown...");
    cluster.shutdown();

    // Give tasks time to finish
    embassy_time::Timer::after(Duration::from_millis(500)).await;

    info!("Shutdown complete. Exiting.");
    cortex_m_semihosting::debug::exit(cortex_m_semihosting::debug::EXIT_SUCCESS);
}
