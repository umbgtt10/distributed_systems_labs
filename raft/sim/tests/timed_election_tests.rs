// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use raft_core::node_state::NodeState;
use raft_sim::timefull_test_cluster::TimefullTestCluster;

#[test]
fn test_liveness_real_timer_triggers_election() {
    let mut cluster = TimefullTestCluster::new();

    // Node 1: Quick timeout (will trigger election first)
    cluster.add_node_with_timeout(1, 100, 150);

    // Nodes 2 & 3: Very long timeout (won't trigger during test)
    cluster.add_node_with_timeout(2, 10000, 10000);
    cluster.add_node_with_timeout(3, 10000, 10000);

    cluster.connect_peers();

    // Advance time past election timeout (max 300ms)
    cluster.advance_time(350);
    cluster.deliver_messages();

    // Should have elected a leader
    let leaders: Vec<_> = (1..=3)
        .filter(|&id| *cluster.get_node(id).role() == NodeState::Leader)
        .collect();

    assert_eq!(leaders.len(), 1, "Exactly one leader should be elected");
}

#[test]
fn test_liveness_leader_sends_heartbeats_automatically() {
    let mut cluster = TimefullTestCluster::new();

    // Node 1: Quick timeout
    cluster.add_node_with_timeout(1, 100, 150);
    cluster.add_node_with_timeout(2, 10000, 10000);
    cluster.add_node_with_timeout(3, 10000, 10000);

    cluster.connect_peers();

    // Elect leader
    cluster.advance_time(350);
    cluster.deliver_messages();

    let leader_id = (1..=3)
        .find(|&id| *cluster.get_node(id).role() == NodeState::Leader)
        .expect("Should have a leader");

    // Advance to heartbeat time (50ms)
    cluster.advance_time(60);
    cluster.deliver_messages();

    // Advance past election timeout again - should NOT re-elect
    // because followers received heartbeats
    cluster.advance_time(300);
    cluster.deliver_messages();

    // Leader should still be leader
    assert_eq!(*cluster.get_node(leader_id).role(), NodeState::Leader);

    // No new leaders
    let leader_count = (1..=3)
        .filter(|&id| *cluster.get_node(id).role() == NodeState::Leader)
        .count();
    assert_eq!(leader_count, 1, "Should still have exactly one leader");
}

#[test]
fn test_liveness_split_vote_then_recovery() {
    let mut cluster = TimefullTestCluster::new();

    // Give nodes DIFFERENT timeouts so randomization spreads them out
    cluster.add_node_with_timeout(1, 100, 200); // 100-200ms
    cluster.add_node_with_timeout(2, 150, 250); // 150-250ms
    cluster.add_node_with_timeout(3, 200, 300); // 200-300ms

    cluster.connect_peers();

    // Try multiple rounds if needed
    let mut elected = false;

    for _ in 1..=5 {
        // Advance time in SMALLER steps so we catch the first timeout
        for _ in 0..15 {
            // 15 steps of 30ms = 450ms total
            cluster.advance_time(30);
            cluster.deliver_messages();

            let leaders: Vec<_> = (1..=3)
                .filter(|&id| *cluster.get_node(id).role() == NodeState::Leader)
                .collect();

            if leaders.len() == 1 {
                elected = true;
                break;
            }
        }

        if elected {
            break;
        }

        let leaders: Vec<_> = (1..=3)
            .filter(|&id| *cluster.get_node(id).role() == NodeState::Leader)
            .collect();

        if leaders.is_empty() {
            println!("⚠️  Split vote, retrying...");
        } else if leaders.len() > 1 {
            panic!("Multiple leaders: {:?}", leaders);
        }
    }

    assert!(elected, "Failed to elect leader after 5 rounds");
}

#[test]
fn test_safety_election_timer_starts_without_messages() {
    let mut cluster = TimefullTestCluster::new();

    // Single isolated node - no peers, no messages
    cluster.add_node_with_timeout(1, 150, 200);
    // Don't connect peers - node is isolated

    // Verify starts as Follower
    assert_eq!(*cluster.get_node(1).role(), NodeState::Follower);

    // Advance time past election timeout
    cluster.advance_time(250);

    // With pre-vote: isolated node sends pre-vote to itself (no peers)
    // It will grant itself a pre-vote and start real election, becoming Candidate
    // Since it has no peers, it immediately wins (majority of 1 = 1)
    assert_eq!(
        *cluster.get_node(1).role(),
        NodeState::Leader,
        "Isolated node should become leader after timeout (no peers to outvote it)"
    );
}
