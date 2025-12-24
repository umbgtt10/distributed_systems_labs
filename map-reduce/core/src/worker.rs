use std::fmt::Display;

/// Trait for workers (mappers and reducers) to abstract communication mechanism
pub trait Worker: Send {
    type Assignment: Send;
    type Completion;
    type Error: Display;

    /// Send a work assignment to this worker
    fn send_work(&self, assignment: Self::Assignment, complete_tx: Self::Completion);

    /// Wait for the worker to shut down
    fn wait(self) -> impl std::future::Future<Output = Result<(), Self::Error>> + Send;
}

/// Trait for creating workers
pub trait WorkerFactory<W>: Send {
    fn create_worker(&mut self, id: usize) -> W;
}

impl<F, W> WorkerFactory<W> for F
where
    F: FnMut(usize) -> W + Send,
{
    fn create_worker(&mut self, id: usize) -> W {
        (self)(id)
    }
}
