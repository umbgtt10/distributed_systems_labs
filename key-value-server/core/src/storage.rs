use crate::StorageError;

/// Trait for abstracting key-value storage with versioning
/// Different implementations handle concurrency internally
#[async_trait::async_trait]
pub trait Storage: Send + Sync {
    /// Get a value and its current version
    /// Returns error if the key doesn't exist
    async fn get(&self, key: &str) -> Result<(String, u64), StorageError>;

    /// Put a value with optimistic concurrency control
    ///
    /// # Arguments
    /// * `key` - The key to store
    /// * `value` - The value to store
    /// * `expected_version` - Expected current version (0 = create new key)
    ///
    /// # Returns
    /// * `Ok(new_version)` - The new version after successful write
    /// * `Err(StorageError)` - Error if version mismatch, key exists/not found, etc.
    async fn put(
        &self,
        key: &str,
        value: String,
        expected_version: u64,
    ) -> Result<u64, StorageError>;
}
