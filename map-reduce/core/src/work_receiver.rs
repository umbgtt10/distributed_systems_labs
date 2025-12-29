use crate::worker_message::WorkerMessage;
use async_trait::async_trait;

/// Trait for receiving work assignments asynchronously
#[async_trait]
pub trait WorkReceiver<A, C>: Send {
    /// Receive the next message (initialization or work)
    /// Returns None if the channel is closed
    async fn recv(&mut self) -> Option<WorkerMessage<A, C>>;
}
