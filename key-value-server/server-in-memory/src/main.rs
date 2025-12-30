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
