// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use raft_core::{
    config_change_manager::{ConfigChangeManager, ConfigError},
    configuration::Configuration,
    log_entry::ConfigurationChange,
    node_collection::NodeCollection,
};
use raft_sim::{
    in_memory_map_collection::InMemoryMapCollection,
    in_memory_node_collection::InMemoryNodeCollection,
};

fn make_test_config(nodes: &[u64]) -> Configuration<InMemoryNodeCollection> {
    let mut members = InMemoryNodeCollection::new();
    for &node_id in nodes {
        members.push(node_id).unwrap();
    }
    Configuration::new(members)
}

// ============================================================================
// Construction and Basic Getters
// ============================================================================

#[test]
fn test_new_manager() {
    let config = make_test_config(&[2, 3]);
    let manager: ConfigChangeManager<InMemoryNodeCollection, InMemoryMapCollection> =
        ConfigChangeManager::new(config);

    assert_eq!(manager.config().size(), 3); // 2 peers + 1 self
    assert!(manager.config().contains(2));
    assert!(manager.config().contains(3));
}

#[test]
fn test_config_getter() {
    let config = make_test_config(&[2, 3, 4]);
    let manager: ConfigChangeManager<InMemoryNodeCollection, InMemoryMapCollection> =
        ConfigChangeManager::new(config);

    let retrieved_config = manager.config();
    assert_eq!(retrieved_config.size(), 4); // 3 peers + 1 self
    assert_eq!(retrieved_config.quorum_size(), 3); // (4/2)+1 = 3
}

// ============================================================================
// add_server Validation Tests
// ============================================================================

#[test]
fn test_add_server_not_leader() {
    let config = make_test_config(&[2, 3]);
    let mut manager: ConfigChangeManager<InMemoryNodeCollection, InMemoryMapCollection> =
        ConfigChangeManager::new(config);

    let result = manager.add_server(
        4,     // node_id to add
        1,     // self_id
        false, // is_leader = false
        0,     // commit_index
    );

    assert_eq!(result, Err(ConfigError::NotLeader));
}

#[test]
fn test_add_server_already_exists_in_peers() {
    let config = make_test_config(&[2, 3]);
    let mut manager: ConfigChangeManager<InMemoryNodeCollection, InMemoryMapCollection> =
        ConfigChangeManager::new(config);

    let result = manager.add_server(
        2,    // node already in config
        1,    // self_id
        true, // is_leader
        0,    // commit_index
    );

    assert_eq!(result, Err(ConfigError::NodeAlreadyExists));
}

#[test]
fn test_add_server_self_already_exists() {
    let config = make_test_config(&[2, 3]);
    let mut manager: ConfigChangeManager<InMemoryNodeCollection, InMemoryMapCollection> =
        ConfigChangeManager::new(config);

    let result = manager.add_server(
        1,    // self_id
        1,    // self_id
        true, // is_leader
        0,    // commit_index
    );

    assert_eq!(result, Err(ConfigError::NodeAlreadyExists));
}

#[test]
fn test_add_server_config_change_in_progress() {
    let config = make_test_config(&[2, 3]);
    let mut manager: ConfigChangeManager<InMemoryNodeCollection, InMemoryMapCollection> =
        ConfigChangeManager::new(config);

    // First add succeeds
    let result1 = manager.add_server(4, 1, true, 0);
    assert!(result1.is_ok());
    manager.track_pending_change(10); // Simulate tracking at index 10

    // Second add should fail because first is not committed
    let result2 = manager.add_server(
        5,    // different node
        1,    // self_id
        true, // is_leader
        5,    // commit_index = 5 (< 10, so not committed)
    );

    assert_eq!(result2, Err(ConfigError::ConfigChangeInProgress));
}

#[test]
fn test_add_server_allows_new_change_after_commit() {
    let config = make_test_config(&[2, 3]);
    let mut manager: ConfigChangeManager<InMemoryNodeCollection, InMemoryMapCollection> =
        ConfigChangeManager::new(config);

    // First add succeeds
    let result1 = manager.add_server(4, 1, true, 0);
    assert!(result1.is_ok());
    manager.track_pending_change(10);

    // Second add succeeds because first is committed (commit_index >= 10)
    let result2 = manager.add_server(5, 1, true, 10);
    assert!(result2.is_ok());
    assert_eq!(result2.unwrap(), ConfigurationChange::AddServer(5));
}

