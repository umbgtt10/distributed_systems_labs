// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use alloc::string::String;
use heapless::index_map::FnvIndexMap;
use heapless::Vec;
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

/// Snapshot data for embassy - serialized key-value pairs
/// Format: "key1=value1\nkey2=value2\n..."
#[derive(Clone, Debug)]
pub struct EmbassySnapshotData {
    pub(crate) data: Vec<u8, 512>, // Max 512 bytes for serialized data
}

impl raft_core::snapshot::SnapshotData for EmbassySnapshotData {
    type Chunk = Vec<u8, 512>;

    fn len(&self) -> usize {
        self.data.len()
    }

    fn chunk_at(&self, offset: usize, max_size: usize) -> Option<Self::Chunk> {
        if offset >= self.data.len() {
            return None;
        }

        let end = (offset + max_size).min(self.data.len());
        let mut chunk = Vec::new();

        for i in offset..end {
            let _ = chunk.push(self.data[i]);
        }

        Some(chunk)
    }
}

impl StateMachine for EmbassyStateMachine {
    type Payload = String;
    type SnapshotData = EmbassySnapshotData;

    fn apply(&mut self, payload: &Self::Payload) {
        // Simple format: "key=value"
        if let Some((key, value)) = payload.split_once('=') {
            let _ = self.data.insert(String::from(key), String::from(value));
        }
    }

    fn get(&self, key: &str) -> Option<&str> {
        self.data.get(key).map(|s| s.as_str())
    }

    // === Snapshot Methods ===

    fn create_snapshot(&self) -> Self::SnapshotData {
        // Serialize state to "key=value\n" format
        let mut data = Vec::new();

        for (key, value) in self.data.iter() {
            // Append "key=value\n"
            for byte in key.as_bytes() {
                let _ = data.push(*byte);
            }
            let _ = data.push(b'=');
            for byte in value.as_bytes() {
                let _ = data.push(*byte);
            }
            let _ = data.push(b'\n');
        }

        EmbassySnapshotData { data }
    }

    fn restore_from_snapshot(
        &mut self,
        snapshot_data: &Self::SnapshotData,
    ) -> Result<(), raft_core::snapshot::SnapshotError> {
        // Clear existing data
        self.data.clear();

        // Parse snapshot data
        let data_str = core::str::from_utf8(&snapshot_data.data)
            .map_err(|_| raft_core::snapshot::SnapshotError::DeserializationFailed)?;

        for line in data_str.lines() {
            if let Some((key, value)) = line.split_once('=') {
                let _ = self.data.insert(String::from(key), String::from(value));
            }
        }

        Ok(())
    }
}

impl Default for EmbassyStateMachine {
    fn default() -> Self {
        Self::new()
    }
}
