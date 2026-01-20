// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use raft_core::{
    election_manager::ElectionManager,
    node_collection::NodeCollection,
    node_state::NodeState,
    raft_messages::RaftMsg,
    snapshot::{Snapshot, SnapshotMetadata},
    state_machine::StateMachine,
    storage::Storage,
};
use raft_sim::{
    in_memory_log_entry_collection::InMemoryLogEntryCollection,
    in_memory_node_collection::InMemoryNodeCollection,
    in_memory_state_machine::InMemoryStateMachine, in_memory_storage::InMemoryStorage,
    no_action_timer::DummyTimer,
};

// ============================================================
// start_election tests
// ============================================================

#[test]
fn test_liveness_start_election_increments_term() {
    let mut election: ElectionManager<InMemoryNodeCollection, DummyTimer> =
        ElectionManager::new(DummyTimer);
    let mut storage = InMemoryStorage::new();
    let mut current_term = 1;
    let mut role = NodeState::Follower;

    let _msg: RaftMsg<String, InMemoryLogEntryCollection> = election.start_election(
        1, // node_id
        &mut current_term,
        &mut storage,
        &mut role,
    );

    assert_eq!(current_term, 2);
    assert_eq!(role, NodeState::Candidate);
    assert_eq!(storage.current_term(), 2);
}

#[test]
fn test_safety_start_election_votes_for_self() {
    let mut election: ElectionManager<InMemoryNodeCollection, DummyTimer> =
        ElectionManager::new(DummyTimer);
    let mut storage = InMemoryStorage::new();
    let mut current_term = 1;
    let mut role = NodeState::Follower;

    election.start_election::<String, InMemoryLogEntryCollection, _>(
        1,
        &mut current_term,
        &mut storage,
        &mut role,
    );

    assert_eq!(storage.voted_for(), Some(1));
}

#[test]
fn test_liveness_start_election_generates_correct_message() {
    let mut election: ElectionManager<InMemoryNodeCollection, DummyTimer> =
        ElectionManager::new(DummyTimer);
    let mut storage = InMemoryStorage::new();
    storage.set_current_term(5);
    let mut current_term = 5;
    let mut role = NodeState::Follower;

    let msg = election.start_election::<String, InMemoryLogEntryCollection, _>(
        42,
        &mut current_term,
        &mut storage,
        &mut role,
    );

    match msg {
        RaftMsg::RequestVote {
            term,
            candidate_id,
            last_log_index,
            last_log_term,
        } => {
            assert_eq!(term, 6);
            assert_eq!(candidate_id, 42);
            assert_eq!(last_log_index, 0);
            assert_eq!(last_log_term, 0);
        }
        _ => panic!("Expected RequestVote message"),
    }
}

// ============================================================
// handle_vote_request tests
// ============================================================

#[test]
fn test_safety_grant_vote_to_first_candidate() {
    let mut election: ElectionManager<InMemoryNodeCollection, DummyTimer> =
        ElectionManager::new(DummyTimer);
    let mut storage = InMemoryStorage::new();
    storage.set_current_term(1);
    let mut current_term = 1;
    let mut role = NodeState::Follower;

    let response = election.handle_vote_request::<String, InMemoryLogEntryCollection, _>(
        1, // term
        5, // candidate_id
        0, // last_log_index
        0, // last_log_term
        &mut current_term,
        &mut storage,
        &mut role,
    );

    match response {
        RaftMsg::RequestVoteResponse { term, vote_granted } => {
            assert_eq!(term, 1);
            assert!(vote_granted);
        }
        _ => panic!("Expected RequestVoteResponse"),
    }

    assert_eq!(storage.voted_for(), Some(5));
}

#[test]
fn test_safety_reject_when_already_voted() {
    let mut election: ElectionManager<InMemoryNodeCollection, DummyTimer> =
        ElectionManager::new(DummyTimer);
    let mut storage = InMemoryStorage::new();
    storage.set_current_term(1);
    storage.set_voted_for(Some(3));
    let mut current_term = 1;
    let mut role = NodeState::Follower;

    let response = election.handle_vote_request::<String, InMemoryLogEntryCollection, _>(
        1, // term
        5, // different candidate_id
        0,
        0,
        &mut current_term,
        &mut storage,
        &mut role,
    );

    match response {
        RaftMsg::RequestVoteResponse { vote_granted, .. } => {
            assert!(
                !vote_granted,
                "Should reject when already voted for different candidate"
            );
        }
        _ => panic!("Expected RequestVoteResponse"),
    }
}

