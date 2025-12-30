mod grpc_client;

use crate::grpc_client::GrpcClient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = GrpcClient::new("http://127.0.0.1:50051".to_string());
    let cancellation_token = client.cancellation_token();

    // Spawn the client task
    let client_handle = tokio::spawn(async move {
        if let Err(e) = client.start().await {
            eprintln!("Client error: {}", e);
        }
    });

    // Wait for Ctrl+C
    tokio::signal::ctrl_c().await?;
    println!("\nReceived Ctrl+C, shutting down...");

    // Trigger cancellation
    cancellation_token.cancel();

    // Wait for client to finish
    client_handle.await?;

    Ok(())
}
