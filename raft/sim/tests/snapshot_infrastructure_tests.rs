// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use raft_core::{
    log_entry::LogEntry, log_entry_collection::LogEntryCollection, snapshot::Snapshot,
    snapshot::SnapshotBuilder, snapshot::SnapshotData, snapshot::SnapshotMetadata,
    state_machine::StateMachine, storage::Storage,
};
use raft_sim::in_memory_chunk_collection::InMemoryChunkCollection;
use raft_sim::in_memory_state_machine::InMemoryStateMachine;
use raft_sim::in_memory_storage::InMemoryStorage;
use raft_sim::snapshot_types::{SimSnapshotBuilder, SimSnapshotData};

// ============================================================
// STATE MACHINE SNAPSHOT TESTS
// ============================================================

#[test]
fn test_state_machine_snapshot_create_and_restore() {
    // Create a state machine and apply some operations
    let mut sm = InMemoryStateMachine::new();
    sm.apply(&"SET x=1".to_string());
    sm.apply(&"SET y=2".to_string());
    sm.apply(&"SET z=3".to_string());

    // Verify initial state
    assert_eq!(sm.get("x"), Some("1"));
    assert_eq!(sm.get("y"), Some("2"));
    assert_eq!(sm.get("z"), Some("3"));

    // Create snapshot
    let snapshot_data = sm.create_snapshot();

    // Clear state machine
    let mut sm2 = InMemoryStateMachine::new();
    assert_eq!(sm2.get("x"), None);
    assert_eq!(sm2.get("y"), None);

    // Restore from snapshot
    let result = sm2.restore_from_snapshot(&snapshot_data);
    assert!(result.is_ok(), "Snapshot restoration should succeed");

    // Verify restored state
    assert_eq!(sm2.get("x"), Some("1"));
    assert_eq!(sm2.get("y"), Some("2"));
    assert_eq!(sm2.get("z"), Some("3"));
}

#[test]
fn test_state_machine_snapshot_empty() {
    // Create empty state machine
    let sm = InMemoryStateMachine::new();

    // Create snapshot of empty state
    let snapshot_data = sm.create_snapshot();

    // Restore to new state machine
    let mut sm2 = InMemoryStateMachine::new();
    sm2.apply(&"SET temp=value".to_string());
    assert_eq!(sm2.get("temp"), Some("value"));

    let result = sm2.restore_from_snapshot(&snapshot_data);
    assert!(result.is_ok());

    // Should be empty now
    assert_eq!(sm2.get("temp"), None);
}

// ============================================================
// STORAGE SNAPSHOT TESTS
// ============================================================

#[test]
fn test_storage_snapshot_save_and_load() {
    let mut storage = InMemoryStorage::new();

    // Create a snapshot
    let snapshot_data = SimSnapshotData(vec![1, 2, 3, 4, 5]);
    let snapshot = Snapshot {
        metadata: SnapshotMetadata {
            last_included_index: 10,
            last_included_term: 5,
        },
        data: snapshot_data,
    };

    // Save snapshot
    storage.save_snapshot(snapshot.clone());

    // Load snapshot
    let loaded = storage.load_snapshot();
    assert!(loaded.is_some(), "Snapshot should be loaded");

    let loaded = loaded.unwrap();
    assert_eq!(loaded.metadata.last_included_index, 10);
    assert_eq!(loaded.metadata.last_included_term, 5);
    assert_eq!(loaded.data.0, vec![1, 2, 3, 4, 5]);
}

#[test]
fn test_storage_snapshot_metadata() {
    let mut storage = InMemoryStorage::new();

    // No snapshot initially
    assert!(storage.snapshot_metadata().is_none());

    // Save snapshot
    let snapshot = Snapshot {
        metadata: SnapshotMetadata {
            last_included_index: 15,
            last_included_term: 3,
        },
        data: SimSnapshotData(vec![1, 2, 3]),
    };
    storage.save_snapshot(snapshot);

    // Metadata should be available
    let metadata = storage.snapshot_metadata();
    assert!(metadata.is_some());
    let metadata = metadata.unwrap();
    assert_eq!(metadata.last_included_index, 15);
    assert_eq!(metadata.last_included_term, 3);
}

// ============================================================
// LOG COMPACTION TESTS
// ============================================================

