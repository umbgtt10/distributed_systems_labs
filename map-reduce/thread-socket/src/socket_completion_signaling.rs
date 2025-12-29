use async_trait::async_trait;
use map_reduce_core::completion_signaling::SynchronizationSignaling;
use map_reduce_core::worker_io::SynchronizationSender as SynchronizationSenderTrait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio_stream::wrappers::TcpListenerStream;
use tokio_stream::{StreamExt, StreamMap};

/// Completion message type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CompletionMessage {
    Success(usize),
    Failure(usize),
}

/// Socket-based completion signaling
pub struct SocketCompletionSignaling {
    listeners: StreamMap<usize, TcpListenerStream>,
    ports: HashMap<usize, u16>,
}

impl SocketCompletionSignaling {
    pub fn new(num_workers: usize) -> Self {
        let mut listeners = StreamMap::new();
        let mut ports = HashMap::new();

        for i in 0..num_workers {
            // Use port 0 to let OS assign an available port
            let std_listener = std::net::TcpListener::bind("127.0.0.1:0")
                .expect("Failed to bind completion listener");
            std_listener
                .set_nonblocking(true)
                .expect("Failed to set nonblocking");
            let actual_port = std_listener
                .local_addr()
                .expect("Failed to get local address")
                .port();

            let tokio_listener =
                TcpListener::from_std(std_listener).expect("Failed to convert to tokio listener");
            let stream = TcpListenerStream::new(tokio_listener);

            listeners.insert(i, stream);
            ports.insert(i, actual_port);
        }

        Self { listeners, ports }
    }

    pub fn get_sender(&self, worker_id: usize) -> SocketCompletionToken {
        let port = self
            .ports
            .get(&worker_id)
            .copied()
            .expect("Invalid worker_id");
        SocketCompletionToken { port, worker_id }
    }
}

impl SynchronizationSignaling for SocketCompletionSignaling {
    type Token = SocketCompletionToken;

    fn setup(num_workers: usize) -> Self {
        Self::new(num_workers)
    }

    fn get_token(&self, worker_id: usize) -> Self::Token {
        self.get_sender(worker_id)
    }

    async fn wait_for_worker_ready(&self, _worker_id: usize) -> bool {
        true
    }

    async fn reset_worker(&mut self, worker_id: usize) -> Self::Token {
        // Remove old listener (closes socket)
        // This implicitly drains any pending connections because the listener is dropped
        self.listeners.remove(&worker_id);

        // Create new listener
        let std_listener =
            std::net::TcpListener::bind("127.0.0.1:0").expect("Failed to bind completion listener");
        std_listener
            .set_nonblocking(true)
            .expect("Failed to set nonblocking");
        let actual_port = std_listener
            .local_addr()
            .expect("Failed to get local address")
            .port();

        let tokio_listener =
            TcpListener::from_std(std_listener).expect("Failed to convert to tokio listener");
        let stream = TcpListenerStream::new(tokio_listener);

        // Insert new listener and update port
        self.listeners.insert(worker_id, stream);
        self.ports.insert(worker_id, actual_port);

        self.get_sender(worker_id)
    }

    async fn wait_next(&mut self) -> Option<Result<usize, usize>> {
        while let Some((_worker_id, connection_result)) = self.listeners.next().await {
            match connection_result {
                Ok(mut stream) => {
                    let mut len_bytes = [0u8; 4];
                    if stream.read_exact(&mut len_bytes).await.is_ok() {
                        let len = u32::from_be_bytes(len_bytes) as usize;
                        let mut buffer = vec![0u8; len];
                        if stream.read_exact(&mut buffer).await.is_ok() {
                            if let Ok(msg) = serde_json::from_slice::<CompletionMessage>(&buffer) {
                                return Some(match msg {
                                    CompletionMessage::Success(id) => Ok(id),
                                    CompletionMessage::Failure(id) => Err(id),
                                });
                            }
                        }
                    }
                }
                Err(_) => {
                    // Error occurred accepting connection
                }
            }
        }
        None
    }
}

/// Completion sender
#[derive(Clone, Serialize, Deserialize)]
pub struct SocketCompletionToken {
    port: u16,
    worker_id: usize,
}

#[async_trait]
impl SynchronizationSenderTrait for SocketCompletionToken {
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
