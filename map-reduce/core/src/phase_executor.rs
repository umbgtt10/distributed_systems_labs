use crate::shutdown_signal::ShutdownSignal;
use crate::worker::Worker;
use async_trait::async_trait;

/// Trait for executing a phase (map or reduce) with fault tolerance
/// This abstracts the entire work distribution pattern:
/// - Setting up completion signaling
/// - Initial work assignment
/// - Dynamic reassignment as workers complete
/// - Worker shutdown
#[async_trait]
pub trait PhaseExecutor: Send {
    /// The type of worker this executor manages
    type Worker: Worker;

    /// Execute a phase by distributing assignments to workers
    /// This method handles the complete lifecycle:
    /// - Assigns initial work
    /// - Waits for completions and reassigns dynamically
    /// - Waits for all workers to finish
    async fn execute<SD>(
        &mut self,
        workers: Vec<Self::Worker>,
        assignments: Vec<<Self::Worker as Worker>::Assignment>,
        shutdown_signal: &SD,
    ) -> Vec<Self::Worker>
    where
        SD: ShutdownSignal + Sync,
        <Self::Worker as Worker>::Assignment: Clone;
}
