// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::grpc_work_receiver::GrpcWorkReceiver;
use crate::rpc::proto;
use map_reduce_core::work_sender::WorkSender;
use proto::work_service_client::WorkServiceClient;
use proto::{InitializeWorkerRequest, WorkMessage};
use serde::{Deserialize, Serialize};
use std::marker::PhantomData;
use std::sync::Arc;
use tokio::sync::Mutex;
use tonic::transport::Channel;

#[derive(Clone)]
pub struct GrpcWorkSender<A, C> {
    worker_addr: String,
    _phantom: PhantomData<(A, C)>,
}

impl<A, C> GrpcWorkSender<A, C>
where
    A: Send + Sync + Serialize + for<'de> Deserialize<'de> + 'static,
    C: Send + Sync + Serialize + for<'de> Deserialize<'de> + 'static,
{
    /// Create a work channel pair.
    /// The server is NOT started here. It is started lazily by the receiver (in the worker process).
    pub async fn create_pair(port: u16) -> (Self, GrpcWorkReceiver<A, C>) {
        let addr_str = format!("127.0.0.1:{}", port);

        let channel = Self {
            worker_addr: addr_str,
            _phantom: PhantomData,
        };

        let receiver = GrpcWorkReceiver {
            port,
            rx: Arc::new(Mutex::new(None)),
        };

        (channel, receiver)
    }
}

impl<A, C> WorkSender<A, C> for GrpcWorkSender<A, C>
where
    A: Clone + Send + Serialize + 'static,
    C: Clone + Send + Serialize + 'static,
{
    fn initialize(&self, token: C) {
        let addr = self.worker_addr.clone();
        let synchronization_token_json = serde_json::to_string(&token).unwrap();

        tokio::spawn(async move {
            let endpoint = format!("http://{}", addr);
            let max_retries = 50; // Try for up to 5 seconds (100ms * 50)
            let retry_delay = std::time::Duration::from_millis(100);

            for attempt in 1..=max_retries {
                // Use connect_lazy to let Tonic handle connection establishment and buffering
                let channel = match Channel::from_shared(endpoint.clone()) {
                    Ok(c) => c.connect_lazy(),
                    Err(e) => {
                        eprintln!("Invalid URI {}: {}", endpoint, e);
                        return;
                    }
                };

                let mut client = WorkServiceClient::new(channel);
                let request = tonic::Request::new(InitializeWorkerRequest {
                    synchronization_token_json: synchronization_token_json.clone(),
                });

                match client.initialize_worker(request).await {
                    Ok(_) => {
                        // Success
                        return;
                    }
                    Err(e) => {
                        if attempt == max_retries {
                            eprintln!(
                                "Failed to initialize worker {} after {} attempts: {}",
                                addr, max_retries, e
                            );
                        } else {
                            tokio::time::sleep(retry_delay).await;
                        }
                    }
                }
            }
        });
    }

    fn send_work(&self, assignment: A, completion: C) {
        let addr = self.worker_addr.clone();
        let assignment_json = serde_json::to_string(&assignment).unwrap();
        let completion_json = serde_json::to_string(&completion).unwrap();

        tokio::spawn(async move {
            let endpoint = format!("http://{}", addr);

            // Use connect_lazy to let Tonic handle connection establishment and buffering
            let channel = match Channel::from_shared(endpoint.clone()) {
                Ok(c) => c.connect_lazy(),
                Err(e) => {
                    eprintln!("Invalid URI {}: {}", endpoint, e);
                    return;
                }
            };

            let mut client = WorkServiceClient::new(channel);
            let request = tonic::Request::new(WorkMessage {
                assignment_json,
                completion_json,
            });

            if let Err(e) = client.receive_work(request).await {
                eprintln!("Failed to send work to {}: {}", addr, e);
            }
        });
    }
}

