// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::{raft_messages::RaftMsg, timer::TimerKind, types::NodeId};

pub enum Event<P> {
    Message { from: NodeId, msg: RaftMsg<P> },
    TimerFired(TimerKind),
    ClientCommand(P),
}