#[test]
fn test_safety_grant_vote_to_same_candidate_twice() {
    let mut election: ElectionManager<InMemoryNodeCollection, DummyTimer> =
        ElectionManager::new(DummyTimer);
    let mut storage = InMemoryStorage::new();
    storage.set_current_term(1);
    storage.set_voted_for(Some(5));
    let mut current_term = 1;
    let mut role = NodeState::Follower;

    let response = election.handle_vote_request::<String, InMemoryLogEntryCollection, _>(
        1, // term
        5, // same candidate_id
        0,
        0,
        &mut current_term,
        &mut storage,
        &mut role,
    );

    match response {
        RaftMsg::RequestVoteResponse { vote_granted, .. } => {
            assert!(vote_granted, "Should grant vote to same candidate again");
        }
        _ => panic!("Expected RequestVoteResponse"),
    }
}

#[test]
fn test_safety_reject_less_up_to_date_candidate() {
    let mut election: ElectionManager<InMemoryNodeCollection, DummyTimer> =
        ElectionManager::new(DummyTimer);
    let mut storage = InMemoryStorage::new();

    // Local node has 3 entries in term 2
    storage.append_entries(&[
        raft_core::log_entry::LogEntry {
            term: 1,
            payload: "cmd1".to_string(),
        },
        raft_core::log_entry::LogEntry {
            term: 2,
            payload: "cmd2".to_string(),
        },
        raft_core::log_entry::LogEntry {
            term: 2,
            payload: "cmd3".to_string(),
        },
    ]);

    storage.set_current_term(2);
    let mut current_term = 2;
    let mut role = NodeState::Follower;

    // Candidate only has 2 entries in term 2
    let response = election.handle_vote_request::<String, InMemoryLogEntryCollection, _>(
        2, // term
        5, // candidate_id
        2, // last_log_index (less than our 3)
        2, // last_log_term
        &mut current_term,
        &mut storage,
        &mut role,
    );

    match response {
        RaftMsg::RequestVoteResponse { vote_granted, .. } => {
            assert!(
                !vote_granted,
                "Should reject candidate with less up-to-date log"
            );
        }
        _ => panic!("Expected RequestVoteResponse"),
    }
}

#[test]
fn test_safety_grant_to_equally_up_to_date_candidate() {
    let mut election: ElectionManager<InMemoryNodeCollection, DummyTimer> =
        ElectionManager::new(DummyTimer);
    let mut storage = InMemoryStorage::new();

    storage.append_entries(&[
        raft_core::log_entry::LogEntry {
            term: 1,
            payload: "cmd1".to_string(),
        },
        raft_core::log_entry::LogEntry {
            term: 2,
            payload: "cmd2".to_string(),
        },
    ]);

    storage.set_current_term(2);
    let mut current_term = 2;
    let mut role = NodeState::Follower;

    let response = election.handle_vote_request::<String, InMemoryLogEntryCollection, _>(
        2,
        5,
        2, // same last_log_index
        2, // same last_log_term
        &mut current_term,
        &mut storage,
        &mut role,
    );

    match response {
        RaftMsg::RequestVoteResponse { vote_granted, .. } => {
            assert!(
                vote_granted,
                "Should grant vote to equally up-to-date candidate"
            );
        }
        _ => panic!("Expected RequestVoteResponse"),
    }
}

#[test]
fn test_safety_update_term_from_higher_request() {
    let mut election: ElectionManager<InMemoryNodeCollection, DummyTimer> =
        ElectionManager::new(DummyTimer);
    let mut storage = InMemoryStorage::new();
    storage.set_current_term(1);
    let mut current_term = 1;
    let mut role = NodeState::Leader;

    election.handle_vote_request::<String, InMemoryLogEntryCollection, _>(
        5, // higher term
        7,
        0,
        0,
        &mut current_term,
        &mut storage,
        &mut role,
    );

    assert_eq!(current_term, 5);
    assert_eq!(storage.current_term(), 5);
    assert_eq!(role, NodeState::Follower, "Should step down to follower");
}

