// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use async_trait::async_trait;

/// Trait for accessing shared state across workers
/// Abstracts the storage mechanism (local, Redis, RPC, etc.)
///
/// **Design Decision**: This trait is async to favor network-based implementations
/// (gRPC, Redis, etc.) where I/O naturally benefits from async/await. Local
/// implementations pay a small cost of wrapping sync operations in async blocks.
#[async_trait]
pub trait StateStore: Clone + Send + Sync + 'static {
    /// Initialize keys with empty vectors
    async fn initialize(&self, keys: Vec<String>);

    /// Update a key with a value (append for mappers)
    async fn update(&self, key: String, value: i32);

    /// Replace the entire value for a key (used by reducers)
    async fn replace(&self, key: String, value: i32);

    /// Get all values for a key
    async fn get(&self, key: &str) -> Vec<i32>;
}

