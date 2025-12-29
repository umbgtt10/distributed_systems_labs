use async_trait::async_trait;

/// Trait for sending synchronization signals (readiness and completion) asynchronously
#[async_trait]
pub trait StatusSender: Send + Clone + Sync {
    /// Register the worker as ready to receive work
    /// Returns true if the signal was sent successfully
    async fn register(&self, worker_id: usize) -> bool;

    /// Send a completion signal (success or failure)
    /// Returns true if the signal was sent successfully, false otherwise
    async fn send(&self, result: Result<usize, ()>) -> bool;
}
