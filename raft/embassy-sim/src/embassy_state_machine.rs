// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use alloc::string::String;
use heapless::index_map::FnvIndexMap;
use raft_core::state_machine::StateMachine;

/// Simple key-value state machine for Embassy
#[derive(Debug, Clone)]
pub struct EmbassyStateMachine {
    data: FnvIndexMap<String, String, 16>,
}

impl EmbassyStateMachine {
    pub fn new() -> Self {
        Self {
            data: FnvIndexMap::new(),
        }
    }
}

impl StateMachine for EmbassyStateMachine {
    type Payload = String;

    fn apply(&mut self, payload: &Self::Payload) {
        // Simple format: "key=value"
        if let Some((key, value)) = payload.split_once('=') {
            let _ = self.data.insert(String::from(key), String::from(value));
        }
    }

    fn get(&self, key: &str) -> Option<&str> {
        self.data.get(key).map(|s| s.as_str())
    }
}

impl Default for EmbassyStateMachine {
    fn default() -> Self {
        Self::new()
    }
}
