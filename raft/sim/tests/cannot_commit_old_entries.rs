use raft_core::{
    event::Event, node_state::NodeState, state_machine::StateMachine, storage::Storage,
    timer::TimerKind,
};
use raft_sim::test_cluster::TestCluster;

#[test]
fn test_cannot_commit_old_term_entry() {
    // Arrange - Node 1 is leader in term 1, replicates entry to node 2 only
    let mut cluster = TestCluster::new();
    cluster.add_node(1);
    cluster.add_node(2);
    cluster.add_node(3);
    cluster.add_node(4);
    cluster.add_node(5);
    cluster.connect_peers();

    // Node 1 becomes leader in term 1
    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Election));
    cluster.deliver_messages();
    assert_eq!(*cluster.get_node(1).role(), NodeState::Leader);
    assert_eq!(cluster.get_node(1).current_term(), 1);

    // Simulate: Leader writes entry but only node 2 receives it (nodes 3,4,5 partitioned)
    // This means only 2 out of 5 nodes have the entry - NO MAJORITY
    cluster.partition(&[1, 2], &[3, 4, 5]);

    cluster
        .get_node_mut(1)
        .on_event(Event::ClientCommand("SET x=1".to_string()));
    cluster.deliver_messages_partition(&[1, 2]);

    // Entry replicated but NOT committed (only 2/5, need 3/5)
    assert_eq!(cluster.get_node(1).storage().last_log_index(), 1);
    assert_eq!(cluster.get_node(2).storage().last_log_index(), 1);
    assert_eq!(cluster.get_node(1).commit_index(), 0); // NOT committed

    // Node 1 crashes, partition heals
    cluster.remove_node(1);
    cluster.heal_partition();
    cluster.connect_peers(); // Update peers after node 1 removed

    // Node 2 (which HAS the entry) becomes leader in term 2
    cluster
        .get_node_mut(2)
        .on_event(Event::TimerFired(TimerKind::Election));
    cluster.deliver_messages();

    assert_eq!(*cluster.get_node(2).role(), NodeState::Leader);
    assert_eq!(cluster.get_node(2).current_term(), 2);

    // Node 2 replicates the old entry from term 1 to all followers
    cluster
        .get_node_mut(2)
        .on_event(Event::TimerFired(TimerKind::Heartbeat));
    cluster.deliver_messages();

    // Now the old entry is on majority (nodes 2, 3, 4, 5)
    assert_eq!(cluster.get_node(2).storage().last_log_index(), 1);
    assert_eq!(cluster.get_node(3).storage().last_log_index(), 1);
    assert_eq!(cluster.get_node(4).storage().last_log_index(), 1);
    assert_eq!(cluster.get_node(5).storage().last_log_index(), 1);

    // But it should NOT be committed yet (it's from term 1, we're in term 2)
    assert_eq!(cluster.get_node(2).commit_index(), 0); // Still 0!
    assert_eq!(cluster.get_node(3).commit_index(), 0);
    assert_eq!(cluster.get_node(4).commit_index(), 0);

    // Only when leader writes a NEW entry in term 2 can old entries be committed
    cluster
        .get_node_mut(2)
        .on_event(Event::ClientCommand("SET y=2".to_string()));
    cluster.deliver_messages();

    // Send heartbeat to propagate commit_index to followers
    cluster
        .get_node_mut(2)
        .on_event(Event::TimerFired(TimerKind::Heartbeat));
    cluster.deliver_messages();

    // NOW both entries are committed (old + new)
    assert_eq!(cluster.get_node(2).commit_index(), 2);
    assert_eq!(cluster.get_node(3).commit_index(), 2);
    assert_eq!(cluster.get_node(4).commit_index(), 2);
    assert_eq!(cluster.get_node(5).commit_index(), 2);

    // Verify both entries are applied to state machine
    assert_eq!(cluster.get_node(2).state_machine().get("x"), Some("1"));
    assert_eq!(cluster.get_node(2).state_machine().get("y"), Some("2"));
    assert_eq!(cluster.get_node(3).state_machine().get("x"), Some("1"));
    assert_eq!(cluster.get_node(3).state_machine().get("y"), Some("2"));
}