#[test]
fn test_storage_discard_entries_before() {
    let mut storage = InMemoryStorage::new();

    // Add 10 log entries
    let entries: Vec<LogEntry<String>> = (1..=10)
        .map(|i| LogEntry {
            term: 1,
            payload: format!("entry_{}", i),
        })
        .collect();
    storage.append_entries(&entries);

    assert_eq!(storage.first_log_index(), 1);
    assert_eq!(storage.last_log_index(), 10);
    assert!(storage.get_entry(5).is_some());

    // Discard entries 1-5 (keep 6-10)
    // Note: discard_entries_before(6) means "discard entries with index < 6"
    storage.discard_entries_before(6);

    assert_eq!(storage.first_log_index(), 6);
    assert_eq!(storage.last_log_index(), 10);

    // Old entries should be gone
    assert!(storage.get_entry(1).is_none());
    assert!(storage.get_entry(5).is_none());

    // New entries should still be accessible
    assert!(storage.get_entry(6).is_some());
    assert_eq!(storage.get_entry(6).unwrap().payload, "entry_6");
    assert!(storage.get_entry(10).is_some());
    assert_eq!(storage.get_entry(10).unwrap().payload, "entry_10");
}

#[test]
fn test_storage_discard_all_entries() {
    let mut storage = InMemoryStorage::new();

    // Add entries
    let entries: Vec<LogEntry<String>> = (1..=5)
        .map(|i| LogEntry {
            term: 1,
            payload: format!("entry_{}", i),
        })
        .collect();
    storage.append_entries(&entries);

    // Discard all entries (entries 1-5)
    // To discard 1-5, we call discard_entries_before(6)
    storage.discard_entries_before(6);

    assert_eq!(storage.first_log_index(), 6);
    assert_eq!(storage.last_log_index(), 5); // No entries remain, but first_index moved
    assert!(storage.get_entry(1).is_none());
    assert!(storage.get_entry(5).is_none());
}

#[test]
fn test_storage_get_entries_after_compaction() {
    let mut storage = InMemoryStorage::new();

    // Add 10 entries
    let entries: Vec<LogEntry<String>> = (1..=10)
        .map(|i| LogEntry {
            term: 1,
            payload: format!("entry_{}", i),
        })
        .collect();
    storage.append_entries(&entries);

    // Discard first 5
    storage.discard_entries_before(5);

    // get_entries should work with adjusted indices
    let fetched = storage.get_entries(6, 9);
    assert_eq!(fetched.len(), 3); // entries 6, 7, 8
    assert_eq!(fetched.as_slice()[0].payload, "entry_6");
    assert_eq!(fetched.as_slice()[1].payload, "entry_7");
    assert_eq!(fetched.as_slice()[2].payload, "entry_8");

    // Requesting compacted entries should return empty
    let fetched = storage.get_entries(1, 5);
    assert_eq!(fetched.len(), 0);
}

#[test]
fn test_storage_last_log_term_after_compaction() {
    let mut storage = InMemoryStorage::new();

    // Add entries with different terms
    storage.append_entries(&[
        LogEntry {
            term: 1,
            payload: "entry_1".to_string(),
        },
        LogEntry {
            term: 2,
            payload: "entry_2".to_string(),
        },
        LogEntry {
            term: 3,
            payload: "entry_3".to_string(),
        },
    ]);

    assert_eq!(storage.last_log_term(), 3);

    // Discard all entries
    storage.discard_entries_before(3);

    // When all entries are compacted but we have a snapshot, last_log_term should come from snapshot
    let snapshot = Snapshot {
        metadata: SnapshotMetadata {
            last_included_index: 3,
            last_included_term: 3,
        },
        data: SimSnapshotData(vec![]),
    };
    storage.save_snapshot(snapshot);

    assert_eq!(storage.last_log_term(), 3); // From snapshot
}

// ============================================================
// SNAPSHOT CHUNKING TESTS
// ============================================================

#[test]
fn test_snapshot_data_chunking() {
    let data = SimSnapshotData(vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);

    // Get chunk at offset 0
    let chunk = data.chunk_at(0, 3);
    assert!(chunk.is_some());
    assert_eq!(
        chunk.unwrap(),
        InMemoryChunkCollection::from_vec(vec![1, 2, 3])
    );

    // Get chunk at offset 3
    let chunk = data.chunk_at(3, 3);
    assert!(chunk.is_some());
    assert_eq!(
        chunk.unwrap(),
        InMemoryChunkCollection::from_vec(vec![4, 5, 6])
    );

    // Get chunk that extends past end
    let chunk = data.chunk_at(8, 5);
    assert!(chunk.is_some());
    assert_eq!(
        chunk.unwrap(),
        InMemoryChunkCollection::from_vec(vec![9, 10])
    ); // Only remaining bytes

    // Out of bounds
    let chunk = data.chunk_at(20, 5);
    assert!(chunk.is_none());
}

