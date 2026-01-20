// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use raft_core::{
    event::Event, node_state::NodeState, state_machine::StateMachine, storage::Storage,
    timer_service::TimerKind,
};
use raft_sim::timeless_test_cluster::TimelessTestCluster;

#[test]
fn test_liveness_single_node_cluster() {
    // Arrange - 1 Node Cluster
    let mut cluster = TimelessTestCluster::with_nodes(1);

    // Act - Start election
    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Election));

    // Deliver messages (self-vote)
    cluster.deliver_messages();

    // Assert - Should become leader immediately (majority of 1 is 1)
    assert_eq!(*cluster.get_node(1).role(), NodeState::Leader);
    assert_eq!(cluster.get_node(1).current_term(), 1);

    // Act - Submit Command "SET k=v"
    cluster
        .get_node_mut(1)
        .on_event(Event::ClientCommand("SET k=v".to_string()));

    // Deliver
    cluster.deliver_messages();

    // Assert - Log should be committed and applied
    assert_eq!(cluster.get_node(1).storage().last_log_index(), 1);

    // Assert Commit Index
    // If this fails, we need to add single-node commit logic in `submit_client_command`
    // because no AppendEntries response will ever trigger commit advancement.
    assert_eq!(
        cluster.get_node(1).commit_index(),
        1,
        "Commit index should advance on single node"
    );

    // Check State Machine
    let sm = cluster.get_node(1).state_machine();
    assert_eq!(sm.get("k"), Some("v"));
}
