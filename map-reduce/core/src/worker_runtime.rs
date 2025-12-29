use async_trait::async_trait;
use std::fmt::Display;
use std::future::Future;

/// Defines a unit of work that can be executed
#[async_trait]
pub trait WorkerTask: Send + 'static {
    type Output;
    async fn run(self) -> Self::Output;
}

/// Trait for abstracting worker runtime (tasks, threads, processes)
pub trait WorkerRuntime<Task>: Send + 'static {
    type Handle: Send;
    type Error: Display + Send;

    /// Spawn a worker task/thread/process
    fn spawn(task: Task) -> Self::Handle;

    /// Wait for the worker to complete
    fn join(handle: Self::Handle) -> impl Future<Output = Result<(), Self::Error>> + Send;
}
