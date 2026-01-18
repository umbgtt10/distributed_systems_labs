// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use alloc::string::String;
use heapless::Vec;
use raft_core::log_entry::LogEntry;
use raft_core::log_entry_collection::LogEntryCollection;
use raft_core::storage::Storage;
use raft_core::types::{LogIndex, NodeId, Term};

use crate::embassy_log_collection::EmbassyLogEntryCollection;

/// Simple in-memory storage for Embassy Raft nodes
/// In a real system, this would persist to flash
#[derive(Clone)]
pub struct EmbassyStorage {
    current_term: Term,
    voted_for: Option<NodeId>,
    log: Vec<LogEntry<String>, 256>, // Max 256 entries
}

impl EmbassyStorage {
    pub fn new() -> Self {
        Self {
            current_term: 0,
            voted_for: None,
            log: Vec::new(),
        }
    }
}

impl Storage for EmbassyStorage {
    type Payload = String;
    type LogEntryCollection = EmbassyLogEntryCollection;

    fn current_term(&self) -> Term {
        self.current_term
    }

    fn set_current_term(&mut self, term: Term) {
        self.current_term = term;
    }

    fn voted_for(&self) -> Option<NodeId> {
        self.voted_for
    }

    fn set_voted_for(&mut self, vote: Option<NodeId>) {
        self.voted_for = vote;
    }

    fn last_log_index(&self) -> LogIndex {
        self.log.len() as LogIndex
    }

    fn last_log_term(&self) -> Term {
        self.log.last().map(|entry| entry.term).unwrap_or(0)
    }

    fn get_entry(&self, index: LogIndex) -> Option<LogEntry<Self::Payload>> {
        if index == 0 || index > self.log.len() as LogIndex {
            None
        } else {
            self.log.get((index - 1) as usize).cloned()
        }
    }

    fn get_entries(&self, start: LogIndex, end: LogIndex) -> Self::LogEntryCollection {
        if start == 0 || start >= end {
            return EmbassyLogEntryCollection::new(&[]);
        }

        let start_idx = (start - 1) as usize;
        let end_idx = (end - 1) as usize;

        if start_idx >= self.log.len() {
            return EmbassyLogEntryCollection::new(&[]);
        }

        // Clamp to available entries to prevent out of bounds
        let actual_end_idx = end_idx.min(self.log.len());

        let slice = &self.log[start_idx..actual_end_idx];
        EmbassyLogEntryCollection::new(slice)
    }

    fn append_entries(&mut self, entries: &[LogEntry<Self::Payload>]) {
        for entry in entries {
            let _ = self.log.push(entry.clone());
        }
    }

    fn truncate_after(&mut self, index: LogIndex) {
        self.log.truncate(index as usize);
    }
}

impl Default for EmbassyStorage {
    fn default() -> Self {
        Self::new()
    }
}
