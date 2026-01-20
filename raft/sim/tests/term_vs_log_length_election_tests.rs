// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use raft_core::{event::Event, node_state::NodeState, storage::Storage, timer_service::TimerKind};
use raft_sim::{in_memory_storage::InMemoryStorage, timeless_test_cluster::TimelessTestCluster};

/// Test: Higher Term wins against Longer Log
///
/// Scenario:
/// - Node 1: Term 2, Log [1, 1] (Length 2) -> "Newer but Shorter"
/// - Node 2: Term 1, Log [1, 1, 1, 1, 1] (Length 5) -> "Older but Longer"
///
/// In Raft, Term takes precedence.
/// If Node 1 requests vote from Node 2:
///   - Node 2 checks: Node 1 Term (2) > Node 2 Term (1).
///   - Node 2 grants vote.
///
/// If Node 2 requests vote from Node 1:
///   - Node 1 checks: Node 2 Term (1) < Node 1 Term (2).
///   - Node 1 rejects vote.
#[test]
fn test_safety_higher_term_beats_longer_log() {
    // Arrange - Custom cluster construction
    let mut cluster = TimelessTestCluster::new();

    // Node 1 setup: Term 2, Short Log
    let mut storage1 = InMemoryStorage::new();
    storage1.set_current_term(2);
    // Append 2 entries (Term 2) - assume they were committed in Term 2 or just local
    // Actually, to be valid, they should be Term 1?
    // Let's say Node 1 was leader in Term 2 and wrote entries but crashed.
    // To make it simple: Log has entries from Term 1 and Term 2.
    // [1, 2]
    // Or just [2, 2].
    // Let's use [1, 2].
    storage1.append_entries(&[
        raft_core::log_entry::LogEntry {
            term: 1,
            payload: "1".to_string(),
        },
        raft_core::log_entry::LogEntry {
            term: 2,
            payload: "2".to_string(),
        },
    ]);
    cluster.add_node_with_storage(1, storage1);

    // Node 2 setup: Term 1, Long Log
    let mut storage2 = InMemoryStorage::new();
    storage2.set_current_term(1);
    // Append 5 entries (Term 1)
    storage2.append_entries(&[
        raft_core::log_entry::LogEntry {
            term: 1,
            payload: "1".to_string(),
        },
        raft_core::log_entry::LogEntry {
            term: 1,
            payload: "2".to_string(),
        },
        raft_core::log_entry::LogEntry {
            term: 1,
            payload: "3".to_string(),
        },
        raft_core::log_entry::LogEntry {
            term: 1,
            payload: "4".to_string(),
        },
        raft_core::log_entry::LogEntry {
            term: 1,
            payload: "5".to_string(),
        },
    ]);
    cluster.add_node_with_storage(2, storage2);

    cluster.connect_peers();

    // Act 1: Node 1 (Newer/Shorter) requests vote
    // Trigger election on Node 1
    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Election));

    // Deliver RequestVote(Term 3) from Node 1 to Node 2
    // Node 1 increments term to 3.
    cluster.deliver_messages();

    // Assert 1: Node 1 should become Leader
    assert_eq!(*cluster.get_node(1).role(), NodeState::Leader);
    assert_eq!(cluster.get_node(1).current_term(), 3);

    // Check Node 2 state - should have updated term and voted for 1
    assert_eq!(cluster.get_node(2).current_term(), 3);
    assert_eq!(cluster.get_node(2).storage().voted_for(), Some(1));
}

#[test]
fn test_safety_older_longer_log_loses_to_newer_shorter() {
    // Arrange - Custom cluster construction
    let mut cluster = TimelessTestCluster::new();

    // Node 1 setup: Term 2, Short Log
    let mut storage1 = InMemoryStorage::new();
    storage1.set_current_term(2);
    storage1.append_entries(&[
        raft_core::log_entry::LogEntry {
            term: 1,
            payload: "1".to_string(),
        },
        raft_core::log_entry::LogEntry {
            term: 2,
            payload: "2".to_string(),
        },
    ]);
    cluster.add_node_with_storage(1, storage1);

    // Node 2 setup: Term 1, Long Log
    let mut storage2 = InMemoryStorage::new();
    storage2.set_current_term(1);
    storage2.append_entries(&[
        raft_core::log_entry::LogEntry {
            term: 1,
            payload: "1".to_string(),
        },
        raft_core::log_entry::LogEntry {
            term: 1,
            payload: "2".to_string(),
        },
        raft_core::log_entry::LogEntry {
            term: 1,
            payload: "3".to_string(),
        },
    ]);
    cluster.add_node_with_storage(2, storage2);

    cluster.connect_peers();

    // Act: Node 2 (Older/Longer) requests vote
    // Trigger election on Node 2
    cluster
        .get_node_mut(2)
        .on_event(Event::TimerFired(TimerKind::Election));

    // Deliver RequestVote(Term 2) from Node 2 to Node 1
    // Wait, Node 2 starts at Term 1. Increments to 2.
    // Node 1 is already at Term 2.
    // Node 2 sends RequestVote (Term 2).
    // Node 1 checks: Term 2 == Term 2.
    // Log check:
    // Node 2 Last Log: Index 3, Term 1.
    // Node 1 Last Log: Index 2, Term 2.
    // Node 1 sees Node 2's LastTerm (1) < Own LastTerm (2).
    // Rejects vote.

    cluster.deliver_messages();

    // Assert: Node 2 should fail to become leader
    assert_ne!(*cluster.get_node(2).role(), NodeState::Leader);

    // Node 1 should eventually time out and win (tested in previous test)
}