#[test]
fn test_storage_get_snapshot_chunk() {
    let mut storage = InMemoryStorage::new();

    // Save a snapshot
    let snapshot = Snapshot {
        metadata: SnapshotMetadata {
            last_included_index: 5,
            last_included_term: 1,
        },
        data: SimSnapshotData(vec![1, 2, 3, 4, 5, 6, 7, 8]),
    };
    storage.save_snapshot(snapshot);

    // Get first chunk
    let chunk = storage.get_snapshot_chunk(0, 3);
    assert!(chunk.is_some());
    let chunk = chunk.unwrap();
    assert_eq!(chunk.offset, 0);
    assert_eq!(chunk.data, InMemoryChunkCollection::from_vec(vec![1, 2, 3]));
    assert!(!chunk.done);

    // Get last chunk
    let chunk = storage.get_snapshot_chunk(6, 3);
    assert!(chunk.is_some());
    let chunk = chunk.unwrap();
    assert_eq!(chunk.offset, 6);
    assert_eq!(chunk.data, InMemoryChunkCollection::from_vec(vec![7, 8]));
    assert!(chunk.done); // Last chunk
}

// ============================================================
// SNAPSHOT BUILDER TESTS
// ============================================================

#[test]
fn test_snapshot_builder_accumulate_chunks() {
    let mut builder = SimSnapshotBuilder::new();

    // Add chunks in order
    let result = builder.add_chunk(0, InMemoryChunkCollection::from_vec(vec![1, 2, 3]));
    assert!(result.is_ok());

    let result = builder.add_chunk(3, InMemoryChunkCollection::from_vec(vec![4, 5, 6]));
    assert!(result.is_ok());

    let result = builder.add_chunk(6, InMemoryChunkCollection::from_vec(vec![7, 8]));
    assert!(result.is_ok());

    // Check if complete (assuming total size is 8)
    assert!(builder.is_complete(8));

    // Build final snapshot data
    let snapshot_data = builder.build();
    assert!(snapshot_data.is_ok());
    let snapshot_data = snapshot_data.unwrap();
    assert_eq!(snapshot_data.0, vec![1, 2, 3, 4, 5, 6, 7, 8]);
}

#[test]
fn test_snapshot_builder_incomplete() {
    let mut builder = SimSnapshotBuilder::new();

    builder
        .add_chunk(0, InMemoryChunkCollection::from_vec(vec![1, 2, 3]))
        .unwrap();
    builder
        .add_chunk(3, InMemoryChunkCollection::from_vec(vec![4, 5]))
        .unwrap();

    // Not complete yet
    assert!(!builder.is_complete(10));

    // Building incomplete snapshot should work (we just have partial data)
    let snapshot_data = builder.build();
    assert!(snapshot_data.is_ok());
    assert_eq!(snapshot_data.unwrap().0, vec![1, 2, 3, 4, 5]);
}

#[test]
fn test_storage_snapshot_transfer_workflow() {
    let mut storage = InMemoryStorage::new();

    // Save original snapshot
    let original = Snapshot {
        metadata: SnapshotMetadata {
            last_included_index: 10,
            last_included_term: 2,
        },
        data: SimSnapshotData(vec![10, 20, 30, 40, 50]),
    };
    storage.save_snapshot(original.clone());

    // Simulate transferring snapshot to another storage
    let mut receiver_storage = InMemoryStorage::new();
    let mut builder = receiver_storage.begin_snapshot_transfer();

    // Transfer in chunks
    let chunk = storage.get_snapshot_chunk(0, 2).unwrap();
    builder.add_chunk(chunk.offset, chunk.data).unwrap();

    let chunk = storage.get_snapshot_chunk(2, 2).unwrap();
    builder.add_chunk(chunk.offset, chunk.data).unwrap();

    let chunk = storage.get_snapshot_chunk(4, 2).unwrap();
    builder.add_chunk(chunk.offset, chunk.data).unwrap();

    // Finalize on receiver
    let result = receiver_storage.finalize_snapshot(builder, original.metadata.clone());
    assert!(result.is_ok());

    // Verify receiver has the snapshot
    let loaded = receiver_storage.load_snapshot();
    assert!(loaded.is_some());
    let loaded = loaded.unwrap();
    assert_eq!(loaded.metadata.last_included_index, 10);
    assert_eq!(loaded.data.0, vec![10, 20, 30, 40, 50]);
}
