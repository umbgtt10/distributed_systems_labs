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
pub mod embassy_log_collection;
pub mod embassy_map_collection;
pub mod embassy_node;
pub mod embassy_node_collection;
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

    // Initialize cluster (handles all network/channel setup internally)
    transport::setup::initialize_cluster(spawner, cancel.clone()).await;

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
