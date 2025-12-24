use crate::map_reduce_logic::map_logic;
use crate::state_access::StateAccess;
use crate::work_channel::WorkChannel;
use crate::worker::Worker;
use crate::worker_runtime::{ShutdownSignal, WorkerRuntime};
use tokio::sync::mpsc;

/// Work assignment for a mapper - describes what chunk to process
#[derive(Clone)]
pub struct WorkAssignment {
    pub chunk_id: usize,
    pub data: Vec<String>,
    pub targets: Vec<String>,
}

/// Mapper worker that searches for target words in its data chunk
/// Generic over state access, work channel, runtime, and shutdown mechanism
pub struct Mapper<S, W, R, SD>
where
    S: StateAccess,
    W: WorkChannel<WorkAssignment, mpsc::Sender<usize>>,
    R: WorkerRuntime,
    SD: ShutdownSignal,
{
    work_channel: W,
    task_handle: R::Handle,
    _phantom: std::marker::PhantomData<(S, SD)>,
}

impl<S, W, R, SD> Mapper<S, W, R, SD>
where
    S: StateAccess,
    W: WorkChannel<WorkAssignment, mpsc::Sender<usize>>,
    R: WorkerRuntime,
    SD: ShutdownSignal,
{
    pub fn new(
        id: usize,
        state: S,
        shutdown_signal: SD,
        work_rx: mpsc::Receiver<(WorkAssignment, mpsc::Sender<usize>)>,
        work_channel: W,
    ) -> Self {
        let handle = R::spawn(move || Self::run_task(id, work_rx, state, shutdown_signal));

        Self {
            work_channel,
            task_handle: handle,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Sends a work assignment to the mapper
    pub fn send_map_assignment(
        &self,
        assignment: WorkAssignment,
        complete_tx: mpsc::Sender<usize>,
    ) {
        self.work_channel.send_work(assignment, complete_tx);
    }

    /// Waits for the mapper task to complete
    pub async fn wait(self) -> Result<(), R::Error> {
        drop(self.work_channel); // Close the channel to signal task to exit
        R::join(self.task_handle).await
    }

    async fn run_task(
        id: usize,
        mut work_rx: mpsc::Receiver<(WorkAssignment, mpsc::Sender<usize>)>,
        state: S,
        shutdown_signal: SD,
    ) {
        loop {
            tokio::select! {
                work = work_rx.recv() => {
                    match work {
                        Some((assignment, complete_tx)) => {
                            if id.is_multiple_of(10) {
                                println!("Mapper {} processing chunk {}", id, assignment.chunk_id);
                            }

                            // Check for cancellation
                            if shutdown_signal.is_cancelled() {
                                println!("Mapper {} cancelled", id);
                                return;
                            }

                            // Use pure business logic
                            let results = map_logic(&assignment.data, &assignment.targets);

                            // Write results to state
                            for (target, count) in results {
                                if count > 0 {
                                    state.update(target, count);
                                }
                            }

                            if id.is_multiple_of(10) {
                                println!("Mapper {} finished chunk {}", id, assignment.chunk_id);
                            }

                            // Notify orchestrator that this mapper is done
                            let _ = complete_tx.send(id).await;
                        }
                        None => {
                            // Channel closed, exit
                            break;
                        }
                    }
                }
            }
        }
    }
}

impl<S, W, R, SD> Worker for Mapper<S, W, R, SD>
where
    S: StateAccess,
    W: WorkChannel<WorkAssignment, mpsc::Sender<usize>>,
    R: WorkerRuntime,
    SD: ShutdownSignal,
{
    type Assignment = WorkAssignment;
    type Completion = mpsc::Sender<usize>;
    type Error = R::Error;

    fn send_work(&self, assignment: Self::Assignment, complete_tx: Self::Completion) {
        self.send_map_assignment(assignment, complete_tx);
    }

    async fn wait(self) -> Result<(), Self::Error> {
        Mapper::wait(self).await
    }
}
