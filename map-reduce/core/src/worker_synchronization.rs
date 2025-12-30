// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use std::future::Future;

/// Trait for abstracting synchronization signaling mechanisms
/// This allows different implementations for tasks, threads, and processes
pub trait WorkerSynchronization: Send {
    /// The sender type passed to workers for signaling completion
    type StatusSender: Clone + Send;

    /// Setup synchronization signaling for N workers
    fn setup(num_workers: usize) -> Self;

    /// Get the synchronization sender for a specific worker
    fn get_status_sender(&self, worker_id: usize) -> Self::StatusSender;

    /// Wait for a specific worker to be ready
    /// Returns true if the worker is ready, false if it timed out or failed
    fn wait_for_worker_ready(&self, worker_id: usize) -> impl Future<Output = bool> + Send;

    /// Wait for the next worker to complete or fail
    /// Returns Ok(worker_id) on success, Err(worker_id) on failure
    /// Returns None if all workers are done
    fn wait_next(&mut self) -> impl Future<Output = Option<Result<usize, usize>>> + Send;

    /// Reset the signaling mechanism for a specific worker
    /// This drains any pending messages and returns a new sender for the new worker
    fn reset_worker(&mut self, worker_id: usize)
        -> impl Future<Output = Self::StatusSender> + Send;
}

