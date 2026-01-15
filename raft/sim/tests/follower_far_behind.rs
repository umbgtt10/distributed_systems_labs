use raft_core::{
    event::Event, node_state::NodeState, state_machine::StateMachine, storage::Storage,
    timer_service::TimerKind,
};
use raft_sim::{in_memory_storage::InMemoryStorage, timeless_test_cluster::TimelessTestCluster};

#[test]
fn test_follower_far_behind() {
    // Arrange - Leader has entries 1-5, follower has only 1-2
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

    // Leader writes 5 entries, all nodes receive them
    for i in 1..=5 {
        cluster
            .get_node_mut(1)
            .on_event(Event::ClientCommand(format!("SET x={}", i)));
        cluster.deliver_messages();
    }

    assert_eq!(cluster.get_node(1).storage().last_log_index(), 5);
    assert_eq!(cluster.get_node(2).storage().last_log_index(), 5);
    assert_eq!(cluster.get_node(3).storage().last_log_index(), 5);

    // Simulate node 2 crashing and losing entries 3-5 (simulate incomplete write)
    let mut storage_node2 = InMemoryStorage::new();
    for i in 1..=2 {
        if let Some(entry) = cluster.get_node(1).storage().get_entry(i) {
            storage_node2.append_entries(&[entry]);
        }
    }

    cluster.remove_node(2);
    cluster.add_node_with_storage(2, storage_node2);
    cluster.connect_peers();

    assert_eq!(cluster.get_node(1).storage().last_log_index(), 5);
    assert_eq!(cluster.get_node(2).storage().last_log_index(), 2);

    // Leader sends heartbeats - follower should catch up
    for _ in 0..10 {
        cluster
            .get_node_mut(1)
            .on_event(Event::TimerFired(TimerKind::Heartbeat));
        cluster.deliver_messages();

        if cluster.get_node(2).storage().last_log_index() == 5 {
            break;
        }
    }

    // Node 2 should now have all 5 entries
    assert_eq!(cluster.get_node(2).storage().last_log_index(), 5);

    for i in 1..=5 {
        let entry = cluster.get_node(2).storage().get_entry(i).unwrap();
        assert_eq!(entry.payload, format!("SET x={}", i));
    }

    // Verify commit and state machine
    assert_eq!(cluster.get_node(2).commit_index(), 5);
    assert_eq!(cluster.get_node(2).state_machine().get("x"), Some("5"));
}
