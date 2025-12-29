use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Message types received by workers
#[derive(Serialize, Deserialize, Debug)]
pub enum WorkerMessage<A, C> {
    /// Initialization message containing the synchronization token
    Initialize(C),
    /// Work assignment
    Work(A, C),
}

/// Trait for receiving work assignments asynchronously
#[async_trait]
pub trait WorkReceiver<A, C>: Send {
    /// Receive the next message (initialization or work)
    /// Returns None if the channel is closed
    async fn recv(&mut self) -> Option<WorkerMessage<A, C>>;
}

/// Trait for sending synchronization signals (readiness and completion) asynchronously
#[async_trait]
pub trait SynchronizationSender: Send + Clone + Sync {
    /// Register the worker as ready to receive work
    /// Returns true if the signal was sent successfully
    async fn register(&self, worker_id: usize) -> bool;

    /// Send a completion signal (success or failure)
    /// Returns true if the signal was sent successfully, false otherwise
    async fn send(&self, result: Result<usize, ()>) -> bool;
}
