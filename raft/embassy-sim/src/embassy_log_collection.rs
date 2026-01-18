// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use alloc::string::String;
use heapless::Vec;
use raft_core::{
    log_entry::LogEntry,
    log_entry_collection::{CollectionError, LogEntryCollection},
};

#[derive(Debug, Clone, PartialEq)]
pub struct EmbassyLogEntryCollection {
    entries: Vec<LogEntry<String>, 128>, // Max 128 entries
}

impl LogEntryCollection for EmbassyLogEntryCollection {
    type Payload = String;
    type Iter<'a> = core::slice::Iter<'a, LogEntry<String>>;

    fn new(entries: &[LogEntry<Self::Payload>]) -> Self {
        let mut vec = Vec::new();
        for entry in entries {
            vec.push(entry.clone()).ok(); // Silently ignore if capacity exceeded
        }
        Self { entries: vec }
    }

    fn clear(&mut self) {
        self.entries.clear();
    }

    fn push(&mut self, entry: LogEntry<Self::Payload>) -> Result<(), CollectionError> {
        self.entries.push(entry).map_err(|_| CollectionError::Full)
    }

    fn len(&self) -> usize {
        self.entries.len()
    }

    fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    fn iter(&self) -> Self::Iter<'_> {
        self.entries.iter()
    }

    fn as_slice(&self) -> &[LogEntry<Self::Payload>] {
        &self.entries
    }

    fn truncate(&mut self, len: usize) {
        self.entries.truncate(len);
    }
}
