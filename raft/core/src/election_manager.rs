// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::{
    log_entry_collection::LogEntryCollection,
    node_collection::NodeCollection,
    node_state::NodeState,
    raft_messages::RaftMsg,
    storage::Storage,
    timer_service::TimerService,
    types::{LogIndex, NodeId, Term},
};

/// Manages elections: voting, vote counting, state transitions
pub struct ElectionManager<C, TS>
where
    C: NodeCollection,
    TS: TimerService,
{
    votes_received: C,
    timer_service: TS,
}

impl<C, TS> ElectionManager<C, TS>
where
    C: NodeCollection,
    TS: TimerService,
{
    pub fn new(timer_service: TS) -> Self {
        Self {
            votes_received: C::new(),
            timer_service,
        }
    }

    /// Start a new election - returns vote request message to broadcast
    pub fn start_election<P, L, S>(
        &mut self,
        node_id: NodeId,
        current_term: &mut Term,
        storage: &mut S,
        role: &mut NodeState,
    ) -> RaftMsg<P, L>
    where
        P: Clone,
        L: LogEntryCollection<Payload = P> + Clone,
        S: Storage<Payload = P, LogEntryCollection = L> + Clone,
    {
        *current_term += 1;
        storage.set_current_term(*current_term);
        storage.set_voted_for(Some(node_id));

        self.votes_received.clear();
        self.votes_received.push(node_id).unwrap(); // Vote for self

        *role = NodeState::Candidate;
        self.timer_service.reset_election_timer();

        RaftMsg::RequestVote {
            term: *current_term,
            candidate_id: node_id,
            last_log_index: storage.last_log_index(),
            last_log_term: storage.last_log_term(),
        }
    }

    /// Handle incoming vote request - returns response message
    #[allow(clippy::too_many_arguments)]
    pub fn handle_vote_request<P, L, S>(
        &mut self,
        term: Term,
        candidate_id: NodeId,
        last_log_index: LogIndex,
        last_log_term: Term,
        current_term: &mut Term,
        storage: &mut S,
        role: &mut NodeState,
    ) -> RaftMsg<P, L>
    where
        P: Clone,
        L: LogEntryCollection<Payload = P> + Clone,
        S: Storage<Payload = P, LogEntryCollection = L> + Clone,
    {
        // Update term if necessary
        if term > *current_term {
            *current_term = term;
            storage.set_current_term(term);
            *role = NodeState::Follower;
            storage.set_voted_for(None);
        }

        let vote_granted = self.should_grant_vote(
            term,
            *current_term,
            candidate_id,
            last_log_index,
            last_log_term,
            storage,
        );

        if vote_granted {
            storage.set_voted_for(Some(candidate_id));
            self.timer_service.reset_election_timer();
        }

        RaftMsg::RequestVoteResponse {
            term: *current_term,
            vote_granted,
        }
    }

    /// Handle vote response - returns true if we should become leader
    pub fn handle_vote_response(
        &mut self,
        from: NodeId,
        term: Term,
        vote_granted: bool,
        current_term: &Term,
        role: &NodeState,
        total_peers: usize,
    ) -> bool {
        if term > *current_term {
            return false; // Caller should step down
        }

        if *role != NodeState::Candidate || term != *current_term || !vote_granted {
            return false;
        }

        self.votes_received.push(from).ok();

        let total_nodes = total_peers + 1;
        let votes = self.votes_received.len();

        votes > total_nodes / 2 // Won election if majority
    }

    fn should_grant_vote<P, L, S>(
        &self,
        term: Term,
        current_term: Term,
        candidate_id: NodeId,
        last_log_index: LogIndex,
        last_log_term: Term,
        storage: &S,
    ) -> bool
    where
        L: LogEntryCollection,
        S: Storage<Payload = P, LogEntryCollection = L>,
    {
        // Reject if term is stale
        if term < current_term {
            return false;
        }

        // Reject if already voted for someone else
        if let Some(voted) = storage.voted_for() {
            if voted != candidate_id {
                return false;
            }
        }

        // Check if candidate's log is at least as up-to-date
        let our_term = storage.last_log_term();
        let our_index = storage.last_log_index();

        last_log_term > our_term || (last_log_term == our_term && last_log_index >= our_index)
    }

    pub fn timer_service(&self) -> &TS {
        &self.timer_service
    }

    pub fn timer_service_mut(&mut self) -> &mut TS {
        &mut self.timer_service
    }
}
