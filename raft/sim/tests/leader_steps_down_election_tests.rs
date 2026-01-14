// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use raft_core::{event::Event, node_state::NodeState, storage::Storage, timer::TimerKind};
use raft_sim::test_cluster::TestCluster;

#[test]
fn test_leader_steps_down_on_higher_term() {
    // Arrange
    let mut cluster = TestCluster::new();
    cluster.add_node(1);
    cluster.add_node(2);
    cluster.add_node(3);
    cluster.connect_peers();

    // Act - Node 1 becomes leader at term 1
    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Election));
    cluster.deliver_messages();
    assert_eq!(*cluster.get_node(1).role(), NodeState::Leader);
    assert_eq!(cluster.get_node(1).current_term(), 1);

    // Node 2 starts election (advances to term 2)
    cluster
        .get_node_mut(2)
        .on_event(Event::TimerFired(TimerKind::Election));

    // Deliver Node 2's RequestVote to Node 1
    cluster.deliver_message_from_to(2, 1);

    // Assert - Node 1 steps down to Follower and updates term
    assert_eq!(*cluster.get_node(1).role(), NodeState::Follower);
    assert_eq!(cluster.get_node(1).current_term(), 2);
    assert_eq!(cluster.get_node(1).storage().voted_for(), Some(2)); // VotedFor reset

    // Node 2 becomes leader after getting majority
    cluster.deliver_messages();
    assert_eq!(*cluster.get_node(2).role(), NodeState::Leader);
    assert_eq!(cluster.get_node(2).current_term(), 2);
}
