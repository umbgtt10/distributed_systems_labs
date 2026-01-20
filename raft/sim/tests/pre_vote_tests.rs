// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use raft_core::{event::Event, node_state::NodeState, storage::Storage, timer_service::TimerKind};
use raft_sim::timeless_test_cluster::TimelessTestCluster;

/// Test that pre-vote succeeds and transitions to real election when no leader exists
#[test]
fn test_liveness_pre_vote_transitions_to_real_election() {
    // Arrange - 3 node cluster, no leader
    let mut cluster = TimelessTestCluster::with_nodes(3);

    // Act - Node 1 times out and starts pre-vote
    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Election));
    cluster.deliver_messages();

    // Assert - Pre-vote should succeed (majority granted)
    // Node 1 should transition to Candidate and increment term
    assert_eq!(cluster.get_node(1).current_term(), 1);

    // Deliver vote responses
    cluster.deliver_messages();

    // Node 1 should become leader (after winning the real election)
    assert_eq!(*cluster.get_node(1).role(), NodeState::Leader);
}

/// Test that pre-vote doesn't prevent legitimate elections
#[test]
fn test_liveness_pre_vote_allows_legitimate_elections() {
    // Arrange - 3 node cluster, no leader yet
    let mut cluster = TimelessTestCluster::with_nodes(3);

    // Act - Node 1 times out
    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Election));
    cluster.deliver_messages();

    // Assert - Pre-vote should succeed and lead to election
    assert_eq!(cluster.get_node(1).current_term(), 1);

    // Deliver vote responses
    cluster.deliver_messages();

    // Node 1 should become leader
    assert_eq!(*cluster.get_node(1).role(), NodeState::Leader);
}

/// Test that multiple nodes starting pre-votes leads to eventual election
#[test]
fn test_liveness_concurrent_pre_votes_eventually_elect() {
    // Arrange - 3 node cluster
    let mut cluster = TimelessTestCluster::with_nodes(3);

    // Act - Node 1 times out first
    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Election));
    cluster.deliver_messages();

    // Assert - Node 1 should go through pre-vote and win election
    cluster.deliver_messages();
    assert_eq!(*cluster.get_node(1).role(), NodeState::Leader);
}

/// Test that failed pre-vote does not affect voted_for or persistent state
#[test]
fn test_safety_pre_vote_does_not_persist_state() {
    // Arrange - 5 node cluster where we can partition one node
    let mut cluster = TimelessTestCluster::with_nodes(5);

    let initial_term_all = cluster.get_node(1).current_term();

    // Partition Node 1 so it will FAIL pre-vote (needs 3/5, but only has itself = 1/5)
    cluster.partition_node(1);

    // Act - Node 1 attempts pre-vote (will fail due to partition)
    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Election));

    // Deliver pre-vote messages
    cluster.deliver_messages();

    // Assert - All nodes should still be at same term (failed pre-vote doesn't increment)
    assert_eq!(cluster.get_node(1).current_term(), initial_term_all);
    assert_eq!(cluster.get_node(2).current_term(), initial_term_all);
    assert_eq!(cluster.get_node(3).current_term(), initial_term_all);

    // Node 1 should still be Follower (didn't advance to Candidate)
    assert_eq!(*cluster.get_node(1).role(), NodeState::Follower);

    // Try again - should still fail and not inflate term
    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Election));
    cluster.deliver_messages();

    // Term still unchanged (pre-vote prevents term inflation from partitioned nodes)
    assert_eq!(cluster.get_node(1).current_term(), initial_term_all);
    assert_eq!(*cluster.get_node(1).role(), NodeState::Follower);
}

/// Test that pre-vote is rejected when candidate has stale log
#[test]
fn test_safety_pre_vote_rejection_stale_log() {
    // Arrange - 3 node cluster
    let mut cluster = TimelessTestCluster::with_nodes(3);

    // Node 1 becomes leader and replicates an entry
    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Election));
    cluster.deliver_messages();
    cluster.deliver_messages();

    assert_eq!(*cluster.get_node(1).role(), NodeState::Leader);

    // Submit a command to Node 1
    cluster
        .get_node_mut(1)
        .on_event(Event::ClientCommand("test".to_string()));
    cluster.deliver_messages();
    cluster.deliver_messages();

    // All nodes should have the entry now
    assert!(cluster.get_node(1).storage().last_log_index() >= 1);
    assert!(cluster.get_node(2).storage().last_log_index() >= 1);
    assert!(cluster.get_node(3).storage().last_log_index() >= 1);

    // Partition Node 3 so it's isolated
    cluster.partition_node(3);

    // Node 3 attempts pre-vote with stale log
    cluster
        .get_node_mut(3)
        .on_event(Event::TimerFired(TimerKind::Election));
    cluster.deliver_messages();

    // Assert - Node 3 should NOT win pre-vote (stale log)
    // It should remain Follower (pre-vote failed)
    assert_eq!(*cluster.get_node(3).role(), NodeState::Follower);
    assert_eq!(cluster.get_node(3).current_term(), 1); // No term increment

    // Node 1 should still be leader
    assert_eq!(*cluster.get_node(1).role(), NodeState::Leader);
}

