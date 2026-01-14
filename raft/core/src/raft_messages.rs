// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::{
    log_entry::LogEntry,
    types::{LogIndex, NodeId, Term, MAX_ENTRIES},
};

#[derive(Clone, Debug, PartialEq)]
pub enum RaftMsg<P> {
    RequestVote {
        term: Term,
        candidate_id: NodeId,
        last_log_index: LogIndex,
        last_log_term: Term,
    },
    RequestVoteResponse {
        term: Term,
        vote_granted: bool,
    },
    AppendEntries {
        term: Term,
        leader_id: NodeId,
        prev_log_index: LogIndex,
        prev_log_term: Term,
        entries: heapless::Vec<LogEntry<P>, MAX_ENTRIES>,
        leader_commit: LogIndex,
    },
    AppendEntriesResponse {
        term: Term,
        success: bool,
        match_index: LogIndex,
    },
}
