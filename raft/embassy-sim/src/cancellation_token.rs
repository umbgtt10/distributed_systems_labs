// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::signal::Signal;

#[derive(Clone)]
pub struct CancellationToken {
    signal: &'static Signal<CriticalSectionRawMutex, ()>,
}

impl CancellationToken {
    pub fn new() -> Self {
        static SIGNAL: Signal<CriticalSectionRawMutex, ()> = Signal::new();
        Self { signal: &SIGNAL }
    }

    /// Cancel all tasks waiting on this token
    pub fn cancel(&self) {
        self.signal.signal(());
    }

    /// Wait until cancellation is requested
    pub async fn cancelled(&self) {
        self.signal.wait().await;
    }

    /// Check if cancellation has been requested (non-blocking)
    pub fn is_cancelled(&self) -> bool {
        self.signal.signaled()
    }
}