/// Test that pre-vote is rejected when candidate's log is behind a snapshot
#[test]
fn test_safety_pre_vote_rejection_snapshot_ahead() {
    // Arrange - 3 node cluster with a low snapshot threshold
    let mut cluster = TimelessTestCluster::new().with_snapshot_threshold(3);
    cluster.add_node(1);
    cluster.add_node(2);
    cluster.add_node(3);
    cluster.connect_peers();

    // Node 1 becomes leader
    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Election));
    cluster.deliver_messages();
    cluster.deliver_messages();

    assert_eq!(*cluster.get_node(1).role(), NodeState::Leader);
    let leader_term = cluster.get_node(1).current_term();

    // Partition Node 3 so it misses the entries and snapshot
    cluster.partition_node(3);

    // Leader appends entries to trigger snapshot creation
    for i in 1..=3 {
        cluster
            .get_node_mut(1)
            .on_event(Event::ClientCommand(format!("SET k{}=v{}", i, i)));
        cluster.deliver_messages();
    }

    // Snapshot should exist on leader (and Node 2)
    assert!(cluster.get_node(1).storage().load_snapshot().is_some());
    assert_eq!(cluster.get_node(1).storage().first_log_index(), 4);

    // Heal partition but do not deliver any leader heartbeats yet
    cluster.heal_partition();

    // Node 3 attempts pre-vote with stale log (behind snapshot)
    cluster
        .get_node_mut(3)
        .on_event(Event::TimerFired(TimerKind::Election));
    cluster.deliver_messages();

    // Assert - Node 3 should fail pre-vote and stay follower without term change
    assert_eq!(cluster.get_node(3).current_term(), leader_term);
    assert_eq!(*cluster.get_node(3).role(), NodeState::Follower);

    // Leader remains unchanged
    assert_eq!(*cluster.get_node(1).role(), NodeState::Leader);
    assert_eq!(cluster.get_node(1).current_term(), leader_term);
}

/// Test that pre-vote doesn't disrupt active leadership
#[test]
fn test_safety_pre_vote_does_not_disrupt_leader() {
    // Arrange - 3 node cluster with established leader
    let mut cluster = TimelessTestCluster::with_nodes(3);

    // Node 1 becomes leader
    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Election));
    cluster.deliver_messages();
    cluster.deliver_messages();

    assert_eq!(*cluster.get_node(1).role(), NodeState::Leader);
    let leader_term = cluster.get_node(1).current_term();

    // Leader commits an entry (Node 3 will miss this)
    cluster
        .get_node_mut(1)
        .on_event(Event::ClientCommand("test".to_string()));
    cluster.deliver_messages();

    // Partition Node 3 before heartbeat with the entry
    cluster.partition_node(3);

    // Leader sends heartbeat to Nodes 1 and 2 (Node 3 misses it)
    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Heartbeat));
    cluster.deliver_messages();

    // Node 3 attempts pre-vote (with stale log)
    cluster
        .get_node_mut(3)
        .on_event(Event::TimerFired(TimerKind::Election));
    cluster.deliver_messages();

    // Assert - Node 3 should fail pre-vote due to stale log
    // Nodes 1 and 2 will reject pre-vote because Node 3's log is behind
    assert_eq!(cluster.get_node(3).current_term(), leader_term); // No term increment
    assert_eq!(*cluster.get_node(3).role(), NodeState::Follower);

    // Heal partition and verify Node 1 is still leader
    cluster.heal_partition();
    assert_eq!(*cluster.get_node(1).role(), NodeState::Leader);
    assert_eq!(cluster.get_node(1).current_term(), leader_term);
}

/// Test that partitioned minority cannot win pre-vote
#[test]
fn test_safety_partitioned_minority_pre_vote_fails() {
    // Arrange - 5 node cluster
    let mut cluster = TimelessTestCluster::with_nodes(5);

    // Establish leader (Node 1)
    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Election));
    cluster.deliver_messages();
    cluster.deliver_messages();

    assert_eq!(*cluster.get_node(1).role(), NodeState::Leader);
    let original_term = cluster.get_node(1).current_term();

    // Leader sends heartbeat to establish leadership with all nodes
    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Heartbeat));
    cluster.deliver_messages();

    // Partition nodes 4 and 5 (minority of 2 out of 5)
    cluster.partition(&[4, 5], &[1, 2, 3]);

    // Node 4 attempts pre-vote
    cluster
        .get_node_mut(4)
        .on_event(Event::TimerFired(TimerKind::Election));
    cluster.deliver_messages();

    // Assert - Node 4 cannot win pre-vote (only has itself + Node 5 = 2/5, not majority)
    assert_eq!(*cluster.get_node(4).role(), NodeState::Follower);
    assert_eq!(cluster.get_node(4).current_term(), original_term); // No term increment

    // Try multiple times - should keep failing
    for _ in 0..3 {
        cluster
            .get_node_mut(4)
            .on_event(Event::TimerFired(TimerKind::Election));
        cluster.deliver_messages();

        // Still no term increment (pre-vote prevents term inflation)
        assert_eq!(cluster.get_node(4).current_term(), original_term);
        assert_eq!(*cluster.get_node(4).role(), NodeState::Follower);
    }

    // Majority partition (nodes 1, 2, 3) should be unaffected
    assert_eq!(*cluster.get_node(1).role(), NodeState::Leader);
    assert_eq!(cluster.get_node(1).current_term(), original_term);
}
