use crate::map_reduce_logic::reduce_logic;
use crate::state_access::StateAccess;
use crate::work_channel::WorkChannel;
use crate::worker::Worker;
use crate::worker_runtime::{ShutdownSignal, WorkerRuntime};
use tokio::sync::mpsc;

/// Reducer assignment - which keys this reducer is responsible for
#[derive(Clone)]
pub struct ReducerAssignment {
    pub keys: Vec<String>,
}

/// Reducer worker that sums up vectors into final counts
/// Generic over state access, work channel, runtime, and shutdown mechanism
pub struct Reducer<S, W, R, SD>
where
    S: StateAccess,
    W: WorkChannel<ReducerAssignment, mpsc::Sender<usize>>,
    R: WorkerRuntime,
    SD: ShutdownSignal,
{
    work_channel: W,
    task_handle: R::Handle,
    _phantom: std::marker::PhantomData<(S, SD)>,
}

impl<S, W, R, SD> Reducer<S, W, R, SD>
where
    S: StateAccess,
    W: WorkChannel<ReducerAssignment, mpsc::Sender<usize>>,
    R: WorkerRuntime,
    SD: ShutdownSignal,
{
    pub fn new(
        id: usize,
        state: S,
        shutdown_signal: SD,
        work_rx: mpsc::Receiver<(ReducerAssignment, mpsc::Sender<usize>)>,
        work_channel: W,
    ) -> Self {
        let handle = R::spawn(move || Self::run_task(id, work_rx, state, shutdown_signal));

        Self {
            work_channel,
            task_handle: handle,
            _phantom: std::marker::PhantomData,
        }
    }

    async fn run_task(
        id: usize,
        mut work_rx: mpsc::Receiver<(ReducerAssignment, mpsc::Sender<usize>)>,
        state: S,
        shutdown_signal: SD,
    ) {
        loop {
            tokio::select! {
                work = work_rx.recv() => {
                    match work {
                        Some((assignment, complete_tx)) => {
                            if id.is_multiple_of(2) {
                                println!("Reducer {} started for {} keys", id, assignment.keys.len());
                            }

                            // Check for cancellation
                            if shutdown_signal.is_cancelled() {
                                println!("Reducer {} cancelled", id);
                                return;
                            }

                            for key in assignment.keys {
                                // Get the vector for this key
                                let values = state.get(&key);

                                // Use pure business logic to reduce
                                let count = reduce_logic(values);

                                // Update state with final count (replacing the vector)
                                state.replace(key, count);
                            }

                            if id.is_multiple_of(2) {
                                println!("Reducer {} finished", id);
                            }

                            // Notify orchestrator that this reducer is done
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

    /// Sends a work assignment to the reducer
    pub fn send_reduce_assignment(
        &self,
        assignment: ReducerAssignment,
        complete_tx: mpsc::Sender<usize>,
    ) {
        self.work_channel.send_work(assignment, complete_tx);
    }

    /// Waits for the reducer task to complete
    pub async fn wait(self) -> Result<(), R::Error> {
        drop(self.work_channel); // Close the channel to signal task to exit
        R::join(self.task_handle).await
    }
}

impl<S, W, R, SD> Worker for Reducer<S, W, R, SD>
where
    S: StateAccess,
    W: WorkChannel<ReducerAssignment, mpsc::Sender<usize>>,
    R: WorkerRuntime,
    SD: ShutdownSignal,
{
    type Assignment = ReducerAssignment;
    type Completion = mpsc::Sender<usize>;
    type Error = R::Error;

    fn send_work(&self, assignment: Self::Assignment, complete_tx: Self::Completion) {
        self.send_reduce_assignment(assignment, complete_tx);
    }

    async fn wait(self) -> Result<(), Self::Error> {
        Reducer::wait(self).await
    }
}
