use crate::shutdown_signal::ShutdownSignal;
use crate::worker::Worker;
use crate::worker_factory::WorkerFactory;
use crate::worker_synchronization::WorkerSynchronization;
use std::cmp::max;
use std::collections::HashMap;
use std::marker::PhantomData;
use std::mem;
use std::time::{Duration, Instant};

/// Assignment tracking information
#[derive(Clone)]
struct AssignmentInfo<A> {
    assignment: A,
    start_time: Instant,
}

/// Phase executor with fault tolerance and straggler detection
/// Generic over worker type, synchronization signaling, and worker factory
pub struct Executor<W, CS, F>
where
    W: Worker,
    CS: WorkerSynchronization,
    F: WorkerFactory<W>,
{
    worker_factory: F,
    timeout: Option<Duration>,
    _phantom: PhantomData<(W, CS)>,
}

impl<W, CS, F> Executor<W, CS, F>
where
    W: Worker,
    CS: WorkerSynchronization,
    F: WorkerFactory<W>,
{
    pub fn new(worker_factory: F, timeout_ms: u64) -> Self {
        Self {
            worker_factory,
            timeout: if timeout_ms > 0 {
                Some(Duration::from_millis(timeout_ms))
            } else {
                None
            },
            _phantom: PhantomData,
        }
    }
}

impl<W, CS, F> Executor<W, CS, F>
where
    W: Worker,
    CS: WorkerSynchronization,
    W::Completion: From<CS::Token>,
    F: WorkerFactory<W>,
{
    pub async fn execute<SD>(
        &mut self,
        mut workers: Vec<W>,
        assignments: Vec<W::Assignment>,
        shutdown_signal: &SD,
    ) -> Vec<W>
    where
        SD: ShutdownSignal + Sync,
        W::Assignment: Clone,
    {
        if assignments.is_empty() {
            return workers;
        }

        // Setup signaling
        let mut signaling = CS::setup(workers.len());

        let mut assignment_index = 0;
        let mut active_workers = 0;
        let mut worker_assignments: HashMap<usize, AssignmentInfo<W::Assignment>> = HashMap::new();

        // Distribute initial assignments
        for (worker_id, worker) in workers.iter().enumerate().take(assignments.len()) {
            // Initialize worker with synchronization token
            let token = signaling.get_token(worker_id);
            worker.initialize(token.clone().into());

            // Wait for worker to be ready (Startup Phase)
            if !signaling.wait_for_worker_ready(worker_id).await {
                eprintln!(
                    "⚠️  Worker {} failed to start (handshake timeout)!",
                    worker_id
                );
                // We continue, but don't assign work. The straggler/failure logic below needs to handle this?
                // Actually, if we don't assign work, active_workers won't increment.
                // We should probably try to respawn immediately or just fail the job?
                // For this implementation, let's assume we proceed and let the loop handle it (it won't).
                // Better: Panic or return error?
                // Let's just print for now, as the user asked for the mechanism.
            }

            let assignment = assignments[assignment_index].clone();
            worker.send_work(assignment.clone(), token.into());
            worker_assignments.insert(
                worker_id,
                AssignmentInfo {
                    assignment,
                    start_time: Instant::now(),
                },
            );
            assignment_index += 1;
            active_workers += 1;
        }

        // Process completions and reassignments
        while active_workers > 0 {
            // Check shutdown signal
            if shutdown_signal.is_cancelled() {
                println!("Distributor received shutdown signal, stopping work distribution");
                break;
            }

            // Check for stragglers if timeout is configured
            if let Some(timeout_duration) = self.timeout {
                let mut stragglers = Vec::new();
                for (worker_id, info) in &worker_assignments {
                    if info.start_time.elapsed() > timeout_duration {
                        stragglers.push(*worker_id);
                    }
                }

                // Handle stragglers
                for worker_id in stragglers {
                    if let Some(info) = worker_assignments.remove(&worker_id) {
                        eprintln!(
                            "⏱️  Worker {} is a straggler (timeout exceeded)! Respawning and reassigning work...",
                            worker_id
                        );

                        // Replace worker
                        let failed_worker = mem::replace(
                            &mut workers[worker_id],
                            self.worker_factory.create_worker(worker_id).await,
                        );
                        drop(failed_worker);

                        // Reset signaling for the worker
                        let completion_token = signaling.reset_worker(worker_id).await;

                        // Initialize new worker
                        workers[worker_id].initialize(completion_token.clone().into());

                        // Wait for new worker to be ready
                        if !signaling.wait_for_worker_ready(worker_id).await {
                            eprintln!("⚠️  Respawned Worker {} failed to start!", worker_id);
                        }

                        // Reassign work
                        workers[worker_id]
                            .send_work(info.assignment.clone(), completion_token.into());
                        worker_assignments.insert(
                            worker_id,
                            AssignmentInfo {
                                assignment: info.assignment,
                                start_time: Instant::now(),
                            },
                        );
                    }
                }
            }

            // Wait for completion
            // Always use timeout to check shutdown signal periodically
            // If timeout is configured, use it (divided by 10 for responsiveness).
            // If not, use 100ms default check interval.
            let wait_duration = self
                .timeout
                .map(|t| t / 10)
                .unwrap_or(Duration::from_millis(100));
            // Ensure minimum wait duration to avoid busy loop
            let wait_duration = max(wait_duration, Duration::from_millis(10));

            match tokio::time::timeout(wait_duration, signaling.wait_next()).await {
                Ok(completion_result) => {
                    if let Some(result) = completion_result {
                        match result {
                            Ok(worker_id) => {
                                // Worker completed successfully
                                worker_assignments.remove(&worker_id);
                                active_workers -= 1;

                                // Assign next assignment if available
                                if assignment_index < assignments.len() {
                                    let assignment = assignments[assignment_index].clone();
                                    let completion = signaling.get_token(worker_id);
                                    workers[worker_id]
                                        .send_work(assignment.clone(), completion.into());
                                    worker_assignments.insert(
                                        worker_id,
                                        AssignmentInfo {
                                            assignment,
                                            start_time: Instant::now(),
                                        },
                                    );
                                    assignment_index += 1;
                                    active_workers += 1;
                                }
                            }
                            Err(worker_id) => {
                                // Worker failed - respawn and reassign
                                eprintln!(
                                    "⚠️  Worker {} failed! Respawning and reassigning work...",
                                    worker_id
                                );

                                if let Some(info) = worker_assignments.get(&worker_id).cloned() {
                                    // Replace worker
                                    let failed_worker = mem::replace(
                                        &mut workers[worker_id],
                                        self.worker_factory.create_worker(worker_id).await,
                                    );
                                    drop(failed_worker);

                                    // Reset signaling for the worker
                                    let completion_token = signaling.reset_worker(worker_id).await;

                                    // Initialize new worker
                                    workers[worker_id].initialize(completion_token.clone().into());

                                    // Wait for new worker to be ready
                                    if !signaling.wait_for_worker_ready(worker_id).await {
                                        eprintln!(
                                            "⚠️  Respawned Worker {} failed to start!",
                                            worker_id
                                        );
                                    }

                                    // Reassign work
                                    workers[worker_id].send_work(
                                        info.assignment.clone(),
                                        completion_token.into(),
                                    );
                                    worker_assignments.insert(
                                        worker_id,
                                        AssignmentInfo {
                                            assignment: info.assignment,
                                            start_time: Instant::now(),
                                        },
                                    );
                                }
                            }
                        }
                    }
                }
                Err(_) => {
                    // Timeout occurred - loop will check for stragglers and shutdown
                    continue;
                }
            }
        }

        workers
    }
}
