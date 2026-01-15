// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::{
    log_entry_collection::LogEntryCollection,
    map_collection::MapCollection,
    node_state::NodeState,
    raft_messages::RaftMsg,
    state_machine::StateMachine,
    storage::Storage,
    types::{LogIndex, NodeId, Term},
};

/// Manages log replication: AppendEntries, commit index, state machine application
pub struct LogReplicationManager<M>
where
    M: MapCollection,
{
    next_index: M,
    match_index: M,
    commit_index: LogIndex,
    last_applied: LogIndex,
}

impl<M> LogReplicationManager<M>
where
    M: MapCollection,
{
    pub fn new() -> Self {
        Self {
            next_index: M::new(),
            match_index: M::new(),
            commit_index: 0,
            last_applied: 0,
        }
    }

    /// Initialize follower tracking when becoming leader
    pub fn initialize_leader_state<P, L, S, I>(&mut self, peers: I, storage: &S)
    where
        S: Storage<Payload = P, LogEntryCollection = L>,
        I: Iterator<Item = NodeId>,
    {
        let next_log_index = storage.last_log_index() + 1;
        for peer in peers {
            self.next_index.insert(peer, next_log_index);
            self.match_index.insert(peer, 0);
        }
    }

    /// Get entries to send to a follower
    pub fn get_append_entries_for_follower<P, L, S>(
        &self,
        peer: NodeId,
        current_term: Term,
        storage: &S,
    ) -> RaftMsg<P, L>
    where
        P: Clone,
        S: Storage<Payload = P, LogEntryCollection = L> + Clone,
        L: LogEntryCollection<Payload = P> + Clone,
    {
        let next_idx = self.next_index.get(peer).unwrap_or(1);
        let prev_log_index = next_idx.saturating_sub(1);
        let prev_log_term = if prev_log_index == 0 {
            0
        } else {
            storage
                .get_entry(prev_log_index)
                .map(|e| e.term)
                .unwrap_or(0)
        };

        let entries = storage.get_entries(next_idx, storage.last_log_index() + 1);

        RaftMsg::AppendEntries {
            term: current_term,
            prev_log_index,
            prev_log_term,
            entries,
            leader_commit: self.commit_index,
        }
    }

    /// Handle incoming AppendEntries - returns response message
    #[allow(clippy::too_many_arguments)]
    pub fn handle_append_entries<P, L, S, SM>(
        &mut self,
        term: Term,
        prev_log_index: LogIndex,
        prev_log_term: Term,
        entries: L,
        leader_commit: LogIndex,
        current_term: &mut Term,
        storage: &mut S,
        state_machine: &mut SM,
        role: &mut NodeState,
    ) -> RaftMsg<P, L>
    where
        P: Clone,
        S: Storage<Payload = P, LogEntryCollection = L> + Clone,
        SM: StateMachine<Payload = P>,
        L: LogEntryCollection<Payload = P> + Clone,
    {
        // Update term if necessary
        if term > *current_term {
            *current_term = term;
            storage.set_current_term(term);
            *role = NodeState::Follower;
            storage.set_voted_for(None);
        }

        let success = if term < *current_term {
            false
        } else {
            self.try_append_entries(
                prev_log_index,
                prev_log_term,
                entries,
                leader_commit,
                storage,
                state_machine,
            )
        };

        RaftMsg::AppendEntriesResponse {
            term: *current_term,
            success,
            match_index: storage.last_log_index(),
        }
    }

    /// Handle AppendEntries response from follower
    pub fn handle_append_entries_response<P, L, S, SM>(
        &mut self,
        from: NodeId,
        success: bool,
        match_index: LogIndex,
        storage: &S,
        state_machine: &mut SM,
    ) where
        S: Storage<Payload = P, LogEntryCollection = L>,
        SM: StateMachine<Payload = P>,
    {
        if success {
            // Only update if match_index actually advanced
            let current_match = self.match_index.get(from).unwrap_or(0);
            if match_index > current_match {
                self.match_index.insert(from, match_index);
                self.next_index.insert(from, match_index + 1);
                self.advance_commit_index(storage, state_machine);
            }
        } else {
            // Decrement next_index on failure
            let next = self.next_index.get(from).unwrap_or(1);
            if next > 1 {
                self.next_index.insert(from, next - 1);
            }
        }
    }

    fn try_append_entries<P, L, S, SM>(
        &mut self,
        prev_log_index: LogIndex,
        prev_log_term: Term,
        entries: L,
        leader_commit: LogIndex,
        storage: &mut S,
        state_machine: &mut SM,
    ) -> bool
    where
        S: Storage<Payload = P, LogEntryCollection = L>,
        SM: StateMachine<Payload = P>,
        L: LogEntryCollection<Payload = P>,
    {
        // Check log consistency
        if !self.check_log_consistency(prev_log_index, prev_log_term, storage) {
            return false;
        }

        // Append entries if any
        if !entries.is_empty() {
            let last_index = storage.last_log_index();
            if last_index > prev_log_index {
                storage.truncate_after(prev_log_index);
            }
            storage.append_entries(entries.as_slice());
        }

        // Update commit index
        if leader_commit > self.commit_index {
            self.commit_index = leader_commit.min(storage.last_log_index());
            self.apply_committed_entries(storage, state_machine);
        }

        true
    }

    fn check_log_consistency<P, L, S>(
        &self,
        prev_log_index: LogIndex,
        prev_log_term: Term,
        storage: &S,
    ) -> bool
    where
        S: Storage<Payload = P, LogEntryCollection = L>,
    {
        if prev_log_index == 0 {
            true
        } else {
            storage.get_entry(prev_log_index).map(|e| e.term) == Some(prev_log_term)
        }
    }

    fn apply_committed_entries<P, L, S, SM>(&mut self, storage: &S, state_machine: &mut SM)
    where
        S: Storage<Payload = P, LogEntryCollection = L>,
        SM: StateMachine<Payload = P>,
    {
        while self.last_applied < self.commit_index {
            self.last_applied += 1;
            if let Some(entry) = storage.get_entry(self.last_applied) {
                state_machine.apply(&entry.payload);
            }
        }
    }

    fn advance_commit_index<P, L, S, SM>(&mut self, storage: &S, state_machine: &mut SM)
    where
        S: Storage<Payload = P, LogEntryCollection = L>,
        SM: StateMachine<Payload = P>,
    {
        let leader_index = storage.last_log_index();
        let total_peers = self.match_index.len();

        if let Some(new_commit) = self.match_index.compute_median(leader_index, total_peers) {
            if new_commit > self.commit_index {
                if let Some(entry) = storage.get_entry(new_commit) {
                    if entry.term == storage.current_term() {
                        self.commit_index = new_commit;
                        self.apply_committed_entries(storage, state_machine);
                    }
                }
            }
        }
    }

    pub fn commit_index(&self) -> LogIndex {
        self.commit_index
    }

    pub fn next_index(&self) -> &M {
        &self.next_index
    }

    pub fn match_index(&self) -> &M {
        &self.match_index
    }
}

impl<M: MapCollection> Default for LogReplicationManager<M> {
    fn default() -> Self {
        Self::new()
    }
}
