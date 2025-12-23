/// Trait for abstracting completion signaling mechanisms
/// This allows different implementations for tasks, threads, and processes
pub trait CompletionSignaling: Send {
    /// The token type passed to workers for signaling completion
    type Token: Clone + Send;

    /// Setup completion signaling for N workers
    fn setup(num_workers: usize) -> Self;

    /// Get the completion token for a specific worker
    fn get_token(&self, worker_id: usize) -> Self::Token;

    /// Wait for the next worker to complete
    /// Returns the worker_id that completed, or None if all are done
    async fn wait_next(&mut self) -> Option<usize>;
}
