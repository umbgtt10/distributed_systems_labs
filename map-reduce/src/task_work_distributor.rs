use crate::work_distributor::WorkDistributor;
use crate::worker::Worker;
use tokio::sync::mpsc::{self, Sender};
use tokio_stream::wrappers::ReceiverStream;
use tokio_stream::{StreamExt, StreamMap};

/// Implementation of WorkDistributor for async task-based workers
/// Uses tokio channels and StreamMap for completion signaling
pub struct TaskWorkDistributor<W: Worker> {
    _phantom: std::marker::PhantomData<W>,
}

impl<W: Worker> TaskWorkDistributor<W> {
    pub fn new() -> Self {
        Self {
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<W> WorkDistributor for TaskWorkDistributor<W>
where
    W: Worker,
    W::Completion: From<Sender<usize>>,
{
    type Worker = W;

    async fn distribute(
        &mut self,
        workers: Vec<Self::Worker>,
        assignments: Vec<<Self::Worker as Worker>::Assignment>,
    ) where
        <Self::Worker as Worker>::Assignment: Clone,
    {
        // Create individual completion channels for each worker
        let mut completion_txs = Vec::new();
        let mut completion_streams = StreamMap::new();
        for worker_idx in 0..workers.len() {
            let (tx, rx) = mpsc::channel::<usize>(10);
            completion_txs.push(tx);
            completion_streams.insert(worker_idx, ReceiverStream::new(rx));
        }

        // Track which assignments have been distributed and which workers are active
        let mut assignment_index = 0;
        let mut active_workers = 0;

        // Assign initial work to all workers
        for (worker_idx, worker) in workers.iter().enumerate() {
            if assignment_index < assignments.len() {
                let tx = completion_txs[worker_idx].clone();
                worker.send_work(assignments[assignment_index].clone(), tx.into());
                assignment_index += 1;
                active_workers += 1;
            }
        }

        // As workers complete, assign them more work
        while active_workers > 0 {
            if let Some((_stream_idx, worker_id)) = completion_streams.next().await {
                active_workers -= 1;

                // Assign next assignment if available
                if assignment_index < assignments.len() {
                    let tx = completion_txs[worker_id].clone();
                    workers[worker_id].send_work(assignments[assignment_index].clone(), tx.into());
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
