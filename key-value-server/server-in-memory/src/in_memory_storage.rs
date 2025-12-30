// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use key_value_server_core::{Storage, StorageError};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::Mutex;

/// In-memory storage implementation using HashMap with Mutex for concurrency
#[derive(Clone)]
pub struct InMemoryStorage {
    data: Arc<Mutex<HashMap<String, (String, u64)>>>,
}

impl InMemoryStorage {
    pub fn new() -> Self {
        Self {
            data: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

#[async_trait::async_trait]
impl Storage for InMemoryStorage {
    async fn get(&self, key: &str) -> Result<(String, u64), StorageError> {
        let data = self.data.lock().await;

        data.get(key)
            .map(|(value, version)| (value.clone(), *version))
            .ok_or_else(|| StorageError::KeyNotFound(key.to_string()))
    }

    async fn put(
        &self,
        key: &str,
        value: String,
        expected_version: u64,
    ) -> Result<u64, StorageError> {
        let mut data = self.data.lock().await;

        if expected_version == 0 {
            // Create new key
            if data.contains_key(key) {
                return Err(StorageError::KeyAlreadyExists(key.to_string()));
            }
            data.insert(key.to_string(), (value, 1));
            Ok(1)
        } else {
            // Update existing key
            match data.get(key) {
                Some((_, current_version)) => {
                    if *current_version == expected_version {
                        let new_version = expected_version + 1;
                        data.insert(key.to_string(), (value, new_version));
                        Ok(new_version)
                    } else {
                        Err(StorageError::VersionMismatch {
                            expected: expected_version,
                            actual: *current_version,
                        })
                    }
                }
                None => Err(StorageError::KeyNotFound(key.to_string())),
            }
        }
    }

    async fn print_all(&self) {
        let data = self.data.lock().await;

        println!("\n=== Final Storage State ===");
        if data.is_empty() {
            println!("  No keys in storage");
        } else {
            let mut keys: Vec<_> = data.keys().cloned().collect();
            keys.sort();
            for key in keys {
                if let Some((value, version)) = data.get(&key) {
                    println!("  '{}' -> value='{}', version={}", key, value, version);
                }
            }
        }
        println!("===========================\n");
    }
}

