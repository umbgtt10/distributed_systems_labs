// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use std::collections::HashMap;

use raft_core::state_machine::StateMachine;

pub struct InMemoryStateMachine {
    data: HashMap<String, String>,
}

impl InMemoryStateMachine {
    pub fn new() -> Self {
        Self {
            data: HashMap::new(),
        }
    }
}

impl Default for InMemoryStateMachine {
    fn default() -> Self {
        Self::new()
    }
}

impl StateMachine for InMemoryStateMachine {
    type Payload = String;

    fn apply(&mut self, payload: &Self::Payload) {
        // Parse "SET key=value" commands
        if let Some(command) = payload.strip_prefix("SET ") {
            if let Some((key, value)) = command.split_once('=') {
                self.data.insert(key.to_string(), value.to_string());
            }
        }
    }

    fn get(&self, key: &str) -> Option<&str> {
        self.data.get(key).map(|s| s.as_str())
    }
}