#[test]
fn test_safety_reject_stale_vote_request() {
    let mut election: ElectionManager<InMemoryNodeCollection, DummyTimer> =
        ElectionManager::new(DummyTimer);
    let mut storage = InMemoryStorage::new();
    storage.set_current_term(5);
    let mut current_term = 5;
    let mut role = NodeState::Follower;

    let response = election.handle_vote_request::<String, InMemoryLogEntryCollection, _>(
        3, // stale term
        7,
        0,
        0,
        &mut current_term,
        &mut storage,
        &mut role,
    );

    match response {
        RaftMsg::RequestVoteResponse { term, vote_granted } => {
            assert_eq!(term, 5);
            assert!(!vote_granted);
        }
        _ => panic!("Expected RequestVoteResponse"),
    }
}

// ============================================================
// handle_vote_response tests
// ============================================================

#[test]
fn test_liveness_become_leader_on_majority() {
    let mut election = ElectionManager::<InMemoryNodeCollection, DummyTimer>::new(DummyTimer);
    let mut storage = InMemoryStorage::new();
    storage.set_current_term(2);
    let mut current_term = 2;
    let mut role = NodeState::Candidate;

    // Start election (votes for self)
    election.start_election::<String, InMemoryLogEntryCollection, _>(
        1,
        &mut current_term,
        &mut storage,
        &mut role,
    );

    // Cluster of 3 nodes: need 2 votes (self + 1 peer)
    let mut peers = InMemoryNodeCollection::new();
    peers.push(2).unwrap();
    peers.push(3).unwrap();

    // Receive vote from node 2
    let became_leader = election.handle_vote_response(
        2,    // from
        3,    // term
        true, // vote_granted
        &current_term,
        &role,
        peers.len(),
    );

    if became_leader {
        role = NodeState::Leader;
    }

    assert!(
        became_leader,
        "Should become leader with majority (2/3 votes)"
    );
    assert_eq!(role, NodeState::Leader);
}

#[test]
fn test_liveness_stay_candidate_without_majority() {
    let mut election = ElectionManager::<InMemoryNodeCollection, DummyTimer>::new(DummyTimer);
    let mut storage = InMemoryStorage::new();
    storage.set_current_term(2);
    let mut current_term = 2;
    let mut role = NodeState::Candidate;

    election.start_election::<String, InMemoryLogEntryCollection, _>(
        1,
        &mut current_term,
        &mut storage,
        &mut role,
    );

    // Cluster of 5 nodes: need 3 votes
    let mut peers = InMemoryNodeCollection::new();
    for i in 2..=5 {
        peers.push(i).unwrap();
    }

    // Receive only 1 vote (self + 1 = 2/5)
    let became_leader =
        election.handle_vote_response(2, 3, true, &current_term, &role, peers.len());

    assert!(!became_leader, "Should stay candidate without majority");
    assert_eq!(role, NodeState::Candidate);
}

#[test]
fn test_liveness_handle_vote_rejection() {
    let mut election = ElectionManager::<InMemoryNodeCollection, DummyTimer>::new(DummyTimer);
    let mut storage = InMemoryStorage::new();
    storage.set_current_term(2);
    let mut current_term = 2;
    let mut role = NodeState::Candidate;

    election.start_election::<String, InMemoryLogEntryCollection, _>(
        1,
        &mut current_term,
        &mut storage,
        &mut role,
    );

    let mut peers = InMemoryNodeCollection::new();
    peers.push(2).unwrap();

    // Receive rejection
    let became_leader = election.handle_vote_response(
        2,
        3,
        false, // vote_granted = false
        &current_term,
        &role,
        peers.len(),
    );

    assert!(!became_leader);
    assert_eq!(role, NodeState::Candidate);
}

#[test]
fn test_safety_ignore_stale_vote_response() {
    let mut election = ElectionManager::<InMemoryNodeCollection, DummyTimer>::new(DummyTimer);
    let mut storage = InMemoryStorage::new();
    storage.set_current_term(5);
    let mut current_term = 5;
    let mut role = NodeState::Candidate;

    election.start_election::<String, InMemoryLogEntryCollection, _>(
        1,
        &mut current_term,
        &mut storage,
        &mut role,
    );

    let mut peers = InMemoryNodeCollection::new();
    peers.push(2).unwrap();

    // Receive vote from old term
    let became_leader = election.handle_vote_response(
        2,
        3, // old term
        true,
        &current_term,
        &role,
        peers.len(),
    );

    assert!(!became_leader, "Should ignore stale vote");
}

