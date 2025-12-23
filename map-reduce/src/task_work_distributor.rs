use crate::completion_signaling::CompletionSignaling;
use crate::work_distributor::WorkDistributor;
use crate::worker::Worker;

/// Implementation of WorkDistributor for async task-based workers
/// Generic over the completion signaling mechanism
pub struct TaskWorkDistributor<W: Worker, CS: CompletionSignaling> {
    _phantom: std::marker::PhantomData<(W, CS)>,
}

impl<W: Worker, CS: CompletionSignaling> TaskWorkDistributor<W, CS> {
    pub fn new() -> Self {
        Self {
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<W, CS> WorkDistributor for TaskWorkDistributor<W, CS>
where
    W: Worker,
    CS: CompletionSignaling,
    W::Completion: From<CS::Token>,
{
    type Worker = W;

    async fn distribute(
        &mut self,
        workers: Vec<Self::Worker>,
        assignments: Vec<<Self::Worker as Worker>::Assignment>,
    ) where
        <Self::Worker as Worker>::Assignment: Clone,
    {
        // Setup completion signaling for all workers
        let mut signaling = CS::setup(workers.len());

        // Track which assignments have been distributed and which workers are active
        let mut assignment_index = 0;
        let mut active_workers = 0;

        // Assign initial work to all workers
        for (worker_idx, worker) in workers.iter().enumerate() {
            if assignment_index < assignments.len() {
                let token = signaling.get_token(worker_idx);
                worker.send_work(assignments[assignment_index].clone(), token.into());
                assignment_index += 1;
                active_workers += 1;
            }
        }

        // As workers complete, assign them more work
        while active_workers > 0 {
            if let Some(worker_id) = signaling.wait_next().await {
                active_workers -= 1;

                // Assign next assignment if available
                if assignment_index < assignments.len() {
                    let token = signaling.get_token(worker_id);
                    workers[worker_id]
                        .send_work(assignments[assignment_index].clone(), token.into());
                    assignment_index += 1;
                    active_workers += 1;
                }
            }
        }

        // Wait for all workers to fully shut down
        for (idx, worker) in workers.into_iter().enumerate() {
            if let Err(e) = worker.wait().await {
                eprintln!("Worker {} task failed: {}", idx, e);
            }
        }
    }
}
