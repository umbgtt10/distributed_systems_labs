use map_reduce_core::shutdown_signal::ShutdownSignal;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

/// Thread-based shutdown signal using atomic flag
#[derive(Clone)]
pub struct SocketShutdownSignal {
    flag: Arc<AtomicBool>,
}

impl SocketShutdownSignal {
    pub fn new() -> Self {
        Self {
            flag: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn shutdown(&self) {
        self.flag.store(true, Ordering::SeqCst);
    }
}

impl ShutdownSignal for SocketShutdownSignal {
    fn is_cancelled(&self) -> bool {
        self.flag.load(Ordering::SeqCst)
    }
}
