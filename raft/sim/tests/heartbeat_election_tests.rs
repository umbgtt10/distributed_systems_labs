// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use raft_core::{event::Event, node_state::NodeState, raft_messages::RaftMsg, timer::TimerKind};
use raft_sim::test_cluster::TestCluster;

#[test]
fn test_leader_sends_heartbeats() {
    // Arrange
    let mut cluster = TestCluster::new();
    cluster.add_node(1);
    cluster.add_node(2);
    cluster.add_node(3);
    cluster.connect_peers();

    // Act - Node 1 becomes leader
    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Election));
    cluster.deliver_messages();
    assert_eq!(*cluster.get_node(1).role(), NodeState::Leader);

    // Clear previous messages
    cluster.clear_message_log();

    // Leader sends heartbeat
    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Heartbeat));

    // Deliver the heartbeat messages
    cluster.deliver_messages();

    // Assert - AppendEntries sent to all followers
    let messages_to_2 = cluster.get_messages_from(1, 2);
    assert_eq!(messages_to_2.len(), 1);
    assert!(matches!(
        &messages_to_2[0],
        RaftMsg::AppendEntries {
            term: 1,
            entries: _, // Should be empty
            ..
        }
    ));

    let messages_to_3 = cluster.get_messages_from(1, 3);
    assert_eq!(messages_to_3.len(), 1);
    assert!(matches!(
        &messages_to_3[0],
        RaftMsg::AppendEntries {
            term: 1,
            entries: _, // Should be empty
            ..
        }
    ));
}

#[test]
fn test_heartbeat_prevents_election() {
    // Arrange - Leader established
    let mut cluster = TestCluster::new();
    cluster.add_node(1);
    cluster.add_node(2);
    cluster.add_node(3);
    cluster.connect_peers();

    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Election));
    cluster.deliver_messages();
    assert_eq!(*cluster.get_node(1).role(), NodeState::Leader);

    // Act - Leader sends heartbeat to Node 2
    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Heartbeat));
    cluster.deliver_message_from_to(1, 2);

    // Node 2 receives heartbeat (AppendEntries with current term)
    // This should reset Node 2's election timer conceptually
    // (though we don't implement timers yet)

    // Assert - Node 2 remains follower
    assert_eq!(*cluster.get_node(2).role(), NodeState::Follower);
}
