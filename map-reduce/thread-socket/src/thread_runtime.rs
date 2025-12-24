use map_reduce_core::shutdown_signal::ShutdownSignal;
use map_reduce_core::worker_runtime::WorkerRuntime;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};

/// Thread-based runtime
pub struct ThreadRuntime;

impl WorkerRuntime for ThreadRuntime {
    type Handle = JoinHandle<()>;
    type Error = Box<dyn std::error::Error + Send + Sync>;

    fn spawn<F, Fut>(f: F) -> Self::Handle
    where
        F: FnOnce() -> Fut + Send + 'static,
        Fut: std::future::Future<Output = ()> + Send + 'static,
    {
        // For thread-based runtime, we need to block on the future
        thread::spawn(move || {
            // Create a simple runtime for blocking on the future
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(f());
        })
    }

    async fn join(handle: Self::Handle) -> Result<(), Self::Error> {
        tokio::task::spawn_blocking(move || {
            handle
                .join()
                .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> {
                    format!("Thread join error: {:?}", e).into()
                })
        })
        .await
        .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> {
            format!("Tokio join error: {}", e).into()
        })?
    }
}

/// Thread-based shutdown signal using atomic flag
#[derive(Clone)]
pub struct AtomicShutdownSignal {
    flag: Arc<AtomicBool>,
}

impl AtomicShutdownSignal {
    pub fn new() -> Self {
        Self {
            flag: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn shutdown(&self) {
        self.flag.store(true, Ordering::SeqCst);
    }
}

impl ShutdownSignal for AtomicShutdownSignal {
    fn is_cancelled(&self) -> bool {
        self.flag.load(Ordering::SeqCst)
    }
}