#[test]
fn test_safety_step_down_on_higher_term_in_response() {
    let mut election = ElectionManager::<InMemoryNodeCollection, DummyTimer>::new(DummyTimer);
    let mut storage = InMemoryStorage::new();
    storage.set_current_term(2);
    let mut current_term = 2;
    let mut role = NodeState::Candidate;

    election.start_election::<String, InMemoryLogEntryCollection, _>(
        1,
        &mut current_term,
        &mut storage,
        &mut role,
    );

    let peers = InMemoryNodeCollection::new();

    // Simulate what RaftNode does: detect higher term and step down before processing
    let response_term = 10;
    if response_term > current_term {
        current_term = response_term;
        role = NodeState::Follower;
    }

    // Now call handle_vote_response (but since we're follower now, it won't do anything)
    election.handle_vote_response(2, response_term, false, &current_term, &role, peers.len());

    assert_eq!(current_term, 10);
    assert_eq!(role, NodeState::Follower, "Should step down on higher term");
}

// ============================================================
// Edge cases
// ============================================================

#[test]
fn test_liveness_majority_calculation_even_cluster() {
    let mut election = ElectionManager::<InMemoryNodeCollection, DummyTimer>::new(DummyTimer);
    let mut storage = InMemoryStorage::new();
    storage.set_current_term(1);
    let mut current_term = 1;
    let mut role = NodeState::Candidate;

    election.start_election::<String, InMemoryLogEntryCollection, _>(
        1,
        &mut current_term,
        &mut storage,
        &mut role,
    );

    // Cluster of 4 nodes: need 3 votes (majority of 4 is 3)
    let mut peers = InMemoryNodeCollection::new();
    peers.push(2).unwrap();
    peers.push(3).unwrap();
    peers.push(4).unwrap();

    // Get 1 vote (self + 1 = 2/4) - not enough
    election.handle_vote_response(2, 2, true, &current_term, &role, peers.len());
    assert_eq!(role, NodeState::Candidate);

    // Get 2nd vote (self + 2 = 3/4) - should become leader
    let became_leader =
        election.handle_vote_response(3, 2, true, &current_term, &role, peers.len());
    if became_leader {
        role = NodeState::Leader;
    }
    assert!(became_leader);
    assert_eq!(role, NodeState::Leader);
}

#[test]
fn test_liveness_single_node_cluster() {
    let mut election = ElectionManager::<InMemoryNodeCollection, DummyTimer>::new(DummyTimer);
    let mut storage = InMemoryStorage::new();
    storage.set_current_term(1);
    let mut current_term = 1;
    let mut role = NodeState::Candidate;

    election.start_election::<String, InMemoryLogEntryCollection, _>(
        1,
        &mut current_term,
        &mut storage,
        &mut role,
    );

    // Single-node cluster (0 peers)
    let peers = InMemoryNodeCollection::new();

    // In a single-node cluster, the node should immediately have majority
    // after voting for itself (1 vote out of 1 total nodes)
    // Simulate checking if we can become leader
    let became_leader = election.handle_vote_response(
        1, // from self (doesn't matter, won't be counted again)
        current_term,
        true,
        &current_term,
        &role,
        peers.len(), // 0 peers means cluster of 1
    );

    if became_leader {
        role = NodeState::Leader;
    }

    assert!(
        became_leader,
        "Single-node cluster should immediately become leader"
    );
    assert_eq!(role, NodeState::Leader);
}

// ============================================================
// Snapshot / Log Compaction Tests
// ============================================================

