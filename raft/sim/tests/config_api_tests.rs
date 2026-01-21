// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use raft_core::{
    config_change_manager::ConfigError, event::Event, storage::Storage, timer_service::TimerKind,
};
use raft_sim::timeless_test_cluster::TimelessTestCluster;

#[test]
fn test_add_server_as_follower_fails() {
    let mut cluster = TimelessTestCluster::with_nodes(3);

    // Elect node 1 as leader
    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Election));
    cluster.deliver_messages();

    // Try to add server as follower (node 2)
    let result = cluster.get_node_mut(2).add_server(4);
    assert_eq!(result, Err(ConfigError::NotLeader));
}

#[test]
fn test_add_server_already_exists() {
    let mut cluster = TimelessTestCluster::with_nodes(3);

    // Elect node 1 as leader
    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Election));
    cluster.deliver_messages();

    // Try to add server that's already in cluster
    let result = cluster.get_node_mut(1).add_server(2);
    assert_eq!(result, Err(ConfigError::NodeAlreadyExists));
}

#[test]
fn test_add_server_self_fails() {
    let mut cluster = TimelessTestCluster::with_nodes(3);

    // Elect node 1 as leader
    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Election));
    cluster.deliver_messages();

    // Try to add self
    let result = cluster.get_node_mut(1).add_server(1);
    assert_eq!(result, Err(ConfigError::NodeAlreadyExists));
}

#[test]
fn test_add_server_success() {
    let mut cluster = TimelessTestCluster::with_nodes(3);

    // Elect node 1 as leader
    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Election));
    cluster.deliver_messages();

    // Add new server
    let result = cluster.get_node_mut(1).add_server(4);
    assert!(result.is_ok(), "Should successfully submit config change");

    let index = result.unwrap();
    assert!(index > 0, "Should return valid log index");
}

#[test]
fn test_add_server_blocks_concurrent_changes() {
    let mut cluster = TimelessTestCluster::with_nodes(3);

    // Elect node 1 as leader
    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Election));
    cluster.deliver_messages();

    // Add first server
    let result1 = cluster.get_node_mut(1).add_server(4);
    assert!(result1.is_ok(), "First add should succeed");

    // Try to add another server before first commits
    let result2 = cluster.get_node_mut(1).add_server(5);
    assert_eq!(result2, Err(ConfigError::ConfigChangeInProgress));
}

#[test]
fn test_remove_server_as_follower_fails() {
    let mut cluster = TimelessTestCluster::with_nodes(3);

    // Elect node 1 as leader
    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Election));
    cluster.deliver_messages();

    // Try to remove server as follower
    let result = cluster.get_node_mut(2).remove_server(3);
    assert_eq!(result, Err(ConfigError::NotLeader));
}

#[test]
fn test_remove_server_not_found() {
    let mut cluster = TimelessTestCluster::with_nodes(3);

    // Elect node 1 as leader
    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Election));
    cluster.deliver_messages();

    // Try to remove server not in cluster
    let result = cluster.get_node_mut(1).remove_server(99);
    assert_eq!(result, Err(ConfigError::NodeNotFound));
}

#[test]
fn test_remove_server_last_node() {
    let mut cluster = TimelessTestCluster::with_nodes(1);

    // Elect node 1 as leader
    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Election));
    cluster.deliver_messages();

    // Try to remove last node
    let result = cluster.get_node_mut(1).remove_server(1);
    assert_eq!(result, Err(ConfigError::CannotRemoveLastNode));
}

#[test]
fn test_remove_server_success() {
    let mut cluster = TimelessTestCluster::with_nodes(3);

    // Elect node 1 as leader
    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Election));
    cluster.deliver_messages();

    // Remove a server
    let result = cluster.get_node_mut(1).remove_server(3);
    assert!(result.is_ok(), "Should successfully submit config change");

    let index = result.unwrap();
    assert!(index > 0, "Should return valid log index");
}

#[test]
fn test_remove_self_success() {
    let mut cluster = TimelessTestCluster::with_nodes(3);

    // Elect node 1 as leader
    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Election));
    cluster.deliver_messages();

    // Leader removes self (valid operation)
    let result = cluster.get_node_mut(1).remove_server(1);
    assert!(result.is_ok(), "Leader can remove itself");
}

// ============================================================================
// Integration Tests - Actual Configuration Changes
// ============================================================================

