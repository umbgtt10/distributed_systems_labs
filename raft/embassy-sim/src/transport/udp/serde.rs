// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! Serializable wire format for RaftMsg
//!
//! Since raft-core's RaftMsg doesn't derive Serialize/Deserialize,
//! we create a mirror type that can be serialized with postcard.

use crate::embassy_log_collection::EmbassyLogEntryCollection;
use alloc::string::String;
use alloc::vec::Vec;
use raft_core::{log_entry::LogEntry, raft_messages::RaftMsg};
use serde::{Deserialize, Serialize};

/// Serializable log entry for wire protocol
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WireLogEntry {
    pub term: u64,
    pub payload: String,
}

impl From<LogEntry<String>> for WireLogEntry {
    fn from(entry: LogEntry<String>) -> Self {
        Self {
            term: entry.term,
            payload: entry.payload,
        }
    }
}

impl From<WireLogEntry> for LogEntry<String> {
    fn from(wire: WireLogEntry) -> Self {
        Self {
            term: wire.term,
            payload: wire.payload,
        }
    }
}

/// Serializable wire format for RaftMsg
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WireRaftMsg {
    RequestVote {
        term: u64,
        candidate_id: u64,
        last_log_index: u64,
        last_log_term: u64,
    },
    RequestVoteResponse {
        term: u64,
        vote_granted: bool,
    },
    AppendEntries {
        term: u64,
        prev_log_index: u64,
        prev_log_term: u64,
        entries: Vec<WireLogEntry>,
        leader_commit: u64,
    },
    AppendEntriesResponse {
        term: u64,
        success: bool,
        match_index: u64,
    },
}

impl From<RaftMsg<String, EmbassyLogEntryCollection>> for WireRaftMsg {
    fn from(msg: RaftMsg<String, EmbassyLogEntryCollection>) -> Self {
        match msg {
            RaftMsg::RequestVote {
                term,
                candidate_id,
                last_log_index,
                last_log_term,
            } => WireRaftMsg::RequestVote {
                term,
                candidate_id,
                last_log_index,
                last_log_term,
            },
            RaftMsg::RequestVoteResponse { term, vote_granted } => {
                WireRaftMsg::RequestVoteResponse { term, vote_granted }
            }
            RaftMsg::AppendEntries {
                term,
                prev_log_index,
                prev_log_term,
                entries,
                leader_commit,
            } => {
                use raft_core::log_entry_collection::LogEntryCollection;
                let wire_entries: Vec<WireLogEntry> = entries
                    .as_slice()
                    .iter()
                    .map(|entry| WireLogEntry::from(entry.clone()))
                    .collect();
                WireRaftMsg::AppendEntries {
                    term,
                    prev_log_index,
                    prev_log_term,
                    entries: wire_entries,
                    leader_commit,
                }
            }
            RaftMsg::AppendEntriesResponse {
                term,
                success,
                match_index,
            } => WireRaftMsg::AppendEntriesResponse {
                term,
                success,
                match_index,
            },
        }
    }
}

impl TryFrom<WireRaftMsg> for RaftMsg<String, EmbassyLogEntryCollection> {
    type Error = &'static str;

