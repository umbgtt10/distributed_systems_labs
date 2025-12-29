use std::fmt::Display;
use std::future::Future;

/// Trait for workers (mappers and reducers) to abstract communication mechanism
pub trait Worker: Send {
    type Assignment: Send;
    type Completion;
    type Error: Display;

    /// Initialize the worker with a synchronization token
    fn initialize(&self, token: Self::Completion);

    /// Send a work assignment to this worker
    fn send_work(&self, assignment: Self::Assignment, complete_tx: Self::Completion);

    /// Wait for the worker to shut down
    fn wait(self) -> impl Future<Output = Result<(), Self::Error>> + Send;
}
