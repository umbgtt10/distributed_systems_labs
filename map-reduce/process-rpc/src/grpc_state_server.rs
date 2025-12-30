// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use map_reduce_core::state_store::StateStore;
use std::sync::Arc;
use tonic::transport::Server;
use tonic::{Request, Response, Status};

use crate::rpc::proto;
use proto::state_service_server::{StateService, StateServiceServer};
use proto::{
    GetRequest, GetResponse, InitializeRequest, ReplaceRequest, StateResponse, UpdateRequest,
};

/// gRPC State Server that wraps any StateAccess implementation
pub struct GrpcStateServer<S> {
    state: Arc<S>,
}

impl<S> GrpcStateServer<S> {
    pub fn new(state: S) -> Self {
        Self {
            state: Arc::new(state),
        }
    }
}

#[tonic::async_trait]
impl<S: StateStore + Send + Sync + 'static> StateService for GrpcStateServer<S> {
    async fn initialize(
        &self,
        request: Request<InitializeRequest>,
    ) -> Result<Response<StateResponse>, Status> {
        let keys = request.into_inner().keys;
        self.state.initialize(keys).await;
        Ok(Response::new(StateResponse {
            success: true,
            error: String::new(),
        }))
    }

    async fn update(
        &self,
        request: Request<UpdateRequest>,
    ) -> Result<Response<StateResponse>, Status> {
        let req = request.into_inner();
        self.state.update(req.key, req.value).await;
        Ok(Response::new(StateResponse {
            success: true,
            error: String::new(),
        }))
    }

    async fn replace(
        &self,
        request: Request<ReplaceRequest>,
    ) -> Result<Response<StateResponse>, Status> {
        let req = request.into_inner();
        self.state.replace(req.key, req.value).await;
        Ok(Response::new(StateResponse {
            success: true,
            error: String::new(),
        }))
    }

    async fn get(&self, request: Request<GetRequest>) -> Result<Response<GetResponse>, Status> {
        let key = request.into_inner().key;
        let values = self.state.get(&key).await;
        Ok(Response::new(GetResponse { values }))
    }
}

/// Manages the gRPC state server lifecycle
pub struct StateServerHandle {
    _shutdown_tx: tokio::sync::oneshot::Sender<()>,
}

/// Start the gRPC state server on a specific port
pub async fn start_state_server<S: StateStore + Send + Sync + 'static>(
    state: S,
    port: u16,
) -> Result<StateServerHandle, Box<dyn std::error::Error>> {
    let addr: std::net::SocketAddr = format!("0.0.0.0:{}", port).parse()?;
    let server = GrpcStateServer::new(state);

    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();

    // Bind first to ensure port is available
    let listener = tokio::net::TcpListener::bind(addr).await?;
    let incoming = tokio_stream::wrappers::TcpListenerStream::new(listener);

    tokio::spawn(async move {
        Server::builder()
            .add_service(StateServiceServer::new(server))
            .serve_with_incoming_shutdown(incoming, async {
                shutdown_rx.await.ok();
            })
            .await
            .expect("gRPC state server failed");
    });

    Ok(StateServerHandle {
        _shutdown_tx: shutdown_tx,
    })
}

