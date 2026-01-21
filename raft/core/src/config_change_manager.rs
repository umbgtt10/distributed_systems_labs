// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::{
    config_change_collection::ConfigChangeCollection,
    configuration::Configuration,
    log_entry::ConfigurationChange,
    log_replication_manager::LogReplicationManager,
    map_collection::MapCollection,
    node_collection::NodeCollection,
    node_state::NodeState,
    observer::Observer,
    types::{LogIndex, NodeId},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigError {
    NotLeader,
    ConfigChangeInProgress,
    NodeAlreadyExists,
    NodeNotFound,
    CannotRemoveLastNode,
}

/// Manages cluster configuration and configuration changes
pub struct ConfigChangeManager<C, M>
where
    C: NodeCollection,
{
    config: Configuration<C>,
    pending_change: Option<LogIndex>,
    _phantom: core::marker::PhantomData<M>,
}

impl<C, M> ConfigChangeManager<C, M>
where
    C: NodeCollection,
    M: MapCollection,
{
    pub fn new(config: Configuration<C>) -> Self {
        Self {
            config,
            pending_change: None,
            _phantom: core::marker::PhantomData,
        }
    }

    pub fn config(&self) -> &Configuration<C> {
        &self.config
    }

    /// Check if a configuration change at the given index has been committed
    pub fn is_change_committed(&self, index: LogIndex, commit_index: LogIndex) -> bool {
        index <= commit_index
    }

    /// Add a server to the cluster configuration
    ///
    /// This initiates a configuration change to add a new server to the cluster.
    /// The change will be replicated through the Raft log and applied when committed.
    ///
    /// # Safety Constraints
    /// - Only the leader can add servers
    /// - Only one configuration change can be in progress at a time
    /// - The server must not already be in the cluster
    pub fn add_server(
        &mut self,
        node_id: NodeId,
        self_id: NodeId,
        is_leader: bool,
        commit_index: LogIndex,
    ) -> Result<ConfigurationChange, ConfigError> {
        // Check if we're the leader
        if !is_leader {
            return Err(ConfigError::NotLeader);
        }

        // Check if there's already a config change in progress
        if let Some(pending_index) = self.pending_change {
            if !self.is_change_committed(pending_index, commit_index) {
                return Err(ConfigError::ConfigChangeInProgress);
            }
        }

        // Check if node already exists in configuration
        if node_id == self_id || self.config.contains(node_id) {
            return Err(ConfigError::NodeAlreadyExists);
        }

        // Return the configuration change to be submitted
        Ok(ConfigurationChange::AddServer(node_id))
    }

    /// Remove a server from the cluster configuration
    ///
    /// This initiates a configuration change to remove a server from the cluster.
    /// The change will be replicated through the Raft log and applied when committed.
    ///
    /// # Safety Constraints
    /// - Only the leader can remove servers
    /// - Only one configuration change can be in progress at a time
    /// - The server must be in the cluster
    /// - Cannot remove the last server (cluster would be empty)
    pub fn remove_server(
        &mut self,
        node_id: NodeId,
        self_id: NodeId,
        is_leader: bool,
        commit_index: LogIndex,
    ) -> Result<ConfigurationChange, ConfigError> {
        // Check if we're the leader
        if !is_leader {
            return Err(ConfigError::NotLeader);
        }

        // Check if there's already a config change in progress
        if let Some(pending_index) = self.pending_change {
            if !self.is_change_committed(pending_index, commit_index) {
                return Err(ConfigError::ConfigChangeInProgress);
            }
        }

        // Check if this would leave the cluster empty
        if self.config.size() == 1 {
            return Err(ConfigError::CannotRemoveLastNode);
        }

        // Check if node exists in configuration (including self)
        if node_id != self_id && !self.config.contains(node_id) {
            return Err(ConfigError::NodeNotFound);
        }

        // Return the configuration change to be submitted
        Ok(ConfigurationChange::RemoveServer(node_id))
    }

    /// Track a pending configuration change
    pub fn track_pending_change(&mut self, index: LogIndex) {
        self.pending_change = Some(index);
    }

    /// Apply configuration changes that have been committed
    pub fn apply_changes<CCC, O, P, L, CC>(
        &mut self,
        changes: CCC,
        self_id: NodeId,
        last_log_index: LogIndex,
        replication: &mut LogReplicationManager<M>,
        observer: &mut O,
        current_role: &mut NodeState,
    ) where
        CCC: ConfigChangeCollection,
        O: Observer<Payload = P, LogEntries = L, ChunkCollection = CC>,
    {
        for (index, change) in changes.iter() {
            match change {
                ConfigurationChange::AddServer(node_id) => {
                    observer.configuration_change_applied(self_id, *node_id, true);

                    // Add to configuration
                    let mut new_members = C::new();
                    for existing_id in self.config.members.iter() {
                        let _ = new_members.push(existing_id);
                    }
                    let _ = new_members.push(*node_id);
                    self.config = Configuration::new(new_members);

                    // Initialize replication state for new member if we're leader
                    if *current_role == NodeState::Leader {
                        let next_index = last_log_index + 1;
                        replication.next_index_mut().insert(*node_id, next_index);
                        replication.match_index_mut().insert(*node_id, 0);
                    }
                }
                ConfigurationChange::RemoveServer(node_id) => {
                    observer.configuration_change_applied(self_id, *node_id, false);

                    // Remove from configuration
                    let mut new_members = C::new();
                    for existing_id in self.config.members.iter() {
                        if existing_id != *node_id {
                            let _ = new_members.push(existing_id);
                        }
                    }
                    self.config = Configuration::new(new_members);

                    // Clean up replication state if we're leader
                    if *current_role == NodeState::Leader {
                        replication.next_index_mut().remove(*node_id);
                        replication.match_index_mut().remove(*node_id);
                    }

                    // If we removed ourselves, step down
                    if *node_id == self_id {
                        *current_role = NodeState::Follower;
                        // Note: Observer notification for role change should be handled by caller
                    }
                }
            }

            // Clear pending config change flag if this change was committed
            if let Some(pending_index) = self.pending_change {
                if index == pending_index {
                    self.pending_change = None;
                }
            }
        }
    }
}
