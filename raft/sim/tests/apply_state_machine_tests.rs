// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use raft_core::{event::Event, state_machine::StateMachine, storage::Storage, timer_service::TimerKind};
use raft_sim::timeless_test_cluster::TimelessTestCluster;

#[test]
fn test_state_machine_apply() {
    // Arrange
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

    // Act - Send multiple commands
    cluster
        .get_node_mut(1)
        .on_event(Event::ClientCommand("SET x=1".to_string()));
    cluster.deliver_messages();
    cluster
        .get_node_mut(1)
        .on_event(Event::ClientCommand("SET y=2".to_string()));
    cluster.deliver_messages();

    // Assert - State machine applied both commands
    assert_eq!(cluster.get_node(1).state_machine().get("x"), Some("1"));
    assert_eq!(cluster.get_node(1).state_machine().get("y"), Some("2"));
    println!(
        "Leader commit_index: {}",
        cluster.get_node(1).commit_index()
    );

    // Followers also applied (after commit propagation)
    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Heartbeat));
    cluster.deliver_messages();

    println!(
        "Node 2 last_log_index: {}",
        cluster.get_node(2).storage().last_log_index()
    );
    println!(
        "Node 3 last_log_index: {}",
        cluster.get_node(3).storage().last_log_index()
    );

    // Print all entries in follower logs
    for i in 1..=cluster.get_node(2).storage().last_log_index() {
        if let Some(entry) = cluster.get_node(2).storage().get_entry(i) {
            println!(
                "Node 2 entry {}: term={}, payload={:?}",
                i, entry.term, entry.payload
            );
        }
    }

    for i in 1..=cluster.get_node(3).storage().last_log_index() {
        if let Some(entry) = cluster.get_node(3).storage().get_entry(i) {
            println!(
                "Node 3 entry {}: term={}, payload={:?}",
                i, entry.term, entry.payload
            );
        }
    }

    println!(
        "Node 2 x={:?}, y={:?}",
        cluster.get_node(2).state_machine().get("x"),
        cluster.get_node(2).state_machine().get("y")
    );
    println!(
        "Node 3 x={:?}, y={:?}",
        cluster.get_node(3).state_machine().get("x"),
        cluster.get_node(3).state_machine().get("y")
    );

    assert_eq!(cluster.get_node(2).state_machine().get("x"), Some("1"));
    assert_eq!(cluster.get_node(3).state_machine().get("y"), Some("2"));
}