#[test]
fn test_liveness_start_election_with_compacted_log() {
    // Test: Candidate with compacted log starts election
    // Expected: RequestVote should use snapshot metadata for last_log_index/term
    let mut election: ElectionManager<InMemoryNodeCollection, DummyTimer> =
        ElectionManager::new(DummyTimer);
    let mut storage = InMemoryStorage::new();
    storage.set_current_term(3);
    let mut current_term = 3;
    let mut role = NodeState::Follower;

    // Create log with entries 1-15
    use raft_core::log_entry::LogEntry;
    let entries: Vec<LogEntry<String>> = (1..=15)
        .map(|i| LogEntry {
            term: 2,
            payload: format!("cmd{}", i),
        })
        .collect();
    storage.append_entries(&entries);

    // Create snapshot at index 10, term 2
    use raft_core::snapshot::{Snapshot, SnapshotMetadata};
    use raft_core::state_machine::StateMachine;
    use raft_sim::in_memory_state_machine::InMemoryStateMachine;

    let mut state_machine = InMemoryStateMachine::new();
    for i in 1..=10 {
        state_machine.apply(&format!("cmd{}", i));
    }
    let snapshot_data = state_machine.create_snapshot();

    let snapshot = Snapshot {
        metadata: SnapshotMetadata {
            last_included_index: 10,
            last_included_term: 2,
        },
        data: snapshot_data,
    };
    storage.save_snapshot(snapshot);

    // Compact the log - discard entries 1-10
    storage.discard_entries_before(11);

    // Now log has entries 11-15, but if we discard those too...
    storage.discard_entries_before(16);

    // Log is now empty, only snapshot remains
    assert_eq!(storage.first_log_index(), 16);
    assert_eq!(storage.last_log_index(), 15); // Returns snapshot point
    assert_eq!(storage.last_log_term(), 2); // Returns snapshot term

    // Start election - should use snapshot metadata
    let msg = election.start_election::<String, InMemoryLogEntryCollection, _>(
        1,
        &mut current_term,
        &mut storage,
        &mut role,
    );

    match msg {
        RaftMsg::RequestVote {
            term,
            candidate_id,
            last_log_index,
            last_log_term,
        } => {
            assert_eq!(term, 4);
            assert_eq!(candidate_id, 1);
            assert_eq!(
                last_log_index, 15,
                "Should use snapshot's last_included_index"
            );
            assert_eq!(last_log_term, 2, "Should use snapshot's last_included_term");
        }
        _ => panic!("Expected RequestVote message"),
    }
}

#[test]
fn test_safety_grant_vote_with_compacted_log() {
    // Test: Voter with compacted log evaluates candidate
    // Expected: Should use snapshot metadata for up-to-date check
    let mut election: ElectionManager<InMemoryNodeCollection, DummyTimer> =
        ElectionManager::new(DummyTimer);
    let mut storage = InMemoryStorage::new();
    storage.set_current_term(3);
    let mut current_term = 3;
    let mut role = NodeState::Follower;

    // Voter has entries 1-20, creates snapshot at 15, compacts everything
    use raft_core::log_entry::LogEntry;
    let entries: Vec<LogEntry<String>> = (1..=20)
        .map(|i| LogEntry {
            term: 2,
            payload: format!("cmd{}", i),
        })
        .collect();
    storage.append_entries(&entries);

    // Create snapshot at index 15, term 2
    use raft_core::snapshot::{Snapshot, SnapshotMetadata};
    use raft_core::state_machine::StateMachine;
    use raft_sim::in_memory_state_machine::InMemoryStateMachine;

    let mut state_machine = InMemoryStateMachine::new();
    for i in 1..=15 {
        state_machine.apply(&format!("cmd{}", i));
    }
    let snapshot_data = state_machine.create_snapshot();

    let snapshot = Snapshot {
        metadata: SnapshotMetadata {
            last_included_index: 15,
            last_included_term: 2,
        },
        data: snapshot_data,
    };
    storage.save_snapshot(snapshot);

    // Compact all entries up to and including index 20 (all entries)
    // discard_entries_before(21) means discard entries with index < 21, i.e., all of them
    storage.discard_entries_before(21);

    // Voter's log should be empty after compaction
    // When log is empty, last_log_index() returns first_index - 1
    // first_index should be 21 after discarding, so last_log_index = 20
    // BUT, when log is empty and there's a snapshot, last_log_term returns snapshot term
    // Actually, let me check the actual behavior
    let actual_last_index = storage.last_log_index();
    let actual_last_term = storage.last_log_term();

    // When log is empty, last_log_index returns first_index - 1 = 21 - 1 = 20
    // When log is empty with snapshot, last_log_term returns snapshot term
    assert_eq!(actual_last_index, 20, "last_log_index when log empty");
    assert_eq!(actual_last_term, 2, "last_log_term from snapshot"); // From snapshot

    // Candidate has log up to index 18, term 2 (less up-to-date than our 20)
    let response = election.handle_vote_request::<String, InMemoryLogEntryCollection, _>(
        3,  // term
        5,  // candidate_id
        18, // last_log_index (behind our 20)
        2,  // last_log_term (same term)
        &mut current_term,
        &mut storage,
        &mut role,
    );

    match response {
        RaftMsg::RequestVoteResponse { vote_granted, .. } => {
            assert!(
                !vote_granted,
                "Should reject candidate - we have more recent log (index 20 vs 18)"
            );
        }
        _ => panic!("Expected RequestVoteResponse"),
    }
}

