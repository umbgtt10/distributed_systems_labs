// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use raft_core::{
    event::Event, node_state::NodeState, state_machine::StateMachine, storage::Storage,
    timer_service::TimerKind,
};
use raft_sim::timeless_test_cluster::TimelessTestCluster;

#[test]
fn test_leader_creates_snapshot_after_threshold() {
    // Arrange: Create 3-node cluster
    let mut cluster = TimelessTestCluster::new();
    cluster.add_node(1);
    cluster.add_node(2);
    cluster.add_node(3);
    cluster.connect_peers();

    // Node 1 becomes leader
    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Election));
    cluster.deliver_messages();
    assert_eq!(*cluster.get_node(1).role(), NodeState::Leader);

    // Act: Submit commands to reach threshold (10 entries)
    // Snapshot should be created automatically when commit_index reaches 10
    for i in 1..=10 {
        cluster
            .get_node_mut(1)
            .on_event(Event::ClientCommand(format!("SET key{}=value{}", i, i)));
        cluster.deliver_messages();
    }

    // Verify all entries committed
    assert_eq!(cluster.get_node(1).commit_index(), 10);

    // Assert: Snapshot created automatically at threshold
    let snapshot = cluster.get_node(1).storage().load_snapshot();
    assert!(
        snapshot.is_some(),
        "Snapshot should be created automatically at threshold"
    );

    let snapshot = snapshot.unwrap();
    assert_eq!(
        snapshot.metadata.last_included_index, 10,
        "Snapshot should include all 10 committed entries"
    );
    assert_eq!(
        snapshot.metadata.last_included_term, 1,
        "Snapshot should be from term 1"
    );

    // Assert: State machine state is captured in snapshot
    // (We can verify this by restoring to a new state machine)
    let mut test_sm = raft_sim::in_memory_state_machine::InMemoryStateMachine::new();
    let restore_result = test_sm.restore_from_snapshot(&snapshot.data);
    assert!(restore_result.is_ok(), "Snapshot should be restorable");

    // Verify all keys present
    for i in 1..=10 {
        let expected = format!("value{}", i);
        assert_eq!(test_sm.get(&format!("key{}", i)), Some(&expected[..]));
    }

    // Assert: Log has been compacted - entries discarded, first_log_index advanced
    assert_eq!(cluster.get_node(1).storage().last_log_index(), 10);
    assert_eq!(
        cluster.get_node(1).storage().first_log_index(),
        11,
        "Log should be compacted"
    );
}

/// Test #4: Log Compaction
/// Leader automatically creates snapshot and compacts log when threshold exceeded
#[test]
fn test_leader_automatically_compacts_log_at_threshold() {
    // Arrange: Create 3-node cluster
    let mut cluster = TimelessTestCluster::new();
    cluster.add_node(1);
    cluster.add_node(2);
    cluster.add_node(3);
    cluster.connect_peers();

    // Node 1 becomes leader
    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Election));
    cluster.deliver_messages();
    assert_eq!(*cluster.get_node(1).role(), NodeState::Leader);

    // Act: Submit commands to trigger automatic snapshot (threshold = 10)
    // We'll submit 15 commands - first 10 should trigger snapshot, then 5 more
    for i in 1..=15 {
        cluster
            .get_node_mut(1)
            .on_event(Event::ClientCommand(format!("SET key{}=value{}", i, i)));
        cluster.deliver_messages();
    }

    // Verify all entries committed
    assert_eq!(cluster.get_node(1).commit_index(), 15);

    // Assert: Snapshot should have been created automatically at index 10
    let snapshot = cluster.get_node(1).storage().load_snapshot();
    assert!(
        snapshot.is_some(),
        "Snapshot should be created automatically at threshold"
    );

    let snapshot = snapshot.unwrap();
    assert_eq!(
        snapshot.metadata.last_included_index, 10,
        "Snapshot should include first 10 entries"
    );
    assert_eq!(
        snapshot.metadata.last_included_term, 1,
        "Snapshot should be from term 1"
    );

    // Assert: Log has been compacted - entries 1-10 should be discarded
    let first_log_index = cluster.get_node(1).storage().first_log_index();
    assert_eq!(
        first_log_index, 11,
        "First log index should be 11 after compacting entries 1-10"
    );

    // Assert: Remaining entries (11-15) should still be in log
    assert_eq!(
        cluster.get_node(1).storage().last_log_index(),
        15,
        "Last log index should be 15"
    );

    // Assert: Can retrieve entries 11-15 but not 1-10
    let entry_11 = cluster.get_node(1).storage().get_entry(11);
    assert!(entry_11.is_some(), "Entry 11 should still be in log");

    let entry_5 = cluster.get_node(1).storage().get_entry(5);
    assert!(entry_5.is_none(), "Entry 5 should have been compacted away");

    // Assert: State machine has all 15 entries applied
    let state_machine = cluster.get_node(1).state_machine();
    for i in 1..=15 {
        let expected = format!("value{}", i);
        assert_eq!(
            state_machine.get(&format!("key{}", i)),
            Some(&expected[..]),
            "Key {} should be in state machine",
            i
        );
    }

    // Assert: Snapshot data contains first 10 entries
    let mut test_sm = raft_sim::in_memory_state_machine::InMemoryStateMachine::new();
    let restore_result = test_sm.restore_from_snapshot(&snapshot.data);
    assert!(restore_result.is_ok(), "Snapshot should be restorable");

    for i in 1..=10 {
        let expected = format!("value{}", i);
        assert_eq!(test_sm.get(&format!("key{}", i)), Some(&expected[..]));
    }

    // Assert: Cluster continues operating - submit another command
    cluster
        .get_node_mut(1)
        .on_event(Event::ClientCommand("SET key16=value16".to_string()));
    cluster.deliver_messages();

    assert_eq!(cluster.get_node(1).commit_index(), 16);
    assert_eq!(
        cluster.get_node(1).state_machine().get("key16"),
        Some("value16")
    );
}
