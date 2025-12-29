use crate::rpc::proto;
use async_trait::async_trait;
use map_reduce_core::status_sender::StatusSender;
use map_reduce_core::worker_synchronization::WorkerSynchronization;
use proto::synchronization_service_client::SynchronizationServiceClient;
use proto::synchronization_service_server::{
    SynchronizationService as SynchronizationServiceTrait, SynchronizationServiceServer,
};
use proto::{CompletionAck, CompletionMessage, RegisterWorkerRequest, RegisterWorkerResponse};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Notify;
use tonic::transport::{Channel, Server};
use tonic::{Request, Response, Status};

/// gRPC Synchronization Token
/// Sent to workers to report completion back to coordinator
#[derive(Clone, Serialize, Deserialize, Default)]
pub struct GrpcSynchronizationToken {
    server_addr: String,
    worker_id: usize,
}

#[async_trait]
impl StatusSender for GrpcSynchronizationToken {
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

/// gRPC Synchronization Service implementation
struct SynchronizationServiceImpl {
    completion_tx: tokio::sync::mpsc::Sender<(usize, bool)>,
    readiness_notifiers: Arc<Vec<Arc<Notify>>>,
}

#[tonic::async_trait]
impl SynchronizationServiceTrait for SynchronizationServiceImpl {
    async fn register_worker(
        &self,
        request: Request<RegisterWorkerRequest>,
    ) -> Result<Response<RegisterWorkerResponse>, Status> {
        let msg = request.into_inner();
        let worker_id = msg.worker_id as usize;

        if let Some(notify) = self.readiness_notifiers.get(worker_id) {
            notify.notify_one();
        } else {
            eprintln!("Received registration for unknown worker {}", worker_id);
        }

        Ok(Response::new(RegisterWorkerResponse {
            success: true,
            error: String::new(),
        }))
    }

    async fn report_completion(
        &self,
        request: Request<CompletionMessage>,
    ) -> Result<Response<CompletionAck>, Status> {
        let msg = request.into_inner();

        self.completion_tx
            .send((msg.worker_id as usize, msg.success))
            .await
            .map_err(|_| Status::internal("Failed to queue completion"))?;

        Ok(Response::new(CompletionAck { received: true }))
    }
}

/// gRPC Synchronization Signaling
/// Coordinator receives completion notifications from workers
pub struct GrpcSynchronizationSignaling {
    completion_rx: tokio::sync::mpsc::Receiver<(usize, bool)>,
    readiness_notifiers: Arc<Vec<Arc<Notify>>>,
    server_addr: String,
}

impl WorkerSynchronization for GrpcSynchronizationSignaling {
    type Token = GrpcSynchronizationToken;

    fn setup(num_workers: usize) -> Self {
        let (tx, rx) = tokio::sync::mpsc::channel(100);
        let (port_tx, port_rx) = std::sync::mpsc::channel();

        let mut notifiers = Vec::with_capacity(num_workers);
        for _ in 0..num_workers {
            notifiers.push(Arc::new(Notify::new()));
        }
        let notifiers = Arc::new(notifiers);
        let service_notifiers = notifiers.clone();

        tokio::spawn(async move {
            // Bind to a random available port
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
                .await
                .expect("Failed to bind synchronization listener");

            let addr = listener.local_addr().expect("No local address");
            port_tx.send(addr.port()).expect("Failed to send port");

            let service = SynchronizationServiceImpl {
                completion_tx: tx,
                readiness_notifiers: service_notifiers,
            };

            // Use the listener directly instead of binding again
            let incoming = tokio_stream::wrappers::TcpListenerStream::new(listener);

            if let Err(e) = Server::builder()
                .add_service(SynchronizationServiceServer::new(service))
                .serve_with_incoming(incoming)
                .await
            {
                eprintln!("Synchronization service error: {}", e);
            }
        });

        let port = port_rx.recv().expect("Failed to receive port");
        let server_addr = format!("127.0.0.1:{}", port);

        Self {
            completion_rx: rx,
            readiness_notifiers: notifiers,
            server_addr,
        }
    }

    fn get_token(&self, worker_id: usize) -> Self::Token {
        GrpcSynchronizationToken {
            server_addr: self.server_addr.clone(),
            worker_id,
        }
    }

    async fn wait_for_worker_ready(&self, worker_id: usize) -> bool {
        if let Some(notify) = self.readiness_notifiers.get(worker_id) {
            notify.notified().await;
            true
        } else {
            false
        }
    }

    async fn wait_next(&mut self) -> Option<Result<usize, usize>> {
        self.completion_rx.recv().await.map(|(worker_id, success)| {
            if success {
                Ok(worker_id)
            } else {
                Err(worker_id)
            }
        })
    }

    async fn reset_worker(&mut self, worker_id: usize) -> Self::Token {
        // No explicit reset needed for Notify as it consumes the permit on wait
        self.get_token(worker_id)
    }
}
