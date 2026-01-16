// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use raft_core::{
    event::Event, node_state::NodeState, raft_messages::RaftMsg, timer_service::TimerKind,
};
use raft_sim::timeless_test_cluster::TimelessTestCluster;

#[test]
fn test_vote_rejection_already_voted() {
    // Arrange
    let mut cluster = TimelessTestCluster::new();
    cluster.add_node(1);
    cluster.add_node(2);
    cluster.add_node(3);
    cluster.connect_peers();

    // Act - Node 1 becomes candidate first
    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Election));

    // Node 3 votes for Node 1
    cluster.deliver_message_from_to(1, 3);

    // Now Node 2 becomes candidate
    cluster
        .get_node_mut(2)
        .on_event(Event::TimerFired(TimerKind::Election));

    // Deliver Node 2's RequestVote to Node 3
    cluster.deliver_message_from_to(2, 3);

    // Deliver remaining messages
    cluster.deliver_messages();

    // Node 1 becomes leader (got votes from Node 3 + self)
    assert_eq!(*cluster.get_node(1).role(), NodeState::Leader);
    assert_eq!(*cluster.get_node(2).role(), NodeState::Candidate);

    // Assert - Node 3 rejected Node 2's vote request
    let messages_to_2 = cluster.get_messages(2);
    let rejection = messages_to_2
        .iter()
        .find(|msg| matches!(msg, RaftMsg::RequestVoteResponse { .. }))
        .unwrap();

    assert!(matches!(
        rejection,
        RaftMsg::RequestVoteResponse {
            term: 1,
            vote_granted: false
        }
    ));
}
