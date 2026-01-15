// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::{
    log_entry::LogEntry,
    log_entry_collection::LogEntryCollection,
    types::{LogIndex, NodeId, Term},
};

pub trait Storage {
    type Payload: Clone;
    type LogEntryCollection: LogEntryCollection<Payload = Self::Payload>;

    fn current_term(&self) -> Term;
    fn set_current_term(&mut self, term: Term);

    fn voted_for(&self) -> Option<NodeId>;
    fn set_voted_for(&mut self, vote: Option<NodeId>);

    fn last_log_index(&self) -> LogIndex;
    fn last_log_term(&self) -> Term;

    fn get_entry(&self, index: LogIndex) -> Option<LogEntry<Self::Payload>>;
    fn get_entries(&self, start: LogIndex, end: LogIndex) -> Self::LogEntryCollection;
    fn append_entries(&mut self, entries: &[LogEntry<Self::Payload>]);
}
