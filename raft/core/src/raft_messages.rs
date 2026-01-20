// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::{
    chunk_collection::ChunkCollection,
    log_entry_collection::LogEntryCollection,
    types::{LogIndex, NodeId, Term},
};

#[derive(Clone, Debug, PartialEq)]
pub enum RaftMsg<P: Clone, L: LogEntryCollection<Payload = P> + Clone, C: ChunkCollection + Clone> {
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
        prev_log_index: LogIndex,
        prev_log_term: Term,
        entries: L,
        leader_commit: LogIndex,
    },
    AppendEntriesResponse {
        term: Term,
        success: bool,
        match_index: LogIndex,
    },
    InstallSnapshot {
        term: Term,
        leader_id: NodeId,
        last_included_index: LogIndex,
        last_included_term: Term,
        offset: u64,
        data: C,
        done: bool,
    },
    InstallSnapshotResponse {
        term: Term,
        success: bool,
    },
}
