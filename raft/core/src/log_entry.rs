// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::types::{NodeId, Term};

#[derive(Clone, Debug, PartialEq)]
pub enum EntryType<P> {
    Command(P),
    ConfigChange(ConfigurationChange),
}

#[derive(Clone, Debug, PartialEq)]
pub enum ConfigurationChange {
    AddServer(NodeId),
    RemoveServer(NodeId),
}

#[derive(Clone, Debug, PartialEq)]
pub struct LogEntry<P> {
    pub term: Term,
    pub entry_type: EntryType<P>,
}
