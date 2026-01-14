// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::in_memory_log_entry_collection::InMemoryLogEntryCollection;
use raft_core::{
    log_entry::LogEntry,
    log_entry_collection::LogEntryCollection,
    storage::Storage,
    types::{LogIndex, NodeId, Term},
};

pub struct InMemoryStorage {
    current_term: Term,
    voted_for: Option<NodeId>,
    log: InMemoryLogEntryCollection,
}

impl InMemoryStorage {
    pub fn new() -> Self {
        Self {
            current_term: 0,
            voted_for: None,
            log: InMemoryLogEntryCollection::new(&[]),
        }
    }
}

impl Default for InMemoryStorage {
    fn default() -> Self {
        Self::new()
    }
}

impl Storage for InMemoryStorage {
    type Payload = String;
    type LogEntryCollection = InMemoryLogEntryCollection;

    fn current_term(&self) -> Term {
        self.current_term
    }

    fn set_current_term(&mut self, term: Term) {
        self.current_term = term;
    }

    fn voted_for(&self) -> Option<NodeId> {
        self.voted_for
    }

    fn set_voted_for(&mut self, voted_for: Option<NodeId>) {
        self.voted_for = voted_for;
    }

    fn last_log_index(&self) -> LogIndex {
        if self.log.is_empty() {
            0
        } else {
            self.log.len() as LogIndex
        }
    }

    fn last_log_term(&self) -> Term {
        if let Some(last_entry) = self.log.as_slice().last() {
            last_entry.term
        } else {
            0
        }
    }

    fn get_entry(&self, index: LogIndex) -> Option<LogEntry<String>> {
        // Convert 1-based index to 0-based
        if index == 0 {
            None
        } else {
            self.log.as_slice().get((index - 1) as usize).cloned()
        }
    }

    fn get_entries(&self) -> InMemoryLogEntryCollection {
        let entries = self
            .log
            .as_slice()
            .get(0..self.log.len())
            .unwrap_or(&[])
            .to_vec();
        InMemoryLogEntryCollection::new(&entries)
    }

    fn append_entries(&mut self, entries: &[LogEntry<String>]) {
        for entry in entries {
            self.log.push(entry.clone()).unwrap();
        }
    }
}
