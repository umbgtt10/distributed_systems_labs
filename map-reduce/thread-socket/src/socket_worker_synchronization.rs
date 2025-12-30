use crate::socket_status_sender::SocketStatusSender;
use map_reduce_core::worker_synchronization::WorkerSynchronization;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::io::AsyncReadExt;
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
pub struct SocketWorkerSynchronization {
    listeners: StreamMap<usize, TcpListenerStream>,
    ports: HashMap<usize, u16>,
}

impl SocketWorkerSynchronization {
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

    pub fn get_sender(&self, worker_id: usize) -> SocketStatusSender {
        let port = self
            .ports
            .get(&worker_id)
            .copied()
            .expect("Invalid worker_id");
        SocketStatusSender { port, worker_id }
    }
}

impl WorkerSynchronization for SocketWorkerSynchronization {
    type StatusSender = SocketStatusSender;

    fn setup(num_workers: usize) -> Self {
        Self::new(num_workers)
    }

    fn get_status_sender(&self, worker_id: usize) -> Self::StatusSender {
        self.get_sender(worker_id)
    }

    async fn wait_for_worker_ready(&self, _worker_id: usize) -> bool {
        true
    }

    async fn reset_worker(&mut self, worker_id: usize) -> Self::StatusSender {
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
