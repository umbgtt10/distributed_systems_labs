// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use raft_core::{
    log_entry::LogEntry,
    log_entry_collection::{CollectionError, LogEntryCollection},
};

#[derive(Debug, Clone, PartialEq)]
pub struct InMemoryLogEntryCollection {
    entries: Vec<LogEntry<String>>,
}

impl LogEntryCollection for InMemoryLogEntryCollection {
    type Payload = String;
    type Iter<'a> = std::slice::Iter<'a, LogEntry<String>>;

    fn new(entries: &[LogEntry<String>]) -> Self {
        Self {
            entries: entries.to_vec(),
        }
    }

    fn clear(&mut self) {
        self.entries.clear();
    }

    fn push(&mut self, entry: LogEntry<Self::Payload>) -> Result<(), CollectionError> {
        self.entries.push(entry);
        Ok(())
    }

    fn len(&self) -> usize {
        self.entries.len()
    }

    fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    fn as_slice(&self) -> &[LogEntry<Self::Payload>] {
        &self.entries
    }

    fn iter(&self) -> Self::Iter<'_> {
        self.entries.iter()
    }

    fn truncate(&mut self, index: usize) {
        self.entries.truncate(index);
    }
}
