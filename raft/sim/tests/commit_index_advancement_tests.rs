// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use raft_core::{event::Event, node_state::NodeState, timer::TimerKind};
use raft_sim::test_cluster::TestCluster;

#[test]
fn test_commit_index_advancement() {
    // Arrange
    let mut cluster = TestCluster::new();
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

    // Act - Client sends command
    cluster
        .get_node_mut(1)
        .on_event(Event::ClientCommand("SET x=1".to_string()));
    cluster.deliver_messages();

    // Assert - Leader's commit_index advances to 1 (majority replicated)
    assert_eq!(cluster.get_node(1).commit_index(), 1);

    // Followers' commit_index also advances (from next AppendEntries/heartbeat)
    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Heartbeat));
    cluster.deliver_messages();

    assert_eq!(cluster.get_node(2).commit_index(), 1);
    assert_eq!(cluster.get_node(3).commit_index(), 1);
}
