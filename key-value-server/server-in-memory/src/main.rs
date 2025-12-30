// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

mod in_memory_storage;

use crate::in_memory_storage::InMemoryStorage;
use key_value_server_core::ServerRunner;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let storage = InMemoryStorage::new();

    ServerRunner::new(storage, "config.json", "127.0.0.1:50051")?
        .run()
        .await
}

