use std::fmt::Display;

/// Trait for workers (mappers and reducers) to abstract communication mechanism
pub trait Worker: Send {
    type Assignment: Send;
    type Completion;
    type Error: Display;

    /// Send a work assignment to this worker
    fn send_work(&self, assignment: Self::Assignment, complete_tx: Self::Completion);

    /// Wait for the worker to shut down
    async fn wait(self) -> Result<(), Self::Error>;
}
