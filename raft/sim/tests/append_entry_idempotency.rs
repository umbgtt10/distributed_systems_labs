use raft_core::{event::Event, log_entry::EntryType, node_state::NodeState, storage::Storage, timer_service::TimerKind};
use raft_sim::timeless_test_cluster::TimelessTestCluster;

#[test]
fn test_safety_append_entries_idempotency() {
    let mut cluster = TimelessTestCluster::new();
    cluster.add_node(1);
    cluster.add_node(2);
    cluster.connect_peers();

    // Node 1 becomes leader
    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Election));
    cluster.deliver_messages();
    assert_eq!(*cluster.get_node(1).role(), NodeState::Leader);

    // Send command
    cluster
        .get_node_mut(1)
        .on_event(Event::ClientCommand("SET x=1".to_string()));
    cluster.deliver_messages();

    assert_eq!(cluster.get_node(2).storage().last_log_index(), 1);
    let entry = cluster.get_node(2).storage().get_entry(1).unwrap();
    if let EntryType::Command(ref p) = entry.entry_type {
        assert_eq!(p, "SET x=1");
    } else {
        panic!("Expected Command entry");
    }

    // Simulate duplicate AppendEntries (network retransmission)
    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Heartbeat));
    cluster.deliver_messages();

    // Should still have exactly 1 entry (not duplicated)
    assert_eq!(cluster.get_node(2).storage().last_log_index(), 1);
    let entry = cluster.get_node(2).storage().get_entry(1).unwrap();
    if let EntryType::Command(ref p) = entry.entry_type {
        assert_eq!(p, "SET x=1");
    }

    // Send another heartbeat
    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Heartbeat));
    cluster.deliver_messages();

    // Still exactly 1 entry
    assert_eq!(cluster.get_node(2).storage().last_log_index(), 1);
}
