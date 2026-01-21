// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use heapless::Vec;
use raft_core::{
    collections::config_change_collection::{CollectionError, ConfigChangeCollection},
    log_entry::ConfigurationChange,
    types::LogIndex,
};

#[derive(Clone, Debug)]
pub struct EmbassyConfigChangeCollection {
    changes: Vec<(LogIndex, ConfigurationChange), 10>,
}

impl ConfigChangeCollection for EmbassyConfigChangeCollection {
    type Iter<'a> = EmbassyConfigChangeIter<'a>;

    fn new() -> Self {
        Self {
            changes: Vec::new(),
        }
    }

    fn push(
        &mut self,
        index: LogIndex,
        change: ConfigurationChange,
    ) -> Result<(), CollectionError> {
        self.changes
            .push((index, change))
            .map_err(|_| CollectionError::Full)
    }

    fn len(&self) -> usize {
        self.changes.len()
    }

    fn clear(&mut self) {
        self.changes.clear();
    }

    fn iter(&self) -> Self::Iter<'_> {
        EmbassyConfigChangeIter {
            inner: self.changes.iter(),
        }
    }
}

pub struct EmbassyConfigChangeIter<'a> {
    inner: core::slice::Iter<'a, (LogIndex, ConfigurationChange)>,
}

impl<'a> Iterator for EmbassyConfigChangeIter<'a> {
    type Item = (LogIndex, &'a ConfigurationChange);

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|(idx, change)| (*idx, change))
    }
}
