use crate::worker::Worker;

/// Trait for distributing work assignments to workers
/// This abstracts the entire work distribution pattern:
/// - Setting up completion signaling
/// - Initial work assignment
/// - Dynamic reassignment as workers complete
/// - Worker shutdown
pub trait WorkDistributor: Send {
    /// The type of worker this distributor manages
    type Worker: Worker;

    /// Distribute assignments to workers
    /// This method handles the complete lifecycle:
    /// - Assigns initial work
    /// - Waits for completions and reassigns dynamically
    /// - Waits for all workers to finish
    async fn distribute(
        &mut self,
        workers: Vec<Self::Worker>,
        assignments: Vec<<Self::Worker as Worker>::Assignment>,
    ) where
        <Self::Worker as Worker>::Assignment: Clone;
}
