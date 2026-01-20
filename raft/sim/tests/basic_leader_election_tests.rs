// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use raft_core::{
    event::Event, log_entry_collection::LogEntryCollection, node_collection::NodeCollection,
    node_state::NodeState, raft_messages::RaftMsg, timer_service::TimerKind,
};
use raft_sim::{
    in_memory_log_entry_collection::InMemoryLogEntryCollection,
    in_memory_node_collection::InMemoryNodeCollection, timeless_test_cluster::TimelessTestCluster,
};

#[test]
fn test_liveness_empty_cluster() {
    // Arrange
    let cluster = TimelessTestCluster::new();

    // Assert
    assert_eq!(cluster.get_node_ids().len(), 0);
}

#[test]
fn test_liveness_add_node() {
    // Arrange
    let mut cluster = TimelessTestCluster::new();

    // Act
    cluster.add_node(1);
    cluster.add_node(2);

    // Assert
    assert!(cluster.get_node(1).id() == 1);
    assert!(cluster.get_node(2).id() == 2);
}

#[test]
fn test_liveness_connection() {
    // Arrange
    let mut cluster = TimelessTestCluster::new();
    cluster.add_node(1);
    cluster.add_node(2);
    cluster.add_node(3);
    cluster.connect_peers();

    let expected_peers_1 = {
        let mut peers = InMemoryNodeCollection::new();
        peers.push(2).unwrap();
        peers.push(3).unwrap();
        peers
    };

    let expected_peers_2 = {
        let mut peers = InMemoryNodeCollection::new();
        peers.push(1).unwrap();
        peers.push(3).unwrap();
        peers
    };

    let expected_peers_3 = {
        let mut peers = InMemoryNodeCollection::new();
        peers.push(1).unwrap();
        peers.push(2).unwrap();
        peers
    };

    // Assert
    assert_eq!(*cluster.get_node(1).peers().unwrap(), expected_peers_1);
    assert_eq!(*cluster.get_node(2).peers().unwrap(), expected_peers_2);
    assert_eq!(*cluster.get_node(3).peers().unwrap(), expected_peers_3);
}

#[test]
fn test_liveness_heartbeat_followers_nothing_delivered() {
    // Arrange
    let mut cluster = TimelessTestCluster::new();
    cluster.add_node(1);
    cluster.add_node(2);
    cluster.add_node(3);
    cluster.connect_peers();

    for i in 1..=3 {
        // Act
        cluster
            .get_node_mut(i)
            .on_event(Event::TimerFired(TimerKind::Heartbeat));

        // Assert
        assert_eq!(cluster.get_messages(i).len(), 0);
        assert_eq!(cluster.get_messages(i).len(), 0);
        assert_eq!(cluster.get_messages(i).len(), 0);
    }
}

#[test]
fn test_liveness_election_triggered_followers_respond() {
    // Arrange
    let mut cluster = TimelessTestCluster::new();
    cluster.add_node(1);
    cluster.add_node(2);
    cluster.add_node(3);
    cluster.connect_peers();

    let expected_vote_request_to_2 = RaftMsg::RequestVote {
        term: 1,
        candidate_id: 1,
        last_log_index: 0,
        last_log_term: 0,
    };

    let expected_vote_request_to_3 = RaftMsg::RequestVote {
        term: 1,
        candidate_id: 1,
        last_log_index: 0,
        last_log_term: 0,
    };

    let expected_heartbeat_to_2 = RaftMsg::AppendEntries {
        term: 1,
        prev_log_index: 0,
        prev_log_term: 0,
        entries: InMemoryLogEntryCollection::new(&[]),
        leader_commit: 0,
    };

    let expected_heartbeat_to_3 = RaftMsg::AppendEntries {
        term: 1,
        prev_log_index: 0,
        prev_log_term: 0,
        entries: InMemoryLogEntryCollection::new(&[]),
        leader_commit: 0,
    };

    let expected_vote_response_from_2 = RaftMsg::RequestVoteResponse {
        term: 1,
        vote_granted: true,
    };

    let expected_vote_response_from_3 = RaftMsg::RequestVoteResponse {
        term: 1,
        vote_granted: true,
    };

    let expected_heartbeat_response_from_2 = RaftMsg::AppendEntriesResponse {
        term: 1,
        success: true,
        match_index: 0,
    };

    let expected_heartbeat_response_from_3 = RaftMsg::AppendEntriesResponse {
        term: 1,
        success: true,
        match_index: 0,
    };

    // Act
    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Election));

    cluster.deliver_messages();

    // Assert - Node 2 receives vote request AND heartbeat (leader sends heartbeat immediately)
    assert_eq!(
        cluster.get_messages(2),
        vec![expected_vote_request_to_2, expected_heartbeat_to_2]
    );

    // Assert - Node 3 receives vote request AND heartbeat
    assert_eq!(
        cluster.get_messages(3),
        vec![expected_vote_request_to_3, expected_heartbeat_to_3]
    );

    // Assert - Node 1 receives vote responses AND heartbeat responses
    assert_eq!(
        cluster.get_messages(1),
        vec![
            expected_vote_response_from_2,
            expected_vote_response_from_3,
            expected_heartbeat_response_from_2,
            expected_heartbeat_response_from_3
        ]
    );

    // Assert - Node 1 is now leader
    assert_eq!(*cluster.get_node(1).role(), NodeState::Leader);
    assert_eq!(cluster.get_node(1).current_term(), 1);
}
