mod in_memory_storage;

use crate::in_memory_storage::InMemoryStorage;
use key_value_server_core::{
    rpc::proto::kv_service_server::KvServiceServer, GrpcClient, KeyValueServer, Storage, TestConfig,
};
use tonic::transport::Server;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load test configuration - use path relative to workspace root
    let workspace_root = std::env::current_dir()?;
    let config_path = workspace_root.join("core/config.json");
    let test_config = TestConfig::from_file(config_path.to_str().unwrap())?;

    println!(
        "Loaded config: {} clients, {} second test duration",
        test_config.clients.len(),
        test_config.test_duration_seconds
    );

    let addr = "127.0.0.1:50051".parse()?;
    let storage = InMemoryStorage::new();
    let storage_clone = storage.clone();
    let service = KeyValueServer::new(storage);

    println!("KV Server listening on {}", addr);
    println!("Press Ctrl+C to stop the server\n");

    // Spawn all clients from config
    let mut client_handles = Vec::new();
    let mut client_cancellations = Vec::new();

    for client_config in test_config.clients.clone() {
        let client = GrpcClient::new(client_config, "http://127.0.0.1:50051".to_string());
        let cancellation = client.cancellation_token();
        client_cancellations.push(cancellation);

        let client_handle = tokio::spawn(async move {
            // Give the server time to start
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            if let Err(e) = client.start().await {
                eprintln!("Client error: {}", e);
            }
        });
        client_handles.push(client_handle);
    }

    // Spawn auto-shutdown timer
    let test_duration = test_config.test_duration_seconds;
    let auto_shutdown_cancellations = client_cancellations.clone();
    let shutdown_tx = tokio::sync::oneshot::channel();
    let (auto_shutdown_sender, auto_shutdown_receiver) = shutdown_tx;

    let auto_shutdown = tokio::spawn(async move {
        tokio::time::sleep(tokio::time::Duration::from_secs(test_duration + 1)).await;
        println!(
            "\n{} seconds elapsed, initiating shutdown...",
            test_duration
        );
        for cancellation in auto_shutdown_cancellations {
            cancellation.cancel();
        }
        let _ = auto_shutdown_sender.send(());
    });

    // Run server with shutdown handling (either Ctrl+C or auto-shutdown)
    let shutdown_signal = async {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                println!("\nReceived Ctrl+C, shutting down...");
            }
            _ = auto_shutdown_receiver => {
                println!("Auto-shutdown triggered");
            }
        }
    };

    let server_future = Server::builder()
        .add_service(KvServiceServer::new(service))
        .serve_with_shutdown(addr, shutdown_signal);

    // Wait for server to finish
    let _ = server_future.await;

    // Cancel all clients (in case Ctrl+C was pressed before timer)
    auto_shutdown.abort();
    for cancellation in client_cancellations {
        cancellation.cancel();
    }

    // Wait for all clients to finish
    for handle in client_handles {
        let _ = handle.await;
    }

    // Print final storage state
    storage_clone.print_all().await;

    println!("Server stopped");
    Ok(())
}
