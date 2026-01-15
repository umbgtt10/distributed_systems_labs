// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::{
    log_entry_collection::LogEntryCollection, raft_messages::RaftMsg, timer_service::TimerKind,
    types::NodeId,
};

pub enum Event<P, L: LogEntryCollection<Payload = P>> {
    Message { from: NodeId, msg: RaftMsg<P, L> },
    TimerFired(TimerKind),
    ClientCommand(P),
}
