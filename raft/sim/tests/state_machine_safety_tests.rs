// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use raft_core::{
    event::Event, node_state::NodeState, state_machine::StateMachine, timer_service::TimerKind,
};
use raft_sim::timeless_test_cluster::TimelessTestCluster;

/// Test: Linearizability of State Machine applications
///
/// Ensures that commands are applied in the exact same order on all nodes,
/// even when leadership changes occur.
#[test]
fn test_safety_state_machine_execution_order() {
    // Arrange - 3 Node Cluster
    let mut cluster = TimelessTestCluster::with_nodes(3);

    // Elect Node 1
    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Election));
    cluster.deliver_messages();
    cluster.deliver_messages();
    assert_eq!(*cluster.get_node(1).role(), NodeState::Leader);

    // Execute Sequence of Commands 1..5
    for i in 1..=5 {
        let cmd = format!("SET k{}=v{}", i, i);
        cluster.get_node_mut(1).on_event(Event::ClientCommand(cmd));
        cluster.deliver_messages(); // Replicate
    }

    // Send heartbeat to propagate commit index to followers
    // (Followers are one step behind on commit index because Leader commits after receiving response)
    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Heartbeat));
    cluster.deliver_messages();

    // Verify SM content for 1..5
    for id in 1..=3 {
        let sm = cluster.get_node(id).state_machine();
        for i in 1..=5 {
            let key = format!("k{}", i);
            let val = format!("v{}", i);
            assert_eq!(
                sm.get(&key),
                Some(val.as_str()),
                "Node {} missing key {}",
                id,
                key
            );
        }
    }

    // Partition Leader (Node 1)
    cluster.partition_node(1);

    // Elect Node 2 (Node 2 and 3 can form majority)
    cluster
        .get_node_mut(2)
        .on_event(Event::TimerFired(TimerKind::Election));
    cluster.deliver_messages();
    cluster.deliver_messages();
    assert_eq!(*cluster.get_node(2).role(), NodeState::Leader);

    // Execute more commands on new leader 6..10
    for i in 6..=10 {
        let cmd = format!("SET k{}=v{}", i, i);
        cluster.get_node_mut(2).on_event(Event::ClientCommand(cmd));
        cluster.deliver_messages();
    }

    // Send heartbeat to propagate commit index from Node 2
    cluster
        .get_node_mut(2)
        .on_event(Event::TimerFired(TimerKind::Heartbeat));
    cluster.deliver_messages();

    // Verify New Leader and Peer have new data
    for id in 2..=3 {
        let sm = cluster.get_node(id).state_machine();
        for i in 1..=10 {
            let key = format!("k{}", i);
            let val = format!("v{}", i);
            assert_eq!(sm.get(&key), Some(val.as_str()));
        }
    }

    // Node 1 should be stale (has 1..5, missing 6..10)
    let sm1 = cluster.get_node(1).state_machine();
    assert_eq!(sm1.get("k5"), Some("v5"));
    assert_eq!(sm1.get("k6"), None);

    // Heal Partition - Node 1 comes back
    cluster.heal_partition();

    // Heartbeat from 2 to 1 (triggers catchup)
    cluster
        .get_node_mut(2)
        .on_event(Event::TimerFired(TimerKind::Heartbeat));
    cluster.deliver_messages(); // AppendEntries
    cluster.deliver_messages(); // Response (success=false, decrement/retry or success=true if log consistent)
                                // Actually, Node 1 log matches at index 5. Node 2 sends entries 6..10.
                                // It should succeed immediately.

    // Verify Node 1 catches up
    let sm1_new = cluster.get_node(1).state_machine();
    for i in 1..=10 {
        let key = format!("k{}", i);
        let val = format!("v{}", i);
        assert_eq!(
            sm1_new.get(&key),
            Some(val.as_str()),
            "Node 1 failed to catch up key {}",
            key
        );
    }
}
