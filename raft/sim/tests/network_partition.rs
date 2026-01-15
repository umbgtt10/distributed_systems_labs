use raft_core::{event::Event, node_state::NodeState, state_machine::StateMachine, storage::Storage, timer::TimerKind};
use raft_sim::test_cluster::TestCluster;

#[test]
fn test_network_partition_recovery() {
    // Arrange - Create 5-node cluster
    let mut cluster = TestCluster::new();
    cluster.add_node(1);
    cluster.add_node(2);
    cluster.add_node(3);
    cluster.add_node(4);
    cluster.add_node(5);
    cluster.connect_peers();

    // Node 1 becomes leader
    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Election));
    cluster.deliver_messages();
    assert_eq!(*cluster.get_node(1).role(), NodeState::Leader);

    // Leader replicates an entry
    cluster
        .get_node_mut(1)
        .on_event(Event::ClientCommand("SET x=1".to_string()));
    cluster.deliver_messages();

    // All nodes have entry 1
    assert_eq!(cluster.get_node(1).storage().last_log_index(), 1);
    assert_eq!(cluster.get_node(5).storage().last_log_index(), 1);

    // Act - Simulate network partition: [1, 2] vs [3, 4, 5]
    cluster.partition(&[1, 2], &[3, 4, 5]);

    // Minority partition (1, 2) tries to make progress
    cluster
        .get_node_mut(1)
        .on_event(Event::ClientCommand("SET y=2".to_string()));
    cluster.deliver_messages_partition(&[1, 2]); // Only deliver within minority

    // Node 1 appends locally but cannot commit (no majority)
    assert_eq!(cluster.get_node(1).storage().last_log_index(), 2);
    assert_eq!(cluster.get_node(1).commit_index(), 1); // Still 1 (no majority)

    // Majority partition (3, 4, 5) elects new leader
    cluster
        .get_node_mut(3)
        .on_event(Event::TimerFired(TimerKind::Election));
    cluster.deliver_messages_partition(&[3, 4, 5]); // Only deliver within majority

    assert_eq!(*cluster.get_node(3).role(), NodeState::Leader);

    // New leader writes and commits entry in majority partition
    cluster
        .get_node_mut(3)
        .on_event(Event::ClientCommand("SET z=3".to_string()));
    cluster.deliver_messages_partition(&[3, 4, 5]);

    // Majority partition commits the new entry
    assert_eq!(cluster.get_node(3).storage().last_log_index(), 2);
    assert_eq!(cluster.get_node(3).commit_index(), 2); // Committed

    // Heal partition - reconnect all nodes
    cluster.heal_partition();

    // Old leader (node 1) receives heartbeat from new leader (node 3)
    cluster
        .get_node_mut(3)
        .on_event(Event::TimerFired(TimerKind::Heartbeat));
    cluster.deliver_messages();

    // Assert - Node 1 steps down and syncs with new leader
    assert_eq!(*cluster.get_node(1).role(), NodeState::Follower);

    // Node 1's uncommitted entry is overwritten
    assert_eq!(cluster.get_node(1).storage().last_log_index(), 2);
    let entry2_node1 = cluster.get_node(1).storage().get_entry(2).unwrap();
    assert_eq!(entry2_node1.payload, "SET z=3"); // Overwritten

    // All nodes have consistent logs
    for node_id in 1..=5 {
        assert_eq!(cluster.get_node(node_id).storage().last_log_index(), 2);
        assert_eq!(cluster.get_node(node_id).commit_index(), 2);

        let entry1 = cluster.get_node(node_id).storage().get_entry(1).unwrap();
        assert_eq!(entry1.payload, "SET x=1");

        let entry2 = cluster.get_node(node_id).storage().get_entry(2).unwrap();
        assert_eq!(entry2.payload, "SET z=3");
    }

    // State machines are consistent
    for node_id in 1..=5 {
        assert_eq!(
            cluster.get_node(node_id).state_machine().get("x"),
            Some("1")
        );
        assert_eq!(
            cluster.get_node(node_id).state_machine().get("z"),
            Some("3")
        );
        assert_eq!(cluster.get_node(node_id).state_machine().get("y"), None); // Uncommitted
    }
}