#[test]
fn test_add_server_replicates_and_commits() {
    let mut cluster = TimelessTestCluster::with_nodes(3);

    // Elect node 1 as leader
    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Election));
    cluster.deliver_messages();

    // Leader submits add_server
    let result = cluster.get_node_mut(1).add_server(4);
    assert!(result.is_ok(), "Should successfully submit config change");
    let config_index = result.unwrap();

    // Replicate to followers
    cluster.deliver_messages();

    // Verify the entry is in all nodes' logs
    for node_id in [1, 2, 3] {
        let node = cluster.get_node(node_id);
        assert!(
            node.storage().last_log_index() >= config_index,
            "Node {} should have config entry",
            node_id
        );
    }

    // Leader should commit once majority has replicated
    assert!(
        cluster.get_node(1).is_committed(config_index),
        "Config change should be committed"
    );
}

#[test]
fn test_add_server_updates_configuration() {
    let mut cluster = TimelessTestCluster::with_nodes(3);

    // Elect node 1 as leader
    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Election));
    cluster.deliver_messages();

    // Verify initial config has 3 nodes (1 self + 2 peers)
    assert_eq!(cluster.get_node(1).config().size(), 3);

    // Add node 4
    let result = cluster.get_node_mut(1).add_server(4);
    assert!(result.is_ok());

    // Replicate and commit
    cluster.deliver_messages();

    // Verify leader has new config (1 self + 3 peers)
    assert_eq!(
        cluster.get_node(1).config().size(),
        4,
        "Leader should have 4 members after commit"
    );
    assert!(
        cluster.get_node(1).config().contains(4),
        "Config should contain node 4"
    );

    // Verify pending flag is cleared after commit
    let result2 = cluster.get_node_mut(1).add_server(5);
    assert!(
        result2.is_ok(),
        "Should allow another config change after first commits"
    );
}

#[test]
fn test_remove_server_replicates_and_commits() {
    let mut cluster = TimelessTestCluster::with_nodes(3);

    // Elect node 1 as leader
    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Election));
    cluster.deliver_messages();

    // Remove node 3
    let result = cluster.get_node_mut(1).remove_server(3);
    assert!(result.is_ok(), "Should successfully submit config change");
    let config_index = result.unwrap();

    // Replicate to followers
    cluster.deliver_messages();

    // Verify committed on leader
    assert!(
        cluster.get_node(1).is_committed(config_index),
        "Config change should be committed"
    );
}

#[test]
fn test_remove_server_updates_configuration() {
    let mut cluster = TimelessTestCluster::with_nodes(3);

    // Elect node 1 as leader
    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Election));
    cluster.deliver_messages();

    // Verify initial config has 3 nodes (1 self + 2 peers)
    assert_eq!(cluster.get_node(1).config().size(), 3);

    // Remove node 3
    let result = cluster.get_node_mut(1).remove_server(3);
    assert!(result.is_ok());

    // Replicate and commit
    cluster.deliver_messages();
    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Heartbeat));
    cluster.deliver_messages();

    // Verify leader has new config (1 self + 1 peer)
    assert_eq!(
        cluster.get_node(1).config().size(),
        2,
        "Leader should have 2 members after commit"
    );
    assert!(
        !cluster.get_node(1).config().contains(3),
        "Config should not contain node 3"
    );

    // Verify quorum calculation updated (2 nodes, quorum = 2)
    assert_eq!(cluster.get_node(1).config().quorum_size(), 2);
}

#[test]
fn test_follower_applies_committed_config_change() {
    let mut cluster = TimelessTestCluster::with_nodes(3);

    // Elect node 1 as leader
    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Election));
    cluster.deliver_messages();

    // Add node 4 on leader
    let result = cluster.get_node_mut(1).add_server(4);
    assert!(result.is_ok());

    // Replicate to followers
    cluster.deliver_messages();
    // Send heartbeat to propagate commit index
    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Heartbeat));
    cluster.deliver_messages();
    // Verify follower node 2 also has new config
    assert_eq!(
        cluster.get_node(2).config().size(),
        4,
        "Follower should have 4 members"
    );
    assert!(
        cluster.get_node(2).config().contains(4),
        "Follower should have node 4 in config"
    );
}

#[test]
fn test_config_change_survives_leadership_change() {
    let mut cluster = TimelessTestCluster::with_nodes(3);

    // Elect node 1 as leader
    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Election));
    cluster.deliver_messages();

    // Add node 4
    let result = cluster.get_node_mut(1).add_server(4);
    assert!(result.is_ok());

    // Replicate and commit
    cluster.deliver_messages();
    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Heartbeat));
    cluster.deliver_messages();

    // Node 2 becomes new leader
    cluster
        .get_node_mut(2)
        .on_event(Event::TimerFired(TimerKind::Election));
    cluster.deliver_messages();

    // New leader should still have the config with 4 nodes
    assert_eq!(
        cluster.get_node(2).config().size(),
        4,
        "New leader should have committed config"
    );
    assert!(cluster.get_node(2).config().contains(4));
}