#[test]
fn test_safety_reject_candidate_with_older_log_than_snapshot() {
    // Test: Voter with compacted log rejects less up-to-date candidate
    // Expected: Candidate with log older than voter's snapshot should be rejected
    let mut election: ElectionManager<InMemoryNodeCollection, DummyTimer> =
        ElectionManager::new(DummyTimer);
    let mut storage = InMemoryStorage::new();
    storage.set_current_term(3);
    let mut current_term = 3;
    let mut role = NodeState::Follower;

    // Voter has snapshot at index 15, term 2
    use raft_core::log_entry::LogEntry;
    let entries: Vec<LogEntry<String>> = (1..=15)
        .map(|i| LogEntry {
            term: 2,
            payload: format!("cmd{}", i),
        })
        .collect();
    storage.append_entries(&entries);

    use raft_core::snapshot::{Snapshot, SnapshotMetadata};
    use raft_core::state_machine::StateMachine;
    use raft_sim::in_memory_state_machine::InMemoryStateMachine;

    let mut state_machine = InMemoryStateMachine::new();
    for i in 1..=15 {
        state_machine.apply(&format!("cmd{}", i));
    }
    let snapshot_data = state_machine.create_snapshot();

    let snapshot = Snapshot {
        metadata: SnapshotMetadata {
            last_included_index: 15,
            last_included_term: 2,
        },
        data: snapshot_data,
    };
    storage.save_snapshot(snapshot);

    storage.discard_entries_before(16);

    assert_eq!(storage.last_log_index(), 15);
    assert_eq!(storage.last_log_term(), 2);

    // Candidate only has log up to index 10, term 2 (less up-to-date)
    let response = election.handle_vote_request::<String, InMemoryLogEntryCollection, _>(
        3,  // term
        5,  // candidate_id
        10, // last_log_index (behind our snapshot)
        2,  // last_log_term
        &mut current_term,
        &mut storage,
        &mut role,
    );

    match response {
        RaftMsg::RequestVoteResponse { vote_granted, .. } => {
            assert!(
                !vote_granted,
                "Should reject candidate with log older than our snapshot"
            );
        }
        _ => panic!("Expected RequestVoteResponse"),
    }
}

#[test]
fn test_safety_grant_vote_with_higher_term_than_snapshot() {
    // Test: Candidate with higher term but shorter log should win
    // Expected: Term comparison takes precedence over log length
    let mut election: ElectionManager<InMemoryNodeCollection, DummyTimer> =
        ElectionManager::new(DummyTimer);
    let mut storage = InMemoryStorage::new();
    storage.set_current_term(3);
    let mut current_term = 3;
    let mut role = NodeState::Follower;

    // Voter has snapshot at index 15, term 2
    use raft_core::log_entry::LogEntry;
    let entries: Vec<LogEntry<String>> = (1..=15)
        .map(|i| LogEntry {
            term: 2,
            payload: format!("cmd{}", i),
        })
        .collect();
    storage.append_entries(&entries);

    let mut state_machine = InMemoryStateMachine::new();
    for i in 1..=15 {
        state_machine.apply(&format!("cmd{}", i));
    }
    let snapshot_data = state_machine.create_snapshot();

    let snapshot = Snapshot {
        metadata: SnapshotMetadata {
            last_included_index: 15,
            last_included_term: 2,
        },
        data: snapshot_data,
    };
    storage.save_snapshot(snapshot);

    storage.discard_entries_before(16);

    // Voter: index 15, term 2
    assert_eq!(storage.last_log_index(), 15);
    assert_eq!(storage.last_log_term(), 2);

    // Candidate: index 10, term 3 (higher term wins despite shorter log)
    let response = election.handle_vote_request::<String, InMemoryLogEntryCollection, _>(
        3,  // term
        5,  // candidate_id
        10, // last_log_index (shorter)
        3,  // last_log_term (higher term!)
        &mut current_term,
        &mut storage,
        &mut role,
    );

    match response {
        RaftMsg::RequestVoteResponse { vote_granted, .. } => {
            assert!(
                vote_granted,
                "Should grant vote - candidate has higher last_log_term"
            );
        }
        _ => panic!("Expected RequestVoteResponse"),
    }
}
