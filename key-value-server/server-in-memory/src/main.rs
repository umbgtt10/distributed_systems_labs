mod key_value_service;

use crate::key_value_service::{rpc::proto::kv_service_server::KvServiceServer, KvServiceImpl};
use tonic::transport::Server;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = "127.0.0.1:50051".parse()?;
    let service = KvServiceImpl::new();

    println!("KV Server listening on {}", addr);
    println!("Press Ctrl+C to stop the server");

    Server::builder()
        .add_service(KvServiceServer::new(service))
        .serve_with_shutdown(addr, async {
            tokio::signal::ctrl_c()
                .await
                .expect("Failed to listen for Ctrl+C");
            println!("\nShutting down server...");
        })
        .await?;

    println!("Server stopped");
    Ok(())
}
