// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

mod flat_file_storage;

use crate::flat_file_storage::FlatFileStorage;
use key_value_server_core::{Config, ServerRunner};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let storage = FlatFileStorage::new("storage.txt".to_string()).await;
    let config = Config::load("config.json").expect("Failed to load config.json");

    ServerRunner::new(storage, &config, "127.0.0.1:50051")?
        .run()
        .await
}
