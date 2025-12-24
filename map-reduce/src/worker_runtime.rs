use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

/// Trait for abstracting worker runtime (tasks, threads, processes)
pub trait WorkerRuntime: Send + 'static {
    type Handle: Send;
    type Error: std::fmt::Display + Send;

    /// Spawn a worker task/thread/process
    fn spawn<F, Fut>(f: F) -> Self::Handle
    where
        F: FnOnce() -> Fut + Send + 'static,
        Fut: std::future::Future<Output = ()> + Send + 'static;

    /// Wait for the worker to complete
    async fn join(handle: Self::Handle) -> Result<(), Self::Error>;
}

/// Tokio task-based runtime
pub struct TokioRuntime;

impl WorkerRuntime for TokioRuntime {
    type Handle = JoinHandle<()>;
    type Error = tokio::task::JoinError;

    fn spawn<F, Fut>(f: F) -> Self::Handle
    where
        F: FnOnce() -> Fut + Send + 'static,
        Fut: std::future::Future<Output = ()> + Send + 'static,
    {
        tokio::spawn(f())
    }

    async fn join(handle: Self::Handle) -> Result<(), Self::Error> {
        handle.await
    }
}

/// Trait for shutdown signaling
pub trait ShutdownSignal: Clone + Send + 'static {
    fn is_cancelled(&self) -> bool;
}

/// Tokio CancellationToken-based shutdown signal
#[derive(Clone)]
pub struct TokenShutdownSignal {
    token: CancellationToken,
}

impl TokenShutdownSignal {
    pub fn new(token: CancellationToken) -> Self {
        Self { token }
    }
}

impl ShutdownSignal for TokenShutdownSignal {
    fn is_cancelled(&self) -> bool {
        self.token.is_cancelled()
    }
}
