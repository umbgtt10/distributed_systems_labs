use map_reduce_core::completion_signaling::CompletionSignaling;
use map_reduce_core::work_distributor::WorkDistributor;
use map_reduce_core::worker::Worker;
use std::collections::HashMap;

/// Implementation of WorkDistributor for async task-based workers
/// Generic over the completion signaling mechanism
/// Supports fault tolerance: respawns failed workers and reassigns work
pub struct TaskWorkDistributor<W: Worker, CS: CompletionSignaling, F>
where
    F: FnMut(usize) -> W,
{
    worker_factory: F,
    _phantom: std::marker::PhantomData<(W, CS)>,
}

impl<W: Worker, CS: CompletionSignaling, F> TaskWorkDistributor<W, CS, F>
where
    F: FnMut(usize) -> W + Send,
{
    pub fn new(worker_factory: F) -> Self {
        Self {
            worker_factory,
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

        // Track which assignment each worker is currently processing
        let mut worker_assignments: HashMap<usize, <Self::Worker as Worker>::Assignment> = HashMap::new();

        // Track active workers
        let mut active_workers = 0;

        // Assign initial work to all workers
        for worker_idx in 0..workers.len() {
            if assignment_index < assignments.len() {
                let assignment = assignments[assignment_index].clone();
                let token = signaling.get_token(worker_idx);
                workers[worker_idx].send_work(assignment.clone(), token.into());
                worker_assignments.insert(worker_idx, assignment);
                assignment_index += 1;
                active_workers += 1;
            }
        }

        // As workers complete or fail, assign them more work
        while active_workers > 0 {
            if let Some(result) = signaling.wait_next().await {
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
                            worker_assignments.insert(worker_id, assignment);
                            assignment_index += 1;
                            active_workers += 1;
                        }
                    }
                    Err(worker_id) => {
                        // Worker failed - respawn and reassign
                        println!("⚠️  Worker {} failed! Respawning and reassigning work...", worker_id);

                        // Get the assignment that failed
                        if let Some(assignment) = worker_assignments.get(&worker_id).cloned() {
                            // Wait for the failed worker to clean up (drop it)
                            let failed_worker = std::mem::replace(&mut workers[worker_id], (self.worker_factory)(worker_id));
                            drop(failed_worker);

                            // Reassign the same work to the new worker
                            let token = signaling.get_token(worker_id);
                            workers[worker_id].send_work(assignment.clone(), token.into());
                            worker_assignments.insert(worker_id, assignment);
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
