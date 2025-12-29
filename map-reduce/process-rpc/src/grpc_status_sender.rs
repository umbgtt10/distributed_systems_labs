use crate::rpc::proto;
use async_trait::async_trait;
use map_reduce_core::status_sender::StatusSender;
use proto::synchronization_service_client::SynchronizationServiceClient;
use proto::{CompletionMessage, RegisterWorkerRequest};
use serde::{Deserialize, Serialize};
use tonic::transport::Channel;

/// gRPC Synchronization Token
/// Sent to workers to report completion back to coordinator
#[derive(Clone, Serialize, Deserialize, Default)]
pub struct GrpcStatusSender {
    pub server_addr: String,
    pub worker_id: usize,
}

#[async_trait]
impl StatusSender for GrpcStatusSender {
    async fn register(&self, _worker_id: usize) -> bool {
        let endpoint = format!("http://{}", self.server_addr);

        // Retry logic for connecting to coordinator
        for _ in 0..5 {
            if let Ok(channel) = Channel::from_shared(endpoint.clone())
                .unwrap()
                .connect()
                .await
            {
                let mut client = SynchronizationServiceClient::new(channel);
                let request = tonic::Request::new(RegisterWorkerRequest {
                    worker_id: self.worker_id as u64,
                });

                if client.register_worker(request).await.is_ok() {
                    return true;
                }
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }
        false
    }

    async fn send(&self, result: Result<usize, ()>) -> bool {
        let endpoint = format!("http://{}", self.server_addr);

        // Retry logic for connecting to coordinator
        for _ in 0..5 {
            if let Ok(channel) = Channel::from_shared(endpoint.clone())
                .unwrap()
                .connect()
                .await
            {
                let mut client = SynchronizationServiceClient::new(channel);
                let request = tonic::Request::new(CompletionMessage {
                    worker_id: self.worker_id as u64,
                    success: result.is_ok(),
                });

                if client.report_completion(request).await.is_ok() {
                    return true;
                }
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }
        false
    }
}
