// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! Tests for MessageHandler in isolation
//!
//! These tests demonstrate that MessageHandler can be instantiated and used
//! independently of RaftNode, testing the message handling logic in isolation.

use raft_core::{
    config_change_manager::ConfigChangeManager, configuration::Configuration,
    election_manager::ElectionManager, log_replication_manager::LogReplicationManager,
    message_handler::MessageHandler, node_state::NodeState, snapshot_manager::SnapshotManager,
    storage::Storage,
};
use raft_sim::{
    in_memory_config_change_collection::InMemoryConfigChangeCollection,
    in_memory_map_collection::InMemoryMapCollection,
    in_memory_node_collection::InMemoryNodeCollection,
    in_memory_state_machine::InMemoryStateMachine, in_memory_storage::InMemoryStorage,
    in_memory_transport::InMemoryTransport, message_broker::MessageBroker,
    no_action_timer::DummyTimer, null_observer::NullObserver,
};
use std::sync::{Arc, Mutex};

// ============================================================
// Helper functions
// ============================================================

fn make_empty_config() -> Configuration<InMemoryNodeCollection> {
    Configuration::new(InMemoryNodeCollection::new())
}

// ============================================================
// Basic instantiation tests
// ============================================================

#[test]
fn test_message_handler_can_be_created() {
    // This test demonstrates that MessageHandler can be instantiated
    // with mutable references to all dependencies

    let node_id = 1;
    let mut role = NodeState::Follower;
    let mut current_term = 5;
    let broker = Arc::new(Mutex::new(MessageBroker::new()));
    let mut transport = InMemoryTransport::new(node_id, broker);
    let mut storage = InMemoryStorage::new();
    let mut state_machine = InMemoryStateMachine::new();
    let mut observer = NullObserver::new();
    let mut election = ElectionManager::new(DummyTimer);
    let mut replication = LogReplicationManager::<InMemoryMapCollection>::new();
    let mut config_manager = ConfigChangeManager::new(make_empty_config());
    let mut snapshot_manager = SnapshotManager::new(1000);

    // Create the MessageHandler
    let _handler = MessageHandler {
        id: &node_id,
        role: &mut role,
        current_term: &mut current_term,
        transport: &mut transport,
        storage: &mut storage,
        state_machine: &mut state_machine,
        observer: &mut observer,
        election: &mut election,
        replication: &mut replication,
        config_manager: &mut config_manager,
        snapshot_manager: &mut snapshot_manager,
        _phantom: core::marker::PhantomData::<InMemoryConfigChangeCollection>,
    };

    // If we get here, MessageHandler was successfully created
    assert_eq!(*_handler.id, 1);
}

#[test]
fn test_message_handler_start_election_increments_term() {
    // This test shows that MessageHandler's start_election method works

    let node_id = 1;
    let mut role = NodeState::Follower;
    let mut current_term = 5;
    let broker = Arc::new(Mutex::new(MessageBroker::new()));
    let mut transport = InMemoryTransport::new(node_id, broker);
    let mut storage = InMemoryStorage::new();
    let mut state_machine = InMemoryStateMachine::new();
    let mut observer = NullObserver::new();
    let mut election = ElectionManager::new(DummyTimer);
    let mut replication = LogReplicationManager::<InMemoryMapCollection>::new();
    let mut config_manager = ConfigChangeManager::new(make_empty_config());
    let mut snapshot_manager = SnapshotManager::new(1000);

    storage.set_current_term(5);

    let mut handler = MessageHandler {
        id: &node_id,
        role: &mut role,
        current_term: &mut current_term,
        transport: &mut transport,
        storage: &mut storage,
        state_machine: &mut state_machine,
        observer: &mut observer,
        election: &mut election,
        replication: &mut replication,
        config_manager: &mut config_manager,
        snapshot_manager: &mut snapshot_manager,
        _phantom: core::marker::PhantomData::<InMemoryConfigChangeCollection>,
    };

    handler.start_election();

    // Should have incremented term and become candidate or leader
    // (becomes leader if it's the only node in the cluster)
    assert_eq!(current_term, 6);
    assert!(role == NodeState::Candidate || role == NodeState::Leader);
    assert_eq!(storage.current_term(), 6);
}