#[test]
fn test_add_server_returns_correct_change() {
    let config = make_test_config(&[2, 3]);
    let mut manager: ConfigChangeManager<InMemoryNodeCollection, InMemoryMapCollection> =
        ConfigChangeManager::new(config);

    let result = manager.add_server(4, 1, true, 0);

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), ConfigurationChange::AddServer(4));
}

// ============================================================================
// remove_server Validation Tests
// ============================================================================

#[test]
fn test_remove_server_not_leader() {
    let config = make_test_config(&[2, 3]);
    let mut manager: ConfigChangeManager<InMemoryNodeCollection, InMemoryMapCollection> =
        ConfigChangeManager::new(config);

    let result = manager.remove_server(
        3,     // node_id to remove
        1,     // self_id
        false, // is_leader = false
        0,     // commit_index
    );

    assert_eq!(result, Err(ConfigError::NotLeader));
}

#[test]
fn test_remove_server_not_found() {
    let config = make_test_config(&[2, 3]);
    let mut manager: ConfigChangeManager<InMemoryNodeCollection, InMemoryMapCollection> =
        ConfigChangeManager::new(config);

    let result = manager.remove_server(
        99,   // node not in config (and not self)
        1,    // self_id
        true, // is_leader
        0,    // commit_index
    );

    assert_eq!(result, Err(ConfigError::NodeNotFound));
}

#[test]
fn test_remove_server_last_node() {
    let config = make_test_config(&[]);
    let mut manager: ConfigChangeManager<InMemoryNodeCollection, InMemoryMapCollection> =
        ConfigChangeManager::new(config);

    // Trying to remove self when it's the only node
    let result = manager.remove_server(
        1,    // self
        1,    // self_id
        true, // is_leader
        0,    // commit_index
    );

    assert_eq!(result, Err(ConfigError::CannotRemoveLastNode));
}

#[test]
fn test_remove_server_self_allowed() {
    let config = make_test_config(&[2, 3]);
    let mut manager: ConfigChangeManager<InMemoryNodeCollection, InMemoryMapCollection> =
        ConfigChangeManager::new(config);

    // Leader can remove itself as long as it's not the last node
    let result = manager.remove_server(
        1,    // self
        1,    // self_id
        true, // is_leader
        0,    // commit_index
    );

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), ConfigurationChange::RemoveServer(1));
}

#[test]
fn test_remove_server_config_change_in_progress() {
    let config = make_test_config(&[2, 3]);
    let mut manager: ConfigChangeManager<InMemoryNodeCollection, InMemoryMapCollection> =
        ConfigChangeManager::new(config);

    // First remove succeeds
    let result1 = manager.remove_server(3, 1, true, 0);
    assert!(result1.is_ok());
    manager.track_pending_change(10);

    // Second remove should fail
    let result2 = manager.remove_server(2, 1, true, 5);

    assert_eq!(result2, Err(ConfigError::ConfigChangeInProgress));
}

#[test]
fn test_remove_server_returns_correct_change() {
    let config = make_test_config(&[2, 3, 4]);
    let mut manager: ConfigChangeManager<InMemoryNodeCollection, InMemoryMapCollection> =
        ConfigChangeManager::new(config);

    let result = manager.remove_server(3, 1, true, 0);

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), ConfigurationChange::RemoveServer(3));
}

// ============================================================================
// Pending Change Tracking Tests
// ============================================================================

#[test]
fn test_track_pending_change() {
    let config = make_test_config(&[2, 3]);
    let mut manager: ConfigChangeManager<InMemoryNodeCollection, InMemoryMapCollection> =
        ConfigChangeManager::new(config);

    manager.track_pending_change(42);

    // Verify it blocks new changes
    let result = manager.add_server(4, 1, true, 10);
    assert_eq!(result, Err(ConfigError::ConfigChangeInProgress));
}

