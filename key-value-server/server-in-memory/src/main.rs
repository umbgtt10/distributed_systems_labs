mod in_memory_storage;

use crate::in_memory_storage::InMemoryStorage;
use key_value_server_core::{
    rpc::proto::kv_service_server::KvServiceServer, GrpcClient, KeyValueServer, Storage,
};
use tonic::transport::Server;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = "127.0.0.1:50051".parse()?;
    let storage = InMemoryStorage::new();
    let storage_clone = storage.clone();
    let service = KeyValueServer::new(storage);

    println!("KV Server listening on {}", addr);
    println!("Press Ctrl+C to stop the server");

    // Spawn a client for testing
    let client = GrpcClient::new("client_1".to_string(), "http://127.0.0.1:50051".to_string());
    let client_cancellation = client.cancellation_token();

    let client_handle = tokio::spawn(async move {
        // Give the server time to start
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        if let Err(e) = client.start().await {
            eprintln!("Client error: {}", e);
        }
    });

    // Run server with shutdown handling
    let server_handle = tokio::spawn(async move {
        Server::builder()
            .add_service(KvServiceServer::new(service))
            .serve_with_shutdown(addr, async {
                tokio::signal::ctrl_c()
                    .await
                    .expect("Failed to listen for Ctrl+C");
                println!("\nShutting down server...");
            })
            .await
    });

    // Wait for Ctrl+C
    tokio::signal::ctrl_c().await?;

    // Cancel client
    client_cancellation.cancel();

    // Wait for both to finish
    let _ = tokio::join!(client_handle, server_handle);

    // Print final storage state
    storage_clone.print_all().await;

    println!("Server stopped");
    Ok(())
}