#[test]
fn test_message_handler_start_pre_vote_does_not_increment_term() {
    // This test shows that pre-vote doesn't increment the term

    let node_id = 1;
    let mut role = NodeState::Follower;
    let mut current_term = 5;
    let broker = Arc::new(Mutex::new(MessageBroker::new()));
    let mut transport = InMemoryTransport::new(node_id, broker);
    let mut storage = InMemoryStorage::new();
    let mut state_machine = InMemoryStateMachine::new();
    let mut observer = NullObserver::new();
    let mut election = ElectionManager::new(DummyTimer);
    let mut replication = LogReplicationManager::<InMemoryMapCollection>::new();
    let mut config_manager = ConfigChangeManager::new(make_empty_config());
    let mut snapshot_manager = SnapshotManager::new(1000);

    storage.set_current_term(5);

    let mut handler = MessageHandler {
        id: &node_id,
        role: &mut role,
        current_term: &mut current_term,
        transport: &mut transport,
        storage: &mut storage,
        state_machine: &mut state_machine,
        observer: &mut observer,
        election: &mut election,
        replication: &mut replication,
        config_manager: &mut config_manager,
        snapshot_manager: &mut snapshot_manager,
        _phantom: core::marker::PhantomData::<InMemoryConfigChangeCollection>,
    };

    handler.start_pre_vote();

    // Pre-vote should NOT increment term or change role
    assert_eq!(current_term, 5);
    assert_eq!(role, NodeState::Follower);
    assert_eq!(storage.current_term(), 5);
}

#[test]
fn test_message_handler_demonstrates_isolation() {
    // This test shows that MessageHandler can be tested without RaftNode

    let node_id = 1;
    let mut role = NodeState::Follower;
    let mut current_term = 5;
    let broker = Arc::new(Mutex::new(MessageBroker::new()));
    let mut transport = InMemoryTransport::new(node_id, broker);
    let mut storage = InMemoryStorage::new();
    let mut state_machine = InMemoryStateMachine::new();
    let mut observer = NullObserver::new();
    let mut election = ElectionManager::new(DummyTimer);
    let mut replication = LogReplicationManager::<InMemoryMapCollection>::new();
    let mut config_manager = ConfigChangeManager::new(make_empty_config());
    let mut snapshot_manager = SnapshotManager::new(1000);

    // Multiple operations can be tested in sequence
    let mut handler = MessageHandler {
        id: &node_id,
        role: &mut role,
        current_term: &mut current_term,
        transport: &mut transport,
        storage: &mut storage,
        state_machine: &mut state_machine,
        observer: &mut observer,
        election: &mut election,
        replication: &mut replication,
        config_manager: &mut config_manager,
        snapshot_manager: &mut snapshot_manager,
        _phantom: core::marker::PhantomData::<InMemoryConfigChangeCollection>,
    };

    // Do pre-vote - still follower
    handler.start_pre_vote();

    assert_eq!(role, NodeState::Follower);
    assert_eq!(current_term, 5);

    // Do real election - becomes candidate, term increments
    let mut handler = MessageHandler {
        id: &node_id,
        role: &mut role,
        current_term: &mut current_term,
        transport: &mut transport,
        storage: &mut storage,
        state_machine: &mut state_machine,
        observer: &mut observer,
        election: &mut election,
        replication: &mut replication,
        config_manager: &mut config_manager,
        snapshot_manager: &mut snapshot_manager,
        _phantom: core::marker::PhantomData::<InMemoryConfigChangeCollection>,
    };
    handler.start_election();

    // Becomes candidate or leader (leader if only node in cluster)
    assert!(role == NodeState::Candidate || role == NodeState::Leader);
    assert_eq!(current_term, 6);
}
