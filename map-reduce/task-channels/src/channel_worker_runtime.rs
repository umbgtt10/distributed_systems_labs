use map_reduce_core::shutdown_signal::ShutdownSignal;
use map_reduce_core::worker_runtime::{WorkerRuntime, WorkerTask};
use tokio::task::{self, JoinError, JoinHandle};
use tokio_util::sync::CancellationToken;

/// Tokio task-based runtime
#[derive(Clone, Copy)]
pub struct TokioRuntime;

impl<T> WorkerRuntime<T> for TokioRuntime
where
    T: WorkerTask<Output = ()> + Send + 'static,
{
    type Handle = JoinHandle<()>;
    type Error = JoinError;

    fn spawn(task: T) -> Self::Handle {
        task::spawn(task.run())
    }

    async fn join(handle: Self::Handle) -> Result<(), Self::Error> {
        handle.await
    }
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
