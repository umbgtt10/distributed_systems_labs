// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use raft_core::timer_service::{ExpiredTimers, TimerKind, TimerService};
use std::cell::RefCell;
use std::rc::Rc;

/// Shared mock clock that can be advanced manually
#[derive(Clone)]
pub struct MockClock {
    current_time: Rc<RefCell<u64>>, // milliseconds
}

impl MockClock {
    pub fn new() -> Self {
        Self {
            current_time: Rc::new(RefCell::new(0)),
        }
    }

    pub fn now(&self) -> u64 {
        *self.current_time.borrow()
    }

    pub fn advance(&self, millis: u64) {
        *self.current_time.borrow_mut() += millis;
    }
}

impl Default for MockClock {
    fn default() -> Self {
        Self::new()
    }
}

/// Mock timer service with controllable time
pub struct MockTimerService {
    election_deadline: Option<u64>,
    heartbeat_deadline: Option<u64>,
    election_timeout_min: u64,
    election_timeout_max: u64,
    heartbeat_interval: u64,
    clock: MockClock,
    node_id: u64,
}

impl MockTimerService {
    pub fn new(
        election_timeout_min: u64,
        election_timeout_max: u64,
        heartbeat_interval: u64,
        clock: MockClock,
        node_id: u64,
    ) -> Self {
        let mut service = Self {
            election_deadline: None,
            heartbeat_deadline: None,
            election_timeout_min,
            election_timeout_max,
            heartbeat_interval,
            clock,
            node_id,
        };
        service.reset_election_timer();
        service
    }

    /// Get deterministic "random" timeout
    fn random_election_timeout(&self) -> u64 {
        let range = self.election_timeout_max - self.election_timeout_min;

        // If min == max, just return that value (no randomness needed)
        if range == 0 {
            return self.election_timeout_min;
        }

        // Use a better mixing function for deterministic randomness
        let seed = self
            .clock
            .now()
            .wrapping_mul(31)
            .wrapping_add(self.node_id.wrapping_mul(97));

        // Mix the bits more
        let seed = seed ^ (seed >> 16);
        let seed = seed.wrapping_mul(0x85ebca6b);
        let seed = seed ^ (seed >> 13);

        self.election_timeout_min + (seed % range)
    }
}

impl TimerService for MockTimerService {
    fn reset_election_timer(&mut self) {
        let timeout = self.random_election_timeout();
        self.election_deadline = Some(self.clock.now() + timeout);
    }

    fn reset_heartbeat_timer(&mut self) {
        self.heartbeat_deadline = Some(self.clock.now() + self.heartbeat_interval);
    }

    fn stop_timers(&mut self) {
        self.election_deadline = None;
        self.heartbeat_deadline = None;
    }

    fn check_expired(&self) -> ExpiredTimers {
        let mut expired = ExpiredTimers::new();
        let now = self.clock.now();

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
