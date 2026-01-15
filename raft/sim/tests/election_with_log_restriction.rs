// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use raft_core::{event::Event, node_state::NodeState, storage::Storage, timer_service::TimerKind};
use raft_sim::{in_memory_storage::InMemoryStorage, timeless_test_cluster::TimelessTestCluster};

#[test]
fn test_election_log_restriction() {
    // Arrange - Create cluster with different log lengths
    let mut cluster = TimelessTestCluster::new();
    cluster.add_node(1);
    cluster.add_node(2);
    cluster.add_node(3);
    cluster.connect_peers();

    // Node 1 becomes leader and replicates entries
    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Election));
    cluster.deliver_messages();
    assert_eq!(*cluster.get_node(1).role(), NodeState::Leader);

    // Leader writes 3 entries
    cluster
        .get_node_mut(1)
        .on_event(Event::ClientCommand("SET a=1".to_string()));
    cluster.deliver_messages();

    cluster
        .get_node_mut(1)
        .on_event(Event::ClientCommand("SET b=2".to_string()));
    cluster.deliver_messages();

    cluster
        .get_node_mut(1)
        .on_event(Event::ClientCommand("SET c=3".to_string()));
    cluster.deliver_messages();

    // All nodes should have 3 entries
    assert_eq!(cluster.get_node(1).storage().last_log_index(), 3);
    assert_eq!(cluster.get_node(2).storage().last_log_index(), 3);
    assert_eq!(cluster.get_node(3).storage().last_log_index(), 3);

    // Simulate node 3 missing the last entry
    let mut storage_node3 = InMemoryStorage::new();
    for i in 1..=2 {
        if let Some(entry) = cluster.get_node(3).storage().get_entry(i) {
            storage_node3.append_entries(&[entry]);
        }
    }

    // Replace node 3 with truncated log
    cluster.remove_node(3);
    cluster.add_node_with_storage(3, storage_node3);
    cluster.connect_peers();

    assert_eq!(cluster.get_node(3).storage().last_log_index(), 2);

    // Act - Node 3 (with shorter log) attempts election
    cluster
        .get_node_mut(3)
        .on_event(Event::TimerFired(TimerKind::Election));
    cluster.deliver_messages();

    // Assert - Node 3 should NOT become leader (log not up-to-date)
    // Nodes 1 and 2 should reject the vote
    assert_eq!(*cluster.get_node(3).role(), NodeState::Candidate);

    // Node 2 (with complete log) attempts election
    cluster
        .get_node_mut(2)
        .on_event(Event::TimerFired(TimerKind::Election));
    cluster.deliver_messages();

    // Node 2 should win election (log is up-to-date)
    assert_eq!(*cluster.get_node(2).role(), NodeState::Leader);
}
