// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use raft_core::log_entry_collection::LogEntryCollection;
use raft_core::observer::{EventLevel, Observer, Role, TimerKind};
use raft_core::raft_messages::RaftMsg;
use raft_core::types::{LogIndex, NodeId, Term};

/// Observer implementation for Embassy that logs essential events
#[derive(Clone)]
pub struct EmbassyObserver<P: Clone, L: LogEntryCollection<Payload = P> + Clone> {
    level: EventLevel,
    _phantom_p: core::marker::PhantomData<P>,
    _phantom_l: core::marker::PhantomData<L>,
}

impl<P: Clone, L: LogEntryCollection<Payload = P> + Clone> EmbassyObserver<P, L> {
    pub fn new(level: EventLevel) -> Self {
        Self {
            level,
            _phantom_p: core::marker::PhantomData,
            _phantom_l: core::marker::PhantomData,
        }
    }
}

impl<P: Clone, L: LogEntryCollection<Payload = P> + Clone> Observer for EmbassyObserver<P, L> {
    type Payload = P;
    type LogEntries = L;

    fn min_level(&self) -> EventLevel {
        self.level
    }

    fn leader_elected(&mut self, node: NodeId, term: Term) {
        if self.level >= EventLevel::Essential {
            info!("Node {} became LEADER (term {})", node, term);
        }
        // Update cluster leader tracking (non-blocking try_lock)
        if let Ok(mut leader) = crate::cluster::CURRENT_LEADER.try_lock() {
            *leader = Some(node);
        }
    }

    fn role_changed(&mut self, node: NodeId, old: Role, new: Role, term: Term) {
        if self.level >= EventLevel::Info {
            info!(
                "Node {} role changed: {:?} -> {:?} (term {})",
                node, old, new, term
            );
        }
    }

    fn leader_lost(&mut self, node: NodeId, old_term: Term, new_term: Term) {
        if self.level >= EventLevel::Essential {
            info!(
                "Node {} lost leadership (term {} -> {})",
                node, old_term, new_term
            );
        }
    }

    fn election_started(&mut self, node: NodeId, term: Term) {
        if self.level >= EventLevel::Info {
            info!("Node {} started election (term {})", node, term);
        }
    }

    fn election_timeout(&mut self, node: NodeId, term: Term) {
        if self.level >= EventLevel::Info {
            info!("Node {} election timeout (term {})", node, term);
        }
    }

    fn commit_advanced(&mut self, node: NodeId, old_index: LogIndex, new_index: LogIndex) {
        if self.level >= EventLevel::Info {
            info!(
                "Node {} commit advanced: {} -> {}",
                node, old_index, new_index
            );
        }
    }

    fn log_appended(&mut self, node: NodeId, count: usize, last_index: LogIndex, last_term: Term) {
        if self.level >= EventLevel::Debug {
            info!(
                "Node {} appended {} entries (last: index={}, term={})",
                node, count, last_index, last_term
            );
        }
    }

    fn log_truncated(&mut self, node: NodeId, from_index: LogIndex) {
        if self.level >= EventLevel::Info {
            info!("Node {} log truncated from index {}", node, from_index);
        }
    }

    fn vote_requested(
        &mut self,
        candidate: NodeId,
        voter: NodeId,
        term: Term,
        last_log_index: LogIndex,
        last_log_term: Term,
    ) {
        if self.level >= EventLevel::Debug {
            info!(
                "Node {} vote requested by {} (term {}, last_log: index={}, term={})",
                voter, candidate, term, last_log_index, last_log_term
            );
        }
    }

    fn vote_granted(&mut self, candidate: NodeId, voter: NodeId, granted: bool, term: Term) {
        if self.level >= EventLevel::Debug {
            info!(
                "Node {} {} vote to {} (term {})",
                voter,
                if granted { "granted" } else { "denied" },
                candidate,
                term
            );
        }
    }

    fn voted_for(&mut self, voter: NodeId, candidate: NodeId, term: Term) {
        if self.level >= EventLevel::Info {
            info!("Node {} voted for {} (term {})", voter, candidate, term);
        }
    }

    fn heartbeat_sent(&mut self, leader: NodeId, follower: NodeId, term: Term) {
        if self.level >= EventLevel::Trace {
            info!(
                "Node {} sent heartbeat to {} (term {})",
                leader, follower, term
            );
        }
    }

    fn append_entries_sent(
        &mut self,
        leader: NodeId,
        follower: NodeId,
        entry_count: usize,
        term: Term,
    ) {
        if self.level >= EventLevel::Debug {
            info!(
                "Node {} sent {} entries to {} (term {})",
                leader, entry_count, follower, term
            );
        }
    }

    fn append_entries_response(
        &mut self,
        follower: NodeId,
        leader: NodeId,
        success: bool,
        match_index: LogIndex,
        term: Term,
    ) {
        if self.level >= EventLevel::Debug {
            info!(
                "Node {} -> Node {}: AppendEntries {} (match_index={}, term={})",
                follower,
                leader,
                if success { "success" } else { "failed" },
                match_index,
                term
            );
        }
    }

    fn timer_fired(&mut self, node: NodeId, timer: TimerKind, term: Term) {
        if self.level >= EventLevel::Debug {
            info!("Node {} {:?} timer fired (term {})", node, timer, term);
        }
    }

    fn timer_reset(&mut self, node: NodeId, timer: TimerKind, term: Term) {
        if self.level >= EventLevel::Trace {
            info!("Node {} {:?} timer reset (term {})", node, timer, term);
        }
    }

    fn message_sent(
        &mut self,
        from: NodeId,
        to: NodeId,
        _msg: &RaftMsg<Self::Payload, Self::LogEntries>,
    ) {
        if self.level >= EventLevel::Trace {
            info!("Node {} -> Node {}: message sent", from, to);
        }
    }

    fn message_received(
        &mut self,
        to: NodeId,
        from: NodeId,
        _msg: &RaftMsg<Self::Payload, Self::LogEntries>,
    ) {
        if self.level >= EventLevel::Trace {
            info!("Node {} <- Node {}: message received", to, from);
        }
    }

    fn higher_term_discovered(
        &mut self,
        node: NodeId,
        old_term: Term,
        new_term: Term,
        from_peer: NodeId,
    ) {
        if self.level >= EventLevel::Info {
            info!(
                "Node {} discovered higher term from {} (term {} -> {})",
                node, from_peer, old_term, new_term
            );
        }
    }
}
