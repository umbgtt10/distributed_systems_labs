// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::socket_worker_synchronization::CompletionMessage;
use async_trait::async_trait;
use map_reduce_core::status_sender::StatusSender;
use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt;

/// Completion sender
#[derive(Clone, Serialize, Deserialize)]
pub struct SocketStatusSender {
    pub port: u16,
    pub worker_id: usize,
}

#[async_trait]
impl StatusSender for SocketStatusSender {
    async fn register(&self, _worker_id: usize) -> bool {
        true
    }

    async fn send(&self, result: Result<usize, ()>) -> bool {
        let addr = format!("127.0.0.1:{}", self.port);
        let message = match result {
            Ok(id) => CompletionMessage::Success(id),
            Err(_) => CompletionMessage::Failure(self.worker_id),
        };
        if let Ok(mut stream) = tokio::net::TcpStream::connect(&addr).await {
            if let Ok(serialized) = serde_json::to_vec(&message) {
                let len = serialized.len() as u32;
                if stream.write_all(&len.to_be_bytes()).await.is_ok()
                    && stream.write_all(&serialized).await.is_ok()
                {
                    return true;
                }
            }
        }
        false
    }
}

