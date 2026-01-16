// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use embassy_time::{Duration, Instant, Timer};

/// Embassy-based timer implementation for Raft
pub struct EmbassyTimer {
    deadline: Option<Instant>,
}

impl EmbassyTimer {
    pub fn new() -> Self {
        Self { deadline: None }
    }

    /// Start a timeout
    pub fn start(&mut self, duration_ms: u64) {
        let duration = Duration::from_millis(duration_ms);
        self.deadline = Some(Instant::now() + duration);
    }

    /// Cancel the timeout
    pub fn cancel(&mut self) {
        self.deadline = None;
    }

    /// Check if timer has expired
    pub fn has_elapsed(&self) -> bool {
        if let Some(deadline) = self.deadline {
            Instant::now() >= deadline
        } else {
            false
        }
    }

    /// Wait until the timer expires
    pub async fn wait(&self) {
        if let Some(deadline) = self.deadline {
            Timer::at(deadline).await;
        } else {
            // No deadline set, wait indefinitely (shouldn't happen in practice)
            core::future::pending::<()>().await;
        }
    }
}

impl Default for EmbassyTimer {
    fn default() -> Self {
        Self::new()
    }
}

// TODO: Implement timer_traits::Timer trait when integrating with core
// impl timer_traits::Timer for EmbassyTimer {
//     fn start(&mut self, duration_ms: u64) { ... }
//     fn cancel(&mut self) { ... }
//     fn has_elapsed(&self) -> bool { ... }
// }
