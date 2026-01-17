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
pub mod embassy_log_collection;
pub mod transport;

// Re-export for convenience
pub use embassy_log_collection::EmbassyLogEntryCollection;
