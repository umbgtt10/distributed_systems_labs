use raft_core::{event::Event, node_state::NodeState, timer::TimerKind};
use raft_sim::test_cluster::TestCluster;

#[test]
fn test_leader_steps_down_on_higher_term() {
    let mut cluster = TestCluster::new();
    cluster.add_node(1);
    cluster.add_node(2);
    cluster.add_node(3);
    cluster.connect_peers();

    // Node 1 becomes leader in term 1
    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Election));
    cluster.deliver_messages();
    assert_eq!(*cluster.get_node(1).role(), NodeState::Leader);
    assert_eq!(cluster.get_node(1).current_term(), 1);

    // Partition: [1] vs [2, 3]
    cluster.partition(&[1], &[2, 3]);

    // Node 2 becomes leader in term 2 (in majority partition)
    cluster
        .get_node_mut(2)
        .on_event(Event::TimerFired(TimerKind::Election));
    cluster.deliver_messages_partition(&[2, 3]);
    assert_eq!(*cluster.get_node(2).role(), NodeState::Leader);
    assert_eq!(cluster.get_node(2).current_term(), 2);

    // Heal partition - node 1 receives heartbeat from node 2
    cluster.heal_partition();
    cluster
        .get_node_mut(2)
        .on_event(Event::TimerFired(TimerKind::Heartbeat));
    cluster.deliver_messages();

    // Node 1 should step down to follower and update term
    assert_eq!(*cluster.get_node(1).role(), NodeState::Follower);
    assert_eq!(cluster.get_node(1).current_term(), 2);
}
