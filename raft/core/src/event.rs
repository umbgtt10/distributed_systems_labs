// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::{
    chunk_collection::ChunkCollection, log_entry_collection::LogEntryCollection,
    raft_messages::RaftMsg, timer_service::TimerKind, types::NodeId,
};

pub enum Event<
    P: Clone,
    L: LogEntryCollection<Payload = P> + Clone,
    C: ChunkCollection + Clone,
> {
    Message { from: NodeId, msg: RaftMsg<P, L, C> },
    TimerFired(TimerKind),
    ClientCommand(P),
}
