// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use raft_core::{
    event::Event, node_state::NodeState, raft_messages::RaftMsg, timer_service::TimerKind,
};
use raft_sim::{
    in_memory_chunk_collection::InMemoryChunkCollection, timeless_test_cluster::TimelessTestCluster,
};

/// Test that a candidate receiving InstallSnapshot at the current term steps down
#[test]
fn test_safety_candidate_steps_down_on_install_snapshot_same_term() {
    // Arrange - 3 node cluster
    let mut cluster = TimelessTestCluster::with_nodes(3);

    // Node 2 times out and starts pre-vote
    cluster
        .get_node_mut(2)
        .on_event(Event::TimerFired(TimerKind::Election));

    // Deliver pre-vote requests to Nodes 1 and 3
    cluster.deliver_message_from_to(2, 1);
    cluster.deliver_message_from_to(2, 3);

    // Deliver pre-vote responses back to Node 2 (pre-vote succeeds)
    cluster.deliver_message_from_to(1, 2);
    cluster.deliver_message_from_to(3, 2);

    // Node 2 should now be Candidate (real election started)
    assert_eq!(*cluster.get_node(2).role(), NodeState::Candidate);
    let candidate_term = cluster.get_node(2).current_term();

    // Inject an InstallSnapshot from Node 1 at the same term
    let snapshot_chunk = InMemoryChunkCollection::from_vec(vec![1, 2, 3]);
    let install_snapshot = RaftMsg::InstallSnapshot {
        term: candidate_term,
        leader_id: 1,
        last_included_index: 1,
        last_included_term: candidate_term,
        offset: 0,
        data: snapshot_chunk,
        done: true,
    };

    cluster.get_node_mut(2).on_event(Event::Message {
        from: 1,
        msg: install_snapshot,
    });

    // Assert - Candidate steps down to follower
    assert_eq!(*cluster.get_node(2).role(), NodeState::Follower);
    assert_eq!(cluster.get_node(2).current_term(), candidate_term);
}
