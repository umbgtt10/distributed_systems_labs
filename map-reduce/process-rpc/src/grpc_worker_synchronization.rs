// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::grpc_status_sender::GrpcStatusSender;
use crate::rpc::proto;
use map_reduce_core::worker_synchronization::WorkerSynchronization;
use proto::synchronization_service_server::{SynchronizationService, SynchronizationServiceServer};
use proto::{CompletionAck, CompletionMessage, RegisterWorkerRequest, RegisterWorkerResponse};
use std::sync::Arc;
use tokio::sync::Notify;
use tonic::transport::Server;
use tonic::{Request, Response, Status};

/// gRPC Synchronization Service implementation
struct SynchronizationServiceImpl {
    completion_tx: tokio::sync::mpsc::Sender<(usize, bool)>,
    readiness_notifiers: Arc<Vec<Arc<Notify>>>,
}

#[tonic::async_trait]
impl SynchronizationService for SynchronizationServiceImpl {
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
pub struct GrpcWorkerSynchronization {
    completion_rx: tokio::sync::mpsc::Receiver<(usize, bool)>,
    readiness_notifiers: Arc<Vec<Arc<Notify>>>,
    server_addr: String,
}

impl WorkerSynchronization for GrpcWorkerSynchronization {
    type StatusSender = GrpcStatusSender;

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

    fn get_status_sender(&self, worker_id: usize) -> Self::StatusSender {
        GrpcStatusSender {
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

    async fn reset_worker(&mut self, worker_id: usize) -> Self::StatusSender {
        // No explicit reset needed for Notify as it consumes the permit on wait
        self.get_status_sender(worker_id)
    }
}

