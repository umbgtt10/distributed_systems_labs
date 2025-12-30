use crate::rpc::proto::kv_service_server::KvServiceServer;
use crate::{GrpcClient, KeyValueServer, PacketLossWrapper, Storage, TestConfig};
use std::net::SocketAddr;
use tonic::transport::Server;

/// Generic server runner that handles all the boilerplate for running a KV server
/// with multiple clients, packet loss simulation, and graceful shutdown.
pub struct ServerRunner<S: Storage> {
    storage: S,
    config: TestConfig,
    addr: SocketAddr,
}

impl<S: Storage + Clone + 'static> ServerRunner<S> {
    /// Create a new server runner
    ///
    /// # Arguments
    /// * `storage` - The storage implementation to use (must be Clone for accessing after shutdown)
    /// * `config_path` - Path to the test configuration JSON file
    /// * `addr` - Server address (e.g., "127.0.0.1:50051")
    pub fn new(
        storage: S,
        config_path: &str,
        addr: &str,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let config = TestConfig::from_file(config_path)?;
        let addr = addr.parse()?;

        Ok(Self {
            storage,
            config,
            addr,
        })
    }

    /// Run the server with all configured clients until shutdown
    pub async fn run(self) -> Result<(), Box<dyn std::error::Error>> {
        println!(
            "Loaded config: {} clients, {} second test duration, {:.1}% packet loss",
            self.config.clients.len(),
            self.config.test_duration_seconds,
            self.config.server_packet_loss_rate
        );

        let storage_clone = self.storage.clone();
        let base_service = KeyValueServer::new(self.storage);

        // Wrap with packet loss simulation (convert percentage to rate)
        let service =
            PacketLossWrapper::new(base_service, self.config.server_packet_loss_rate / 100.0);

        println!("KV Server listening on {}", self.addr);
        println!("Press Ctrl+C to stop the server\n");

        // Spawn all clients from config
        let mut client_handles = Vec::new();
        let mut client_cancellations = Vec::new();

        for client_config in self.config.clients.clone() {
            let client = GrpcClient::new(
                client_config,
                format!("http://{}", self.addr),
                self.config.max_retries_server_packet_loss,
            );
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
        let test_duration = self.config.test_duration_seconds;
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
            .serve_with_shutdown(self.addr, shutdown_signal);

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
}
