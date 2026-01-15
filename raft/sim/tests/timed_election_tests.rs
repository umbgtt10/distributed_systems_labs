// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use raft_core::node_state::NodeState;
use raft_sim::timefull_test_cluster::TimefullTestCluster;

#[test]
fn test_real_timer_triggers_election() {
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
fn test_leader_sends_heartbeats_automatically() {
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
fn test_split_vote_then_recovery() {
    let mut cluster = TimefullTestCluster::new();

    // Give nodes DIFFERENT timeouts so randomization spreads them out
    cluster.add_node_with_timeout(1, 100, 200); // 100-200ms
    cluster.add_node_with_timeout(2, 150, 250); // 150-250ms
    cluster.add_node_with_timeout(3, 200, 300); // 200-300ms

    cluster.connect_peers();

    // Try multiple rounds if needed
    let mut elected = false;

    for round in 1..=5 {
        println!("\n=== Round {} ===", round);

        // Advance time in SMALLER steps so we catch the first timeout
        for step in 0..15 {
            // 15 steps of 30ms = 450ms total
            cluster.advance_time(30);
            cluster.deliver_messages();

            let leaders: Vec<_> = (1..=3)
                .filter(|&id| *cluster.get_node(id).role() == NodeState::Leader)
                .collect();

            if leaders.len() == 1 {
                println!(
                    "✅ Leader elected at {}ms in round {}: Node {}",
                    (step + 1) * 30,
                    round,
                    leaders[0]
                );
                elected = true;
                break;
            }
        }

        if elected {
            break;
        }

        println!("After round {}:", round);
        for id in 1..=3 {
            println!(
                "  Node {}: role={:?}, term={}",
                id,
                cluster.get_node(id).role(),
                cluster.get_node(id).current_term()
            );
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
