// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! Embassy-sim library exports for testing
//!
//! This library target is primarily used to expose modules for integration tests.

#![no_std]

extern crate alloc;

#[macro_use]
pub mod logging;
pub mod cancellation_token;
pub mod cluster;
pub mod config;
pub mod embassy_log_collection;
pub mod embassy_map_collection;
pub mod embassy_node;
pub mod embassy_node_collection;
pub mod embassy_observer;
pub mod embassy_state_machine;
pub mod embassy_storage;
pub mod embassy_timer;
pub mod heap;
pub mod led_state;
pub mod time_driver;
pub mod transport;

// Re-export for convenience
pub use embassy_log_collection::EmbassyLogEntryCollection;
