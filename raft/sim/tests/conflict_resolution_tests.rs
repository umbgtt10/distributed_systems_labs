// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use raft_core::{
    event::Event, log_entry::LogEntry, node_state::NodeState, state_machine::StateMachine, storage::Storage, timer::TimerKind
};
use raft_sim::test_cluster::TestCluster;

#[test]
fn test_log_conflict_resolution() {
    // Arrange - Create cluster with divergent logs
    let mut cluster = TestCluster::new();
    cluster.add_node(1);
    cluster.add_node(2);
    cluster.add_node(3);
    cluster.connect_peers();

    // Node 1 becomes leader and replicates one entry
    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Election));
    cluster.deliver_messages();
    assert_eq!(*cluster.get_node(1).role(), NodeState::Leader);

    cluster
        .get_node_mut(1)
        .on_event(Event::ClientCommand("SET x=1".to_string()));
    cluster.deliver_messages();

    // All nodes have entry 1: "SET x=1"
    assert_eq!(cluster.get_node(1).storage().last_log_index(), 1);
    assert_eq!(cluster.get_node(2).storage().last_log_index(), 1);
    assert_eq!(cluster.get_node(3).storage().last_log_index(), 1);

    // Simulate network partition: node 2 isolated, node 1 fails
    // Node 3 becomes leader in term 2 and writes a conflicting entry

    // Manually inject conflicting entry into node 2 (simulating old leader write)
    let conflicting_entry = LogEntry {
        term: 1,
        payload: "SET x=99".to_string(), // Conflicting value
    };
    cluster
        .get_node_mut(2)
        .storage_mut()
        .append_entries(&[conflicting_entry]);

    assert_eq!(cluster.get_node(2).storage().last_log_index(), 2);

    // Node 3 becomes new leader in term 2
    cluster
        .get_node_mut(3)
        .on_event(Event::TimerFired(TimerKind::Election));
    cluster.deliver_messages();
    assert_eq!(*cluster.get_node(3).role(), NodeState::Leader);

    // New leader writes correct entry at index 2
    cluster
        .get_node_mut(3)
        .on_event(Event::ClientCommand("SET y=2".to_string()));
    cluster.deliver_messages();

    // Assert - Node 2's conflicting entry at index 2 is overwritten
    assert_eq!(cluster.get_node(2).storage().last_log_index(), 2);

    let entry2_node2 = cluster.get_node(2).storage().get_entry(2).unwrap();
    assert_eq!(entry2_node2.payload, "SET y=2"); // Overwritten with correct entry

    let entry2_node3 = cluster.get_node(3).storage().get_entry(2).unwrap();
    assert_eq!(entry2_node3.payload, "SET y=2");

    // State machines should match
    cluster
        .get_node_mut(3)
        .on_event(Event::TimerFired(TimerKind::Heartbeat));
    cluster.deliver_messages();

    assert_eq!(cluster.get_node(2).state_machine().get("x"), Some("1"));
    assert_eq!(cluster.get_node(2).state_machine().get("y"), Some("2"));
    assert_eq!(cluster.get_node(3).state_machine().get("x"), Some("1"));
    assert_eq!(cluster.get_node(3).state_machine().get("y"), Some("2"));
}
