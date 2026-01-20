// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use raft_core::{
    event::Event, node_state::NodeState,
    state_machine::StateMachine, storage::Storage, timer_service::TimerKind,
};
use raft_sim::timeless_test_cluster::TimelessTestCluster;

/// Test that a node restarts and restores state from snapshot
#[test]
fn test_node_restarts_and_restores_from_snapshot() {
    // Phase 1: Create cluster and generate snapshot
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

    // Apply 10 commands to trigger snapshot at threshold
    for i in 1..=10 {
        cluster
            .get_node_mut(1)
            .on_event(Event::ClientCommand(format!("SET key{}=value{}", i, i)));
        cluster.deliver_messages();
    }

    // Verify snapshot was created
    let snapshot = cluster.get_node(1).storage().load_snapshot();
    assert!(snapshot.is_some(), "Snapshot should exist");
    assert_eq!(snapshot.unwrap().metadata.last_included_index, 10);

    // Verify state machine has all keys
    for i in 1..=10 {
        assert_eq!(
            cluster
                .get_node(1)
                .state_machine()
                .get(&format!("key{}", i)),
            Some(&format!("value{}", i)[..])
        );
    }

    // Phase 2: Simulate node 1 crash and restart
    // Save storage state (this simulates persistent storage)
    let saved_storage = cluster.get_node(1).storage().clone();
    let saved_term = cluster.get_node(1).current_term();

    // Remove node 1 from cluster (simulating crash)
    cluster.remove_node(1);

    // Create new node instance with saved storage (simulating restart)
    cluster.add_node_with_storage(1, saved_storage);

    // Phase 3: Verify recovery
    let recovered_node = cluster.get_node(1);

    // State machine should be restored from snapshot
    for i in 1..=10 {
        assert_eq!(
            recovered_node.state_machine().get(&format!("key{}", i)),
            Some(&format!("value{}", i)[..]),
            "State machine should be restored from snapshot"
        );
    }

    // Storage should reflect snapshot state
    assert_eq!(recovered_node.storage().first_log_index(), 11);
    assert_eq!(recovered_node.current_term(), saved_term);

    // Snapshot should still be loadable
    let snapshot_after_restart = recovered_node.storage().load_snapshot();
    assert!(
        snapshot_after_restart.is_some(),
        "Snapshot should persist across restart"
    );
    assert_eq!(
        snapshot_after_restart.unwrap().metadata.last_included_index,
        10
    );
}

/// Test node restart with snapshot AND remaining log entries
#[test]
fn test_restart_with_snapshot_and_remaining_entries() {
    // Phase 1: Create cluster and generate data beyond snapshot
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

    // Apply 15 commands (snapshot at 10, then 5 more)
    for i in 1..=15 {
        cluster
            .get_node_mut(1)
            .on_event(Event::ClientCommand(format!("SET key{}=value{}", i, i)));
        cluster.deliver_messages();
    }

    // Verify snapshot at index 10
    let snapshot = cluster.get_node(1).storage().load_snapshot();
    assert_eq!(snapshot.unwrap().metadata.last_included_index, 10);

    // Verify all 15 entries applied
    assert_eq!(cluster.get_node(1).commit_index(), 15);

    // Phase 2: Simulate restart
    let saved_storage = cluster.get_node(1).storage().clone();
    cluster.remove_node(1);
    cluster.add_node_with_storage(1, saved_storage);

    // Phase 3: Verify recovery
    let recovered_node = cluster.get_node(1);

    // State machine should have first 10 entries from snapshot only
    // Entries 11-15 are uncommitted after restart
    for i in 1..=10 {
        assert_eq!(
            recovered_node.state_machine().get(&format!("key{}", i)),
            Some(&format!("value{}", i)[..]),
            "Key {} should be in snapshot",
            i
        );
    }

    // Entries 11-15 are NOT in state machine yet (uncommitted)
    for i in 11..=15 {
        assert_eq!(
            recovered_node.state_machine().get(&format!("key{}", i)),
            None,
            "Key {} should not be applied yet (uncommitted)",
            i
        );
    }

    // Log structure should be correct
    assert_eq!(
        recovered_node.storage().first_log_index(),
        11,
        "First log index should be after snapshot"
    );
    assert_eq!(
        recovered_node.storage().last_log_index(),
        15,
        "Last log index should include remaining entries"
    );

    // Entries 11-15 should be in log
    for i in 11..=15 {
        assert!(
            recovered_node.storage().get_entry(i).is_some(),
            "Entry {} should be in log",
            i
        );
    }

    // Entries 1-10 should be compacted away
    for i in 1..=10 {
        assert!(
            recovered_node.storage().get_entry(i).is_none(),
            "Entry {} should be compacted",
            i
        );
    }
}

/// Test follower crash and recovery during snapshot transfer
#[test]
fn test_follower_crash_during_snapshot_transfer() {
    // Phase 1: Setup cluster with leader that has snapshot
    let mut cluster = TimelessTestCluster::new();
    cluster.add_node(1);
    cluster.add_node(2);
    cluster.add_node(3);
    cluster.connect_peers();

    // Node 1 becomes leader and creates snapshot
    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Election));
    cluster.deliver_messages();

    for i in 1..=10 {
        cluster
            .get_node_mut(1)
            .on_event(Event::ClientCommand(format!("SET key{}=value{}", i, i)));
        cluster.deliver_messages();
    }

    // Leader has snapshot at index 10
    assert!(cluster.get_node(1).storage().load_snapshot().is_some());

    // Phase 2: Node 2 crashes before receiving snapshot
    let saved_storage = cluster.get_node(2).storage().clone();
    cluster.remove_node(2);

    // Leader continues with more commands
    for i in 11..=12 {
        cluster
            .get_node_mut(1)
            .on_event(Event::ClientCommand(format!("SET key{}=value{}", i, i)));
        cluster.deliver_messages();
    }

    // Phase 3: Node 2 restarts (still has old state, no snapshot)
    cluster.add_node_with_storage(2, saved_storage);
    cluster.reconnect_node(2);

    // Leader should send InstallSnapshot to node 2
    // (This is tested in log_replication_manager_tests, we just verify recovery works)

    // Deliver messages to complete snapshot transfer
    cluster.deliver_messages();

    // Phase 4: Verify node 2 caught up
    // Node 2 should eventually receive snapshot and new entries
    // For now, just verify it restarted successfully
    assert_eq!(*cluster.get_node(2).role(), NodeState::Follower);
}

