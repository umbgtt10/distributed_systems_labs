// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use raft_core::{
    event::Event, node_state::NodeState, raft_messages::RaftMsg, state_machine::StateMachine,
    storage::Storage, timer_service::TimerKind,
};
use raft_sim::timeless_test_cluster::TimelessTestCluster;

/// Test that conflicting entries after snapshot are truncated during leader changes
#[test]
fn test_safety_truncate_conflicts_after_snapshot_with_partitioned_election() {
    // Arrange - 3 node cluster with low snapshot threshold
    let mut cluster = TimelessTestCluster::new().with_snapshot_threshold(3);
    for id in 1..=3 {
        cluster.add_node(id);
    }
    cluster.connect_peers();

    // Node 1 becomes leader
    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Election));
    cluster.deliver_messages();
    cluster.deliver_messages();

    assert_eq!(*cluster.get_node(1).role(), NodeState::Leader);

    // Commit 3 entries to trigger snapshot
    for i in 1..=3 {
        cluster
            .get_node_mut(1)
            .on_event(Event::ClientCommand(format!("SET x={}", i)));
        cluster.deliver_messages();
    }

    assert!(cluster.get_node(1).storage().load_snapshot().is_some());
    assert_eq!(cluster.get_node(1).storage().first_log_index(), 4);

    // Partition the cluster: minority [1], majority [2,3]
    cluster.partition(&[1], &[2, 3]);

    // Minority leader appends conflicting entries (uncommitted)
    cluster
        .get_node_mut(1)
        .on_event(Event::ClientCommand("SET x=minority_4".to_string()));
    cluster.deliver_messages_partition(&[1]);

    cluster
        .get_node_mut(1)
        .on_event(Event::ClientCommand("SET x=minority_5".to_string()));
    cluster.deliver_messages_partition(&[1]);

    assert_eq!(cluster.get_node(1).storage().last_log_index(), 5);
    assert_eq!(cluster.get_node(1).commit_index(), 3);

    // Majority elects new leader (Node 2)
    cluster
        .get_node_mut(2)
        .on_event(Event::TimerFired(TimerKind::Election));
    cluster.deliver_messages_partition(&[2, 3]);

    assert_eq!(*cluster.get_node(2).role(), NodeState::Leader);

    // Majority leader commits a conflicting entry at index 4
    cluster
        .get_node_mut(2)
        .on_event(Event::ClientCommand("SET x=majority_4".to_string()));
    cluster.deliver_messages_partition(&[2, 3]);

    assert_eq!(cluster.get_node(2).commit_index(), 4);
    // New leader should have created a snapshot at index 4
    let leader_snapshot = cluster.get_node(2).storage().load_snapshot();
    assert!(leader_snapshot.is_some());
    let leader_snapshot = leader_snapshot.unwrap();

    // Heal partition and ensure old leader steps down
    cluster.heal_partition();
    cluster
        .get_node_mut(2)
        .on_event(Event::TimerFired(TimerKind::Heartbeat));
    cluster.deliver_messages();

    // Send InstallSnapshot from new leader to old leader
    let leader_term = cluster.get_node(2).current_term();
    let snapshot_chunk = cluster
        .get_node(2)
        .storage()
        .get_snapshot_chunk(0, usize::MAX)
        .expect("Leader snapshot chunk should exist");

    let install_snapshot = RaftMsg::InstallSnapshot {
        term: leader_term,
        leader_id: 2,
        last_included_index: leader_snapshot.metadata.last_included_index,
        last_included_term: leader_snapshot.metadata.last_included_term,
        offset: snapshot_chunk.offset as u64,
        data: snapshot_chunk.data,
        done: snapshot_chunk.done,
    };

    cluster.get_node_mut(1).on_event(Event::Message {
        from: 2,
        msg: install_snapshot,
    });

    // Old leader should step down and truncate conflicting entries
    assert_eq!(*cluster.get_node(1).role(), NodeState::Follower);
    assert_eq!(cluster.get_node(1).storage().first_log_index(), 5);
    assert!(cluster.get_node(1).storage().get_entry(4).is_none());

    // State machines should reflect the committed majority value
    assert_eq!(
        cluster.get_node(1).state_machine().get("x"),
        Some("majority_4")
    );
}
