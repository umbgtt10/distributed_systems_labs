// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::rpc::proto::{
    kv_service_server::KvService, GetRequest, GetResponse, PutRequest, PutResponse,
};
use crate::{KeyValueServer, Storage};
use tonic::{Request, Response, Status};

/// Wrapper that simulates packet loss by dropping responses after PUT operations
pub struct PacketLossWrapper<S: Storage> {
    inner: KeyValueServer<S>,
    loss_rate: f32,
}

impl<S: Storage> PacketLossWrapper<S> {
    pub fn new(inner: KeyValueServer<S>, loss_rate: f32) -> Self {
        Self { inner, loss_rate }
    }
}

#[tonic::async_trait]
impl<S: Storage + 'static> KvService for PacketLossWrapper<S> {
    async fn get(&self, request: Request<GetRequest>) -> Result<Response<GetResponse>, Status> {
        // GET operations pass through without simulation
        self.inner.get(request).await
    }

    async fn put(&self, request: Request<PutRequest>) -> Result<Response<PutResponse>, Status> {
        // Extract key for logging
        let key = request.get_ref().key.clone();

        // Execute the PUT operation
        let response = self.inner.put(request).await?;

        // Simulate packet loss AFTER the operation succeeded
        if fastrand::f32() < self.loss_rate {
            println!(
                "[SERVER] Simulating packet loss - dropping PUT response for key: {}",
                key
            );
            return Err(Status::deadline_exceeded("simulated packet loss"));
        }

        Ok(response)
    }
}

