use map_reduce_core::completion_signaling::CompletionSignaling;
use map_reduce_core::work_distributor::WorkDistributor;
use map_reduce_core::worker::Worker;
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Assignment tracking information
#[derive(Clone)]
struct AssignmentInfo<A> {
    assignment: A,
    start_time: Instant,
}

/// Implementation of WorkDistributor for async task-based workers
/// Generic over the completion signaling mechanism
/// Supports fault tolerance: respawns failed workers and reassigns work
/// Supports straggler detection: timeouts and preemptive reassignment
pub struct TaskWorkDistributor<W: Worker, CS: CompletionSignaling, F>
where
    F: FnMut(usize) -> W,
{
    worker_factory: F,
    timeout: Option<Duration>,
    _phantom: std::marker::PhantomData<(W, CS)>,
}

impl<W: Worker, CS: CompletionSignaling, F> TaskWorkDistributor<W, CS, F>
where
    F: FnMut(usize) -> W + Send,
{
    pub fn new(worker_factory: F) -> Self {
        Self {
            worker_factory,
            timeout: None,
            _phantom: std::marker::PhantomData,
        }
    }

    pub fn with_timeout(worker_factory: F, timeout_ms: u64) -> Self {
        Self {
            worker_factory,
            timeout: if timeout_ms > 0 { Some(Duration::from_millis(timeout_ms)) } else { None },
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<W, CS, F> WorkDistributor for TaskWorkDistributor<W, CS, F>
where
    W: Worker,
    CS: CompletionSignaling,
    W::Completion: From<CS::Token>,
    F: FnMut(usize) -> W + Send,
{
    type Worker = W;

    async fn distribute(
        &mut self,
        mut workers: Vec<Self::Worker>,
        assignments: Vec<<Self::Worker as Worker>::Assignment>,
    ) where
        <Self::Worker as Worker>::Assignment: Clone,
    {
        // Setup completion signaling for all workers
        let mut signaling = CS::setup(workers.len());

        // Track which assignments have been distributed
        let mut assignment_index = 0;

        // Track which assignment each worker is currently processing with start time
        let mut worker_assignments: HashMap<usize, AssignmentInfo<<Self::Worker as Worker>::Assignment>> = HashMap::new();

        // Track active workers
        let mut active_workers = 0;

        // Assign initial work to all workers
        for worker_idx in 0..workers.len() {
            if assignment_index < assignments.len() {
                let assignment = assignments[assignment_index].clone();
                let token = signaling.get_token(worker_idx);
                workers[worker_idx].send_work(assignment.clone(), token.into());
                worker_assignments.insert(worker_idx, AssignmentInfo {
                    assignment,
                    start_time: Instant::now(),
                });
                assignment_index += 1;
                active_workers += 1;
            }
        }

        // As workers complete or fail, assign them more work
        while active_workers > 0 {
            // Check for stragglers (workers that exceed timeout)
            if let Some(timeout) = self.timeout {
                let now = Instant::now();
                let mut stragglers = Vec::new();

                for (worker_id, info) in &worker_assignments {
                    if now.duration_since(info.start_time) > timeout {
                        stragglers.push(*worker_id);
                    }
                }

                // Handle stragglers as failures - respawn and reassign
                for worker_id in stragglers {
                    if let Some(info) = worker_assignments.remove(&worker_id) {
                        eprintln!("⏱️  Worker {} is a straggler (timeout exceeded)! Respawning and reassigning work...", worker_id);

                        // Drop the slow worker and spawn a new one
                        let failed_worker = std::mem::replace(&mut workers[worker_id], (self.worker_factory)(worker_id));
                        drop(failed_worker);

                        // Drain any pending completion messages from the old worker
                        // This is critical to avoid receiving stale completions from timed-out workers
                        signaling.drain_worker(worker_id).await;

                        // Reassign the same work to the new worker
                        let token = signaling.get_token(worker_id);
                        workers[worker_id].send_work(info.assignment.clone(), token.into());
                        worker_assignments.insert(worker_id, AssignmentInfo {
                            assignment: info.assignment,
                            start_time: Instant::now(),
                        });
                        // active_workers stays the same - we replaced the worker
                    }
                }
            }

            // Wait for completion with a small timeout to allow straggler checks
            let wait_duration = self.timeout.map(|t| t / 10).unwrap_or(Duration::from_secs(60));
            if let Ok(Some(result)) = tokio::time::timeout(wait_duration, signaling.wait_next()).await {
                match result {
                    Ok(worker_id) => {
                        // Worker completed successfully
                        worker_assignments.remove(&worker_id);
                        active_workers -= 1;

                        // Assign next assignment if available
                        if assignment_index < assignments.len() {
                            let assignment = assignments[assignment_index].clone();
                            let token = signaling.get_token(worker_id);
                            workers[worker_id].send_work(assignment.clone(), token.into());
                            worker_assignments.insert(worker_id, AssignmentInfo {
                                assignment,
                                start_time: Instant::now(),
                            });
                            assignment_index += 1;
                            active_workers += 1;
                        }
                    }
                    Err(worker_id) => {
                        // Worker failed - respawn and reassign
                        println!("⚠️  Worker {} failed! Respawning and reassigning work...", worker_id);

                        // Get the assignment that failed
                        if let Some(info) = worker_assignments.get(&worker_id).cloned() {
                            // Wait for the failed worker to clean up (drop it)
                            let failed_worker = std::mem::replace(&mut workers[worker_id], (self.worker_factory)(worker_id));
                            drop(failed_worker);

                            // Drain any pending completion messages from the old worker
                            // This is critical to avoid receiving stale completions from failed workers
                            signaling.drain_worker(worker_id).await;

                            // Reassign the same work to the new worker
                            let token = signaling.get_token(worker_id);
                            workers[worker_id].send_work(info.assignment.clone(), token.into());
                            worker_assignments.insert(worker_id, AssignmentInfo {
                                assignment: info.assignment,
                                start_time: Instant::now(),
                            });
                            // active_workers stays the same - we replaced the worker
                        }
                    }
                }
            }
        }

        // Wait for all workers to fully shut down
        for (idx, worker) in workers.into_iter().enumerate() {
            if let Err(e) = worker.wait().await {
                eprintln!("Worker {} task shutdown failed: {}", idx, e);
            }
        }
    }
}
