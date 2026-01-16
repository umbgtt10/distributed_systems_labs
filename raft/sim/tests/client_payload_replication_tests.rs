// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use raft_core::{
    event::Event, node_state::NodeState, raft_messages::RaftMsg, timer_service::TimerKind,
};
use raft_sim::timeless_test_cluster::TimelessTestCluster;

#[test]
fn test_client_command_replication() {
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
    assert_eq!(*cluster.get_node(1).role(), NodeState::Leader);

    // Act - Client sends command to leader
    cluster.clear_message_log();
    cluster
        .get_node_mut(1)
        .on_event(Event::ClientCommand("SET x=1".to_string()));
    cluster.deliver_messages();

    // Assert - AppendEntries sent to followers
    let all_messages = cluster.get_all_messages();

    let append_entries_count = all_messages
        .iter()
        .filter(|(from, _to, msg)| *from == 1 && matches!(msg, RaftMsg::AppendEntries { .. }))
        .count();
    assert_eq!(append_entries_count, 2); // To nodes 2 and 3

    // Followers respond with success
    let response_count = all_messages
        .iter()
        .filter(|(_from, to, msg)| {
            *to == 1 && matches!(msg, RaftMsg::AppendEntriesResponse { success: true, .. })
        })
        .count();
    assert_eq!(response_count, 2); // From nodes 2 and 3
}