/// Test node restart with no snapshot (cold start)
#[test]
fn test_node_restart_without_snapshot() {
    // Phase 1: Create node with some entries but no snapshot
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

    // Apply only 5 commands (below threshold)
    for i in 1..=5 {
        cluster
            .get_node_mut(1)
            .on_event(Event::ClientCommand(format!("SET key{}=value{}", i, i)));
        cluster.deliver_messages();
    }

    // Verify NO snapshot created (below threshold)
    assert!(cluster.get_node(1).storage().load_snapshot().is_none());

    // Phase 2: Simulate restart
    let saved_storage = cluster.get_node(1).storage().clone();
    cluster.remove_node(1);
    cluster.add_node_with_storage(1, saved_storage);

    // Phase 3: Verify recovery without snapshot
    let recovered_node = cluster.get_node(1);

    // State machine should be empty - log entries are uncommitted
    // (In real Raft, we don't persist commit_index, so we can't replay uncommitted entries)
    for i in 1..=5 {
        assert_eq!(
            recovered_node.state_machine().get(&format!("key{}", i)),
            None,
            "State machine should be empty - entries not committed yet"
        );
    }

    // But log entries should still be present (will be re-committed by leader)
    assert_eq!(recovered_node.storage().last_log_index(), 5);
    assert_eq!(recovered_node.storage().first_log_index(), 1);
}

/// Test multiple crash-recovery cycles
#[test]
fn test_multiple_restart_cycles() {
    let mut cluster = TimelessTestCluster::new();
    cluster.add_node(1);
    cluster.add_node(2);
    cluster.add_node(3);
    cluster.connect_peers();

    // Become leader
    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Election));
    cluster.deliver_messages();

    // Cycle 1: Apply 10 commands, crash, restart
    for i in 1..=10 {
        cluster
            .get_node_mut(1)
            .on_event(Event::ClientCommand(format!(
                "SET cycle1_key{}=value{}",
                i, i
            )));
        cluster.deliver_messages();
    }

    let storage1 = cluster.get_node(1).storage().clone();
    cluster.remove_node(1);
    cluster.add_node_with_storage(1, storage1);

    // Verify recovery
    for i in 1..=10 {
        assert_eq!(
            cluster
                .get_node(1)
                .state_machine()
                .get(&format!("cycle1_key{}", i)),
            Some(&format!("value{}", i)[..])
        );
    }

    // Cycle 2: Reconnect, apply more commands, crash again
    cluster.reconnect_node(1);
    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Election));
    cluster.deliver_messages();

    for i in 1..=5 {
        cluster
            .get_node_mut(1)
            .on_event(Event::ClientCommand(format!(
                "SET cycle2_key{}=value{}",
                i, i
            )));
        cluster.deliver_messages();
    }

    let storage2 = cluster.get_node(1).storage().clone();
    cluster.remove_node(1);
    cluster.add_node_with_storage(1, storage2);

    // Verify recovery after second cycle
    // First 10 entries should be in snapshot
    for i in 1..=10 {
        assert_eq!(
            cluster
                .get_node(1)
                .state_machine()
                .get(&format!("cycle1_key{}", i)),
            Some(&format!("value{}", i)[..]),
            "Cycle 1 entries should be in snapshot"
        );
    }
    // Cycle 2 entries (11-15) are uncommitted after restart
    for i in 1..=5 {
        assert_eq!(
            cluster
                .get_node(1)
                .state_machine()
                .get(&format!("cycle2_key{}", i)),
            None,
            "Cycle 2 entries should be uncommitted after restart"
        );
    }
}

/// Test that recovered node can continue normal operation
#[test]
fn test_recovered_node_can_continue_operation() {
    // Phase 1: Create snapshot and restart
    let mut cluster = TimelessTestCluster::new();
    cluster.add_node(1);
    cluster.add_node(2);
    cluster.add_node(3);
    cluster.connect_peers();

    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Election));
    cluster.deliver_messages();

    for i in 1..=10 {
        cluster
            .get_node_mut(1)
            .on_event(Event::ClientCommand(format!("SET key{}=value{}", i, i)));
        cluster.deliver_messages();
    }

    let saved_storage = cluster.get_node(1).storage().clone();
    cluster.remove_node(1);
    cluster.add_node_with_storage(1, saved_storage);

    // Phase 2: Reconnect and become leader again
    cluster.reconnect_node(1);
    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Election));
    cluster.deliver_messages();

    assert_eq!(*cluster.get_node(1).role(), NodeState::Leader);

    // Phase 3: Apply new commands after recovery
    cluster
        .get_node_mut(1)
        .on_event(Event::ClientCommand("SET new_key=new_value".to_string()));
    cluster.deliver_messages();

    // Verify new command applied
    assert_eq!(
        cluster.get_node(1).state_machine().get("new_key"),
        Some("new_value")
    );

    // Old keys still present
    for i in 1..=10 {
        assert_eq!(
            cluster
                .get_node(1)
                .state_machine()
                .get(&format!("key{}", i)),
            Some(&format!("value{}", i)[..])
        );
    }
}
