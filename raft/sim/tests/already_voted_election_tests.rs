// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use raft_core::{event::Event, node_state::NodeState, timer_service::TimerKind};
use raft_sim::timeless_test_cluster::TimelessTestCluster;

#[test]
fn test_safety_vote_rejection_already_voted() {
    // Arrange
    let mut cluster = TimelessTestCluster::new();
    cluster.add_node(1);
    cluster.add_node(2);
    cluster.add_node(3);
    cluster.connect_peers();

    // Act - Node 1 starts election (sends pre-vote)
    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Election));

    // Deliver all of Node 1's messages (pre-vote + real vote)
    cluster.deliver_messages();

    // Node 1 should become leader at term 1
    assert_eq!(*cluster.get_node(1).role(), NodeState::Leader);
    assert_eq!(cluster.get_node(1).current_term(), 1);

    // Now Node 2 attempts election
    cluster
        .get_node_mut(2)
        .on_event(Event::TimerFired(TimerKind::Election));

    // Deliver Node 2's messages
    cluster.deliver_messages();

    // With pre-vote: Node 2 wins pre-vote at term 1, then starts real election at term 2
    // At term 2, all nodes can vote for Node 2 (voted_for resets on new term)
    // So Node 2 becomes leader at term 2
    assert_eq!(*cluster.get_node(2).role(), NodeState::Leader);
    assert_eq!(cluster.get_node(2).current_term(), 2);

    // Node 1 steps down to Follower when it sees term 2
    assert_eq!(*cluster.get_node(1).role(), NodeState::Follower);
    assert_eq!(cluster.get_node(1).current_term(), 2);
}