#[test]
fn test_is_change_committed() {
    let config = make_test_config(&[2, 3]);
    let manager: ConfigChangeManager<InMemoryNodeCollection, InMemoryMapCollection> =
        ConfigChangeManager::new(config);

    assert!(manager.is_change_committed(5, 10)); // index <= commit_index
    assert!(manager.is_change_committed(10, 10)); // index == commit_index
    assert!(!manager.is_change_committed(15, 10)); // index > commit_index
}

#[test]
fn test_pending_change_cleared_after_commit() {
    let config = make_test_config(&[2, 3]);
    let mut manager: ConfigChangeManager<InMemoryNodeCollection, InMemoryMapCollection> =
        ConfigChangeManager::new(config);

    // Add server and track it
    let result1 = manager.add_server(4, 1, true, 0);
    assert!(result1.is_ok());
    manager.track_pending_change(10);

    // With commit_index < 10, new change blocked
    let result2 = manager.add_server(5, 1, true, 9);
    assert_eq!(result2, Err(ConfigError::ConfigChangeInProgress));

    // With commit_index >= 10, new change allowed
    let result3 = manager.add_server(5, 1, true, 10);
    assert!(result3.is_ok());
}

// ============================================================================
// Edge Cases
// ============================================================================

#[test]
fn test_add_to_empty_cluster() {
    let config = make_test_config(&[]);
    let mut manager: ConfigChangeManager<InMemoryNodeCollection, InMemoryMapCollection> =
        ConfigChangeManager::new(config);

    // Single node cluster (self_id=1, no peers)
    let result = manager.add_server(2, 1, true, 0);

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), ConfigurationChange::AddServer(2));
}

#[test]
fn test_remove_from_two_node_cluster() {
    let config = make_test_config(&[2]);
    let mut manager: ConfigChangeManager<InMemoryNodeCollection, InMemoryMapCollection> =
        ConfigChangeManager::new(config);

    // Two node cluster (self_id=1, peer=2)
    let result = manager.remove_server(2, 1, true, 0);

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), ConfigurationChange::RemoveServer(2));
}

#[test]
fn test_cannot_remove_only_member() {
    let config = make_test_config(&[]);
    let mut manager: ConfigChangeManager<InMemoryNodeCollection, InMemoryMapCollection> =
        ConfigChangeManager::new(config);

    // Single node, trying to remove self
    let result = manager.remove_server(1, 1, true, 0);

    assert_eq!(result, Err(ConfigError::CannotRemoveLastNode));
}

#[test]
fn test_multiple_changes_in_sequence() {
    let config = make_test_config(&[2, 3]);
    let mut manager: ConfigChangeManager<InMemoryNodeCollection, InMemoryMapCollection> =
        ConfigChangeManager::new(config);

    // Change 1
    let result1 = manager.add_server(4, 1, true, 0);
    assert!(result1.is_ok());
    manager.track_pending_change(10);

    // Change 2 (after first commits)
    let result2 = manager.remove_server(3, 1, true, 10);
    assert!(result2.is_ok());
    manager.track_pending_change(15);

    // Change 3 (blocked)
    let result3 = manager.add_server(5, 1, true, 14);
    assert_eq!(result3, Err(ConfigError::ConfigChangeInProgress));

    // Change 3 (allowed after commit)
    let result4 = manager.add_server(5, 1, true, 15);
    assert!(result4.is_ok());
}

// ============================================================================
// Integration-like Tests (without full RaftNode)
// ============================================================================

#[test]
fn test_config_manager_workflow() {
    let config = make_test_config(&[2, 3]);
    let mut manager: ConfigChangeManager<InMemoryNodeCollection, InMemoryMapCollection> =
        ConfigChangeManager::new(config);

    // 1. Validate we can add a server
    let change = manager.add_server(4, 1, true, 0).unwrap();
    assert_eq!(change, ConfigurationChange::AddServer(4));

    // 2. Track it at index 10
    manager.track_pending_change(10);

    // 3. Try another change before commit - fails
    assert_eq!(
        manager.add_server(5, 1, true, 9),
        Err(ConfigError::ConfigChangeInProgress)
    );

    // 4. After commit, another change succeeds
    let change2 = manager.remove_server(2, 1, true, 10).unwrap();
    assert_eq!(change2, ConfigurationChange::RemoveServer(2));
}
