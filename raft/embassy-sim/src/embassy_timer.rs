// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use embassy_time::{Duration, Instant};
use raft_core::timer_service::{ExpiredTimers, TimerKind, TimerService};

const ELECTION_TIMEOUT_MIN_MS: u64 = 300;
const ELECTION_TIMEOUT_MAX_MS: u64 = 600;
const HEARTBEAT_TIMEOUT_MS: u64 = 100;

/// Embassy-based timer implementation for Raft
pub struct EmbassyTimer {
    election_deadline: Option<Instant>,
    heartbeat_deadline: Option<Instant>,
}

impl EmbassyTimer {
    pub fn new() -> Self {
        Self {
            election_deadline: None,
            heartbeat_deadline: None,
        }
    }

    /// Generate random election timeout between MIN and MAX
    fn random_election_timeout(&self) -> Duration {
        // Use embassy_time's timer tick as a simple entropy source
        let now_ticks = Instant::now().as_ticks();
        let range = ELECTION_TIMEOUT_MAX_MS - ELECTION_TIMEOUT_MIN_MS;
        let offset = (now_ticks as u64) % (range + 1);
        Duration::from_millis(ELECTION_TIMEOUT_MIN_MS + offset)
    }
}

impl TimerService for EmbassyTimer {
    fn reset_election_timer(&mut self) {
        self.election_deadline = Some(Instant::now() + self.random_election_timeout());
    }

    fn reset_heartbeat_timer(&mut self) {
        self.heartbeat_deadline =
            Some(Instant::now() + Duration::from_millis(HEARTBEAT_TIMEOUT_MS));
    }

    fn stop_timers(&mut self) {
        self.election_deadline = None;
        self.heartbeat_deadline = None;
    }

    fn check_expired(&self) -> ExpiredTimers {
        let mut expired = ExpiredTimers::new();
        let now = Instant::now();

        if let Some(deadline) = self.election_deadline {
            if now >= deadline {
                expired.push(TimerKind::Election);
            }
        }

        if let Some(deadline) = self.heartbeat_deadline {
            if now >= deadline {
                expired.push(TimerKind::Heartbeat);
            }
        }

        expired
    }
}

impl Default for EmbassyTimer {
    fn default() -> Self {
        Self::new()
    }
}
