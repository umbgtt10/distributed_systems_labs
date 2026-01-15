// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use raft_core::{
    event::Event, node_collection::NodeCollection, node_state::NodeState, raft_messages::RaftMsg,
    timer_service::TimerKind,
};
use raft_sim::{
    timeless_test_cluster::TimelessTestCluster, vec_node_collection::VecNodeCollection,
};

#[test]
fn test_empty_cluster() {
    // Arrange
    let cluster = TimelessTestCluster::new();

    // Assert
    assert_eq!(cluster.get_node_ids().len(), 0);
}

#[test]
fn test_add_node() {
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
fn test_connection() {
    // Arrange
    let mut cluster = TimelessTestCluster::new();
    cluster.add_node(1);
    cluster.add_node(2);
    cluster.add_node(3);
    cluster.connect_peers();

    let expected_peers_1 = {
        let mut peers = VecNodeCollection::new();
        peers.push(2).unwrap();
        peers.push(3).unwrap();
        peers
    };

    let expected_peers_2 = {
        let mut peers = VecNodeCollection::new();
        peers.push(1).unwrap();
        peers.push(3).unwrap();
        peers
    };

    let expected_peers_3 = {
        let mut peers = VecNodeCollection::new();
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
fn test_heartbeat_followers_nothing_delivered() {
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
fn test_election_triggered_followers_respond() {
    // Arrange
    let mut cluster = TimelessTestCluster::new();
    cluster.add_node(1);
    cluster.add_node(2);
    cluster.add_node(3);
    cluster.connect_peers();

    let expected_from_1_to_2 = RaftMsg::RequestVote {
        term: 1,
        candidate_id: 1,
        last_log_index: 0,
        last_log_term: 0,
    };

    let expected_from_1_to_3 = RaftMsg::RequestVote {
        term: 1,
        candidate_id: 1,
        last_log_index: 0,
        last_log_term: 0,
    };

    let expected_response_from_2_to_1 = RaftMsg::RequestVoteResponse {
        term: 1,
        vote_granted: true,
    };

    let expected_response_from_3_to_1 = RaftMsg::RequestVoteResponse {
        term: 1,
        vote_granted: true,
    };

    // Act
    cluster
        .get_node_mut(1)
        .on_event(Event::TimerFired(TimerKind::Election));

    cluster.deliver_messages();

    // Assert
    assert_eq!(cluster.get_messages(2), vec![expected_from_1_to_2]);
    assert_eq!(cluster.get_messages(3), vec![expected_from_1_to_3]);
    assert_eq!(
        cluster.get_messages(1),
        vec![expected_response_from_2_to_1, expected_response_from_3_to_1]
    );

    assert_eq!(*cluster.get_node(1).role(), NodeState::Leader);
    assert_eq!(cluster.get_node(1).current_term(), 1);
}