    fn try_from(wire: WireRaftMsg) -> Result<Self, Self::Error> {
        use raft_core::log_entry_collection::LogEntryCollection;

        match wire {
            WireRaftMsg::RequestVote {
                term,
                candidate_id,
                last_log_index,
                last_log_term,
            } => Ok(RaftMsg::RequestVote {
                term,
                candidate_id,
                last_log_index,
                last_log_term,
            }),
            WireRaftMsg::RequestVoteResponse { term, vote_granted } => {
                Ok(RaftMsg::RequestVoteResponse { term, vote_granted })
            }
            WireRaftMsg::AppendEntries {
                term,
                prev_log_index,
                prev_log_term,
                entries,
                leader_commit,
            } => {
                let log_entries: Vec<LogEntry<String>> = entries
                    .into_iter()
                    .map(LogEntry::from)
                    .collect();
                let collection = EmbassyLogEntryCollection::new(&log_entries);
                Ok(RaftMsg::AppendEntries {
                    term,
                    prev_log_index,
                    prev_log_term,
                    entries: collection,
                    leader_commit,
                })
            }
            WireRaftMsg::AppendEntriesResponse {
                term,
                success,
                match_index,
            } => Ok(RaftMsg::AppendEntriesResponse {
                term,
                success,
                match_index,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::ToString;
    use alloc::vec;
    use raft_core::log_entry_collection::LogEntryCollection;

    #[test]
    fn test_request_vote_round_trip() {
        let msg = RaftMsg::RequestVote {
            term: 5,
            candidate_id: 2,
            last_log_index: 10,
            last_log_term: 4,
        };

        let wire = WireRaftMsg::from(msg.clone());
        let bytes = postcard::to_allocvec(&wire).unwrap();
        let wire_decoded: WireRaftMsg = postcard::from_bytes(&bytes).unwrap();
        let msg_decoded = RaftMsg::try_from(wire_decoded).unwrap();

        assert_eq!(msg, msg_decoded);
    }

    #[test]
    fn test_request_vote_response_round_trip() {
        let msg = RaftMsg::RequestVoteResponse {
            term: 7,
            vote_granted: true,
        };

        let wire = WireRaftMsg::from(msg.clone());
        let bytes = postcard::to_allocvec(&wire).unwrap();
        let wire_decoded: WireRaftMsg = postcard::from_bytes(&bytes).unwrap();
        let msg_decoded = RaftMsg::try_from(wire_decoded).unwrap();

        assert_eq!(msg, msg_decoded);
    }

    #[test]
    fn test_append_entries_empty_round_trip() {
        let collection = EmbassyLogEntryCollection::new(&[]);

        let msg = RaftMsg::AppendEntries {
            term: 3,
            prev_log_index: 5,
            prev_log_term: 2,
            entries: collection,
            leader_commit: 4,
        };

        let wire = WireRaftMsg::from(msg.clone());
        let bytes = postcard::to_allocvec(&wire).unwrap();
        let wire_decoded: WireRaftMsg = postcard::from_bytes(&bytes).unwrap();
        let msg_decoded = RaftMsg::try_from(wire_decoded).unwrap();

        assert_eq!(msg, msg_decoded);
    }

    #[test]
    fn test_append_entries_with_log_round_trip() {
        let entries = vec![
            LogEntry {
                term: 1,
                payload: "cmd1".to_string(),
            },
            LogEntry {
                term: 2,
                payload: "cmd2".to_string(),
            },
            LogEntry {
                term: 2,
                payload: "cmd3".to_string(),
            },
        ];
        let collection = EmbassyLogEntryCollection::new(&entries);

        let msg = RaftMsg::AppendEntries {
            term: 3,
            prev_log_index: 5,
            prev_log_term: 2,
            entries: collection,
            leader_commit: 4,
        };

        let wire = WireRaftMsg::from(msg.clone());
        let bytes = postcard::to_allocvec(&wire).unwrap();
        let wire_decoded: WireRaftMsg = postcard::from_bytes(&bytes).unwrap();
        let msg_decoded = RaftMsg::try_from(wire_decoded).unwrap();

        assert_eq!(msg, msg_decoded);
    }

    #[test]
    fn test_append_entries_response_success_round_trip() {
        let msg = RaftMsg::AppendEntriesResponse {
            term: 5,
            success: true,
            match_index: 10,
        };

        let wire = WireRaftMsg::from(msg.clone());
        let bytes = postcard::to_allocvec(&wire).unwrap();
        let wire_decoded: WireRaftMsg = postcard::from_bytes(&bytes).unwrap();
        let msg_decoded = RaftMsg::try_from(wire_decoded).unwrap();

        assert_eq!(msg, msg_decoded);
    }

    #[test]
    fn test_append_entries_response_failure_round_trip() {
        let msg = RaftMsg::AppendEntriesResponse {
            term: 5,
            success: false,
            match_index: 0,
        };

        let wire = WireRaftMsg::from(msg.clone());
        let bytes = postcard::to_allocvec(&wire).unwrap();
        let wire_decoded: WireRaftMsg = postcard::from_bytes(&bytes).unwrap();
        let msg_decoded = RaftMsg::try_from(wire_decoded).unwrap();

        assert_eq!(msg, msg_decoded);
    }

    #[test]
    fn test_wire_msg_size_reasonable() {
        // Test that serialized messages are reasonably sized
        let msg = RaftMsg::RequestVote {
            term: 5,
            candidate_id: 2,
            last_log_index: 10,
            last_log_term: 4,
        };

        let wire = WireRaftMsg::from(msg);
        let bytes = postcard::to_allocvec(&wire).unwrap();

        // RequestVote should be < 100 bytes
        assert!(
            bytes.len() < 100,
            "RequestVote too large: {} bytes",
            bytes.len()
        );
    }

    #[test]
    fn test_large_log_entry_serialization() {
        // Test with larger payload
        let large_payload = "X".repeat(1000);
        let entries = vec![LogEntry {
            term: 1,
            payload: large_payload,
        }];
        let collection = EmbassyLogEntryCollection::new(&entries);

        let msg = RaftMsg::AppendEntries {
            term: 3,
            prev_log_index: 5,
            prev_log_term: 2,
            entries: collection,
            leader_commit: 4,
        };

        let wire = WireRaftMsg::from(msg.clone());
        let bytes = postcard::to_allocvec(&wire).unwrap();
        let wire_decoded: WireRaftMsg = postcard::from_bytes(&bytes).unwrap();
        let msg_decoded = RaftMsg::try_from(wire_decoded).unwrap();

        assert_eq!(msg, msg_decoded);
    }
}
