// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use map_reduce_core::shutdown_signal::ShutdownSignal;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
pub struct DummyShutdownSignal;

impl ShutdownSignal for DummyShutdownSignal {
    fn is_cancelled(&self) -> bool {
        false
    }
}

