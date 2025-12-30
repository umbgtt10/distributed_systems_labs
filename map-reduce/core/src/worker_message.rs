// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use serde::{Deserialize, Serialize};

/// Message types received by workers
#[derive(Serialize, Deserialize, Debug)]
pub enum WorkerMessage<A, C> {
    /// Initialization message containing the synchronization sender
    Initialize(C),
    /// Work assignment
    Work(A, C),
}

