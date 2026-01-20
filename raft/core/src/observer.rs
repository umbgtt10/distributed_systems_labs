// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! Observability trait for Raft node events
//!
//! Provides a platform-agnostic way to observe Raft internals for:
//! - Debugging (defmt, println)
//! - Monitoring (metrics, structured logging)
//! - Testing (capturing events for assertions)
//!
//! The implementation handles filtering and formatting internally,
//! similar to Log4Net/Log4j architecture.

use crate::types::{LogIndex, NodeId, Term};

/// Importance level for events (filter threshold)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum EventLevel {
    /// No logging (useful for tests)
    None = 0,
    /// Must-know events (leader changes, critical state transitions)
    Essential = 1,
    /// Should-know events (elections, commits, important operations)
    Info = 2,
    /// Nice-to-know events (votes, heartbeats, detailed operations)
    Debug = 3,
    /// Every detail (every message, every timer)
    Trace = 4,
}

/// Node roles
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Follower,
    Candidate,
    Leader,
}

/// Timer types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimerKind {
    Election,
    Heartbeat,
}

/// Observer trait for Raft node events
///
/// Implementations should:
/// 1. Check min_level() to determine what events to log
/// 2. Format output according to platform (embedded, cloud, test)
/// 3. Handle filtering internally to keep RaftNode code clean
///
/// # Associated Types
///
/// - `Payload`: The type of data stored in log entries (must match RaftMsg)
/// - `LogEntries`: The collection type for log entries (must match RaftMsg)
///
/// # Example
///
/// ```ignore
/// struct MyObserver { min_level: EventLevel }
///
/// impl Observer for MyObserver {
///     type Payload = String;
///     type LogEntries = Vec<LogEntry<String>>;
///
///     fn min_level(&self) -> EventLevel { self.min_level }
///
///     fn leader_elected(&mut self, node: NodeId, term: Term) {
///         if self.min_level >= EventLevel::Essential {
///             println!("Node {} became LEADER (term {})", node, term);
///         }
///     }
/// }
/// ```
pub trait Observer {
    /// The type of payload stored in log entries
    type Payload: Clone;

    /// The collection type for log entries
    type LogEntries: crate::log_entry_collection::LogEntryCollection<Payload = Self::Payload>
        + Clone;

    /// The collection type for snapshot chunks
    type ChunkCollection: crate::chunk_collection::ChunkCollection + Clone;

    /// Get the minimum event level this observer cares about
    fn min_level(&self) -> EventLevel;

    // === Leadership Events (Essential) ===

    /// Node became leader for a term
    fn leader_elected(&mut self, node: NodeId, term: Term);

    /// Node's role changed
    fn role_changed(&mut self, node: NodeId, old: Role, new: Role, term: Term);

    /// Node lost leadership (stepped down or higher term discovered)
    fn leader_lost(&mut self, node: NodeId, old_term: Term, new_term: Term);

    // === Election Events (Info) ===

    /// Node started an election
    fn election_started(&mut self, node: NodeId, term: Term);

    /// Election timeout fired
    fn election_timeout(&mut self, node: NodeId, term: Term);

    // === Log Events (Info) ===

    /// Commit index advanced
    fn commit_advanced(&mut self, node: NodeId, old_index: LogIndex, new_index: LogIndex);

    /// Log entries appended
    fn log_appended(&mut self, node: NodeId, count: usize, last_index: LogIndex, last_term: Term);

    /// Log truncated (conflict resolution)
    fn log_truncated(&mut self, node: NodeId, from_index: LogIndex);

    // === Voting Events (Debug) ===

    /// Vote requested from this node
    fn vote_requested(
        &mut self,
        candidate: NodeId,
        voter: NodeId,
        term: Term,
        last_log_index: LogIndex,
        last_log_term: Term,
    );

    /// Vote granted or denied
    fn vote_granted(&mut self, candidate: NodeId, voter: NodeId, granted: bool, term: Term);

    /// This node voted for someone
    fn voted_for(&mut self, voter: NodeId, candidate: NodeId, term: Term);

    // === Replication Events (Debug) ===

    /// Heartbeat sent to follower
    fn heartbeat_sent(&mut self, leader: NodeId, follower: NodeId, term: Term);

    /// AppendEntries sent with log entries
    fn append_entries_sent(
        &mut self,
        leader: NodeId,
        follower: NodeId,
        entry_count: usize,
        term: Term,
    );

    /// AppendEntries response received
    fn append_entries_response(
        &mut self,
        follower: NodeId,
        leader: NodeId,
        success: bool,
        match_index: LogIndex,
        term: Term,
    );

    // === Timer Events (Debug) ===

    /// Timer fired
    fn timer_fired(&mut self, node: NodeId, timer: TimerKind, term: Term);

    /// Timer reset
    fn timer_reset(&mut self, node: NodeId, timer: TimerKind, term: Term);

    // === Message Events (Trace) ===

    /// Message sent to peer
    fn message_sent(
        &mut self,
        from: NodeId,
        to: NodeId,
        msg: &crate::raft_messages::RaftMsg<Self::Payload, Self::LogEntries, Self::ChunkCollection>,
    );

    /// Message received from peer
    fn message_received(
        &mut self,
        to: NodeId,
        from: NodeId,
        msg: &crate::raft_messages::RaftMsg<Self::Payload, Self::LogEntries, Self::ChunkCollection>,
    );

    /// Higher term discovered in message
    fn higher_term_discovered(
        &mut self,
        node: NodeId,
        old_term: Term,
        new_term: Term,
        from_peer: NodeId,
    );
}
