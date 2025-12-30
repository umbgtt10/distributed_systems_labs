// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use map_reduce_core::worker_runtime::{WorkerRuntime, WorkerTask};
use std::thread::{self, JoinHandle};

/// Thread-based runtime
#[derive(Clone, Copy)]
pub struct ThreadRuntime;

impl<T> WorkerRuntime<T> for ThreadRuntime
where
    T: WorkerTask<Output = ()> + Send + 'static,
{
    type Handle = JoinHandle<()>;
    type Error = Box<dyn std::error::Error + Send + Sync>;

    fn spawn(task: T) -> Self::Handle {
        // For thread-based runtime, we need to block on the future
        thread::spawn(move || {
            // Create a simple runtime for blocking on the future
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(task.run());
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

