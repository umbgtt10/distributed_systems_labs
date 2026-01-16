// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use alloc::string::String;
use heapless::Vec;
use raft_core::log_entry::LogEntry;
use raft_core::types::{LogIndex, NodeId, Term};

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

    pub fn current_term(&self) -> Term {
        self.current_term
    }

    pub fn set_current_term(&mut self, term: Term) {
        self.current_term = term;
    }

    pub fn voted_for(&self) -> Option<NodeId> {
        self.voted_for
    }

    pub fn set_voted_for(&mut self, voted_for: Option<NodeId>) {
        self.voted_for = voted_for;
    }

    pub fn last_log_index(&self) -> LogIndex {
        self.log.len() as LogIndex
    }

    pub fn last_log_term(&self) -> Term {
        self.log.last().map(|entry| entry.term).unwrap_or(0)
    }

    pub fn append_entry(&mut self, entry: LogEntry<alloc::string::String>) -> Result<(), ()> {
        self.log.push(entry).map_err(|_| ())
    }

    pub fn get_entry(&self, index: LogIndex) -> Option<&LogEntry<alloc::string::String>> {
        if index == 0 || index > self.log.len() as LogIndex {
            None
        } else {
            self.log.get((index - 1) as usize)
        }
    }

    pub fn truncate_after(&mut self, index: LogIndex) {
        self.log.truncate(index as usize);
    }
}

impl Default for EmbassyStorage {
    fn default() -> Self {
        Self::new()
    }
}

// TODO: Implement raft_core::storage::Storage trait when integrating
