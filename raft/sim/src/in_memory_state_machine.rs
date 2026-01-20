// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::snapshot_types::SimSnapshotData;
use raft_core::snapshot::SnapshotError;
use raft_core::state_machine::StateMachine;
use std::collections::HashMap;

#[derive(Clone)]
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
    type SnapshotData = SimSnapshotData;

    fn apply(&mut self, payload: &Self::Payload) {
        // Parse "SET key=value" commands
        if let Some(command) = payload.strip_prefix("SET ") {
            if let Some((key, value)) = command.split_once('=') {
                self.data.insert(key.to_string(), value.to_string());
            }
        } else {
            self.data.insert(payload.to_string(), payload.to_string());
        }
    }

    fn get(&self, key: &str) -> Option<&str> {
        self.data.get(key).map(|s| s.as_str())
    }

    fn create_snapshot(&self) -> Self::SnapshotData {
        // Serialize HashMap to JSON (simple and readable for testing)
        let vec = serde_json::to_vec(&self.data).unwrap_or_default();
        SimSnapshotData::from_vec(vec)
    }

    fn restore_from_snapshot(&mut self, data: &Self::SnapshotData) -> Result<(), SnapshotError> {
        self.data = serde_json::from_slice(data.as_slice())
            .map_err(|_| SnapshotError::DeserializationFailed)?;
        Ok(())
    }
}
