// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::snapshot::{SnapshotData, SnapshotError};

pub trait StateMachine {
    type Payload;
    type SnapshotData: SnapshotData;

    fn apply(&mut self, payload: &Self::Payload);
    fn get(&self, key: &str) -> Option<&str>;

    // === Snapshot Methods ===

    fn create_snapshot(&self) -> Self::SnapshotData;
    fn restore_from_snapshot(&mut self, data: &Self::SnapshotData) -> Result<(), SnapshotError>;
}
