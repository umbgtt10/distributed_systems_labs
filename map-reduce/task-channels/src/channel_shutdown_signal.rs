// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use map_reduce_core::shutdown_signal::ShutdownSignal;
use tokio_util::sync::CancellationToken;

/// Tokio CancellationToken-based shutdown signal
#[derive(Clone)]
pub struct ChannelShutdownSignal {
    token: CancellationToken,
}

impl ChannelShutdownSignal {
    pub fn new(token: CancellationToken) -> Self {
        Self { token }
    }
}

impl ShutdownSignal for ChannelShutdownSignal {
    fn is_cancelled(&self) -> bool {
        self.token.is_cancelled()
    }
}

