use raft_core::{
    log_entry::LogEntry,
    storage::Storage,
    types::{LogIndex, NodeId, Term},
};

pub struct InMemoryStorage {
    current_term: Term,
    voted_for: Option<NodeId>,
    log: Vec<LogEntry<String>>,
}

impl InMemoryStorage {
    pub fn new() -> Self {
        Self {
            current_term: 0,
            voted_for: None,
            log: Vec::new(),
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
        if let Some(last_entry) = self.log.last() {
            last_entry.term
        } else {
            0
        }
    }

    fn get_entry(&self, index: LogIndex) -> Option<LogEntry<String>> {
        self.log.get(index as usize).cloned()
    }

    fn append_entries(&mut self, entries: &[LogEntry<String>]) {
        self.log.extend_from_slice(entries);
    }

    fn truncate_suffix(&mut self, from: LogIndex) {
        self.log.truncate(from as usize);
    }
}
