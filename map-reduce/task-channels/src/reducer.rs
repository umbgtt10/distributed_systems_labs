use map_reduce_core::map_reduce_problem::MapReduceProblem;
use map_reduce_core::shutdown_signal::ShutdownSignal;
use map_reduce_core::state_access::StateAccess;
use map_reduce_core::work_channel::WorkChannel;
use map_reduce_core::worker::Worker;
use map_reduce_core::worker_runtime::WorkerRuntime;
use tokio::sync::mpsc;

/// Reducer worker that executes reduce work for a given problem
/// Generic over problem type, state access, work channel, runtime, and shutdown mechanism
pub struct Reducer<P, S, W, R, SD>
where
    P: MapReduceProblem,
    S: StateAccess,
    W: WorkChannel<P::ReduceAssignment, mpsc::Sender<Result<usize, ()>>>,
    R: WorkerRuntime,
    SD: ShutdownSignal,
{
    work_channel: W,
    task_handle: R::Handle,
    _phantom: std::marker::PhantomData<(P, S, SD)>,
}

impl<P, S, W, R, SD> Reducer<P, S, W, R, SD>
where
    P: MapReduceProblem,
    S: StateAccess,
    W: WorkChannel<P::ReduceAssignment, mpsc::Sender<Result<usize, ()>>>,
    R: WorkerRuntime,
    SD: ShutdownSignal,
{
    pub fn new(
        id: usize,
        state: S,
        shutdown_signal: SD,
        work_rx: mpsc::Receiver<(P::ReduceAssignment, mpsc::Sender<Result<usize, ()>>)>,
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
        mut work_rx: mpsc::Receiver<(P::ReduceAssignment, mpsc::Sender<Result<usize, ()>>)>,
        state: S,
        shutdown_signal: SD,
    ) {
        loop {
            tokio::select! {
                work = work_rx.recv() => {
                    match work {
                        Some((assignment, complete_tx)) => {
                            if id.is_multiple_of(2) {
                                println!("Reducer {} started work", id);
                            }

                            // Check for cancellation
                            if shutdown_signal.is_cancelled() {
                                println!("Reducer {} cancelled", id);
                                return;
                            }

                            // Execute problem-specific reduce work with error handling
                            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                                P::reduce_work(&assignment, &state);
                            }));

                            match result {
                                Ok(_) => {
                                    if id.is_multiple_of(2) {
                                        println!("Reducer {} finished", id);
                                    }
                                    // Notify orchestrator of success
                                    let _ = complete_tx.send(Ok(id)).await;
                                }
                                Err(_) => {
                                    eprintln!("❌ Reducer {} panicked during work!", id);
                                    // Notify orchestrator of failure
                                    let _ = complete_tx.send(Err(())).await;
                                }
                            }
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

    /// Waits for the reducer task to complete
    pub async fn wait(self) -> Result<(), R::Error> {
        drop(self.work_channel); // Close the channel to signal task to exit
        R::join(self.task_handle).await
    }
}

impl<P, S, W, R, SD> Worker for Reducer<P, S, W, R, SD>
where
    P: MapReduceProblem,
    S: StateAccess,
    W: WorkChannel<P::ReduceAssignment, mpsc::Sender<Result<usize, ()>>>,
    R: WorkerRuntime,
    SD: ShutdownSignal,
{
    type Assignment = P::ReduceAssignment;
    type Completion = mpsc::Sender<Result<usize, ()>>;
    type Error = R::Error;

    fn send_work(&self, assignment: Self::Assignment, complete_tx: Self::Completion) {
        self.work_channel.send_work(assignment, complete_tx);
    }

    async fn wait(self) -> Result<(), Self::Error> {
        Reducer::wait(self).await
    }
}
