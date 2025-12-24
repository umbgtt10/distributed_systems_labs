use map_reduce_core::map_reduce_problem::MapReduceProblem;
use map_reduce_core::shutdown_signal::ShutdownSignal;
use map_reduce_core::state_access::StateAccess;
use map_reduce_core::work_channel::WorkChannel;
use map_reduce_core::worker::Worker;
use map_reduce_core::worker_runtime::WorkerRuntime;
use tokio::sync::mpsc;

/// Mapper worker that executes map work for a given problem
/// Generic over problem type, state access, work channel, runtime, and shutdown mechanism
pub struct Mapper<P, S, W, R, SD>
where
    P: MapReduceProblem,
    S: StateAccess,
    W: WorkChannel<P::MapAssignment, mpsc::Sender<Result<usize, ()>>>,
    R: WorkerRuntime,
    SD: ShutdownSignal,
{
    work_channel: W,
    task_handle: R::Handle,
    _phantom: std::marker::PhantomData<(P, S, SD)>,
}

impl<P, S, W, R, SD> Mapper<P, S, W, R, SD>
where
    P: MapReduceProblem,
    S: StateAccess,
    W: WorkChannel<P::MapAssignment, mpsc::Sender<Result<usize, ()>>>,
    R: WorkerRuntime,
    SD: ShutdownSignal,
{
    pub fn new(
        id: usize,
        state: S,
        shutdown_signal: SD,
        work_rx: mpsc::Receiver<(P::MapAssignment, mpsc::Sender<Result<usize, ()>>)>,
        work_channel: W,
    ) -> Self {
        let handle = R::spawn(move || Self::run_task(id, work_rx, state, shutdown_signal));

        Self {
            work_channel,
            task_handle: handle,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Waits for the mapper task to complete
    pub async fn wait(self) -> Result<(), R::Error> {
        drop(self.work_channel); // Close the channel to signal task to exit
        R::join(self.task_handle).await
    }

    async fn run_task(
        id: usize,
        mut work_rx: mpsc::Receiver<(P::MapAssignment, mpsc::Sender<Result<usize, ()>>)>,
        state: S,
        shutdown_signal: SD,
    ) {
        loop {
            tokio::select! {
                work = work_rx.recv() => {
                    match work {
                        Some((assignment, complete_tx)) => {
                            if id.is_multiple_of(10) {
                                println!("Mapper {} processing work", id);
                            }

                            // Check for cancellation
                            if shutdown_signal.is_cancelled() {
                                println!("Mapper {} cancelled", id);
                                return;
                            }

                            // Execute problem-specific map work with error handling
                            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                                P::map_work(&assignment, &state);
                            }));

                            match result {
                                Ok(_) => {
                                    if id.is_multiple_of(10) {
                                        println!("Mapper {} finished work", id);
                                    }
                                    // Notify orchestrator of success
                                    let _ = complete_tx.send(Ok(id)).await;
                                }
                                Err(_) => {
                                    eprintln!("❌ Mapper {} panicked during work!", id);
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
}

impl<P, S, W, R, SD> Worker for Mapper<P, S, W, R, SD>
where
    P: MapReduceProblem,
    S: StateAccess,
    W: WorkChannel<P::MapAssignment, mpsc::Sender<Result<usize, ()>>>,
    R: WorkerRuntime,
    SD: ShutdownSignal,
{
    type Assignment = P::MapAssignment;
    type Completion = mpsc::Sender<Result<usize, ()>>;
    type Error = R::Error;

    fn send_work(&self, assignment: Self::Assignment, complete_tx: Self::Completion) {
        self.work_channel.send_work(assignment, complete_tx);
    }

    async fn wait(self) -> Result<(), Self::Error> {
        Mapper::wait(self).await
    }
}
