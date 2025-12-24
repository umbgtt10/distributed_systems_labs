/// Trait for abstracting completion signaling mechanisms
/// This allows different implementations for tasks, threads, and processes
pub trait CompletionSignaling: Send {
    /// The token type passed to workers for signaling completion
    type Token: Clone + Send;

    /// Setup completion signaling for N workers
    fn setup(num_workers: usize) -> Self;

    /// Get the completion token for a specific worker
    fn get_token(&self, worker_id: usize) -> Self::Token;

    /// Wait for the next worker to complete or fail
    /// Returns Ok(worker_id) on success, Err(worker_id) on failure
    /// Returns None if all workers are done
    fn wait_next(
        &mut self,
    ) -> impl std::future::Future<Output = Option<Result<usize, usize>>> + Send;

    /// Reset the signaling mechanism for a specific worker
    /// This drains any pending messages and returns a new token for the new worker
    fn reset_worker(
        &mut self,
        worker_id: usize,
    ) -> impl std::future::Future<Output = Self::Token> + Send;
}
