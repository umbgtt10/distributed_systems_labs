// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use map_reduce_core::worker_runtime::{WorkerRuntime, WorkerTask};
use tokio::task::{self, JoinError, JoinHandle};

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

