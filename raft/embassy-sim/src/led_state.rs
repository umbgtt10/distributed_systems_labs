// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use raft_core::node_state::NodeState;

/// LED state indicator for Raft node
/// 🟢 Green  = Leader
/// 🟡 Yellow = Candidate
/// 🔴 Red    = Follower
pub struct LedState {
    node_id: u8,
    current_state: NodeState,
}

impl LedState {
    pub fn new(node_id: u8) -> Self {
        Self {
            node_id,
            current_state: NodeState::Follower,
        }
    }

    pub fn update(&mut self, new_state: NodeState) {
        if self.current_state != new_state {
            self.current_state = new_state;
            self.display();
        }
    }

    fn display(&self) {
        let (color, emoji) = match self.current_state {
            NodeState::Leader => ("Green", "🟢"),
            NodeState::Candidate => ("Yellow", "🟡"),
            NodeState::Follower => ("Red", "🔴"),
        };

        crate::info!(
            "{} Node {} LED: {} ({:?})",
            emoji,
            self.node_id,
            color,
            self.current_state
        );
    }
}
