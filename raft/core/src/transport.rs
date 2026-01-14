// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::{raft_messages::RaftMsg, types::NodeId};

pub trait Transport {
    type Payload;

    fn send(
        &mut self,
        target: NodeId,
        msg: RaftMsg<Self::Payload>,
    );
}
