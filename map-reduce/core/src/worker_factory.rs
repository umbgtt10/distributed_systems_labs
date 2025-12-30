// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

/// Trait for creating workers
use async_trait::async_trait;

#[async_trait]
pub trait WorkerFactory<W>: Send {
    async fn create_worker(&mut self, id: usize) -> W;
}

#[async_trait]
impl<F, W> WorkerFactory<W> for F
where
    F: FnMut(usize) -> W + Send,
    W: Send,
{
    async fn create_worker(&mut self, id: usize) -> W {
        (self)(id)
    }
}

