// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use async_trait::async_trait;
use map_reduce_core::state_store::StateStore;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;
use tonic::transport::Channel;

use crate::rpc::proto;
use proto::state_service_client::StateServiceClient;
use proto::{GetRequest, InitializeRequest, ReplaceRequest, UpdateRequest};

/// gRPC client for StateAccess
/// Native async implementation - no blocking required!
#[derive(Clone, Serialize, Deserialize)]
pub struct GrpcStateStore {
    server_addr: String,
    #[serde(skip)]
    client: Arc<Mutex<Option<StateServiceClient<Channel>>>>,
}

impl GrpcStateStore {
    pub fn new(server_addr: String) -> Self {
        Self {
            server_addr,
            client: Arc::new(Mutex::new(None)),
        }
    }

    async fn get_client(&self) -> Result<StateServiceClient<Channel>, tonic::transport::Error> {
        let mut client_guard = self.client.lock().await;

        if client_guard.is_none() {
            let endpoint = format!("http://{}", self.server_addr);
            let channel = Channel::from_shared(endpoint).unwrap().connect().await?;
            *client_guard = Some(StateServiceClient::new(channel));
        }

        Ok(client_guard.as_ref().unwrap().clone())
    }
}

#[async_trait]
impl StateStore for GrpcStateStore {
    async fn initialize(&self, keys: Vec<String>) {
        if let Ok(mut client) = self.get_client().await {
            let request = tonic::Request::new(InitializeRequest { keys });
            let _ = client.initialize(request).await;
        }
    }

    async fn update(&self, key: String, value: i32) {
        if let Ok(mut client) = self.get_client().await {
            let request = tonic::Request::new(UpdateRequest { key, value });
            if let Err(e) = client.update(request).await {
                eprintln!("State update error: {}", e);
            }
        }
    }

    async fn replace(&self, key: String, value: i32) {
        if let Ok(mut client) = self.get_client().await {
            let request = tonic::Request::new(ReplaceRequest { key, value });
            if let Err(e) = client.replace(request).await {
                eprintln!("State replace error: {}", e);
            }
        }
    }

    async fn get(&self, key: &str) -> Vec<i32> {
        if let Ok(mut client) = self.get_client().await {
            let request = tonic::Request::new(GetRequest {
                key: key.to_string(),
            });
            if let Ok(response) = client.get(request).await {
                return response.into_inner().values;
            }
        }
        Vec::new()
    }
}

