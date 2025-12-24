use map_reduce_core::map_reduce_problem::MapReduceProblem;
use map_reduce_core::shutdown_signal::ShutdownSignal;
use map_reduce_core::state_access::StateAccess;
use map_reduce_core::work_channel::WorkChannel;
use map_reduce_core::worker::Worker;
use map_reduce_core::worker_runtime::WorkerRuntime;
use rand::Rng;
use std::marker::PhantomData;
use std::panic::{catch_unwind, AssertUnwindSafe};
use tokio::sync::mpsc;
use tokio::time::{sleep, Duration};

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
    _phantom: PhantomData<(P, S, SD)>,
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
        failure_probability: u32,
        straggler_probability: u32,
        straggler_delay_ms: u64,
    ) -> Self {
        let handle = R::spawn(move || {
            Self::run_task(
                id,
                work_rx,
                state,
                shutdown_signal,
                failure_probability,
                straggler_probability,
                straggler_delay_ms,
            )
        });

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
        failure_probability: u32,
        straggler_probability: u32,
        straggler_delay_ms: u64,
    ) {
        loop {
            tokio::select! {
                work = work_rx.recv() => {
                    match work {
                        Some((assignment, complete_tx)) => {
                            // Check for cancellation
                            if shutdown_signal.is_cancelled() {
                                println!("Reducer {} cancelled", id);
                                return;
                            }

                            // Simulate random failure based on probability
                            let should_fail = if failure_probability > 0 {
                                let random_value = rand::rng().random_range(0..100);
                                random_value < failure_probability
                            } else {
                                false
                            };

                            if should_fail {
                                eprintln!("💥 Reducer {} simulated failure!", id);
                                let _ = complete_tx.send(Err(())).await;
                                continue;
                            }

                            // Simulate straggler (slow worker) with random delay
                            if straggler_probability > 0 {
                                let random_value = rand::rng().random_range(0..100);
                                if random_value < straggler_probability {
                                    let delay = rand::rng().random_range(1..=straggler_delay_ms);
                                    eprintln!("🐌 Reducer {} is a straggler! Delaying {}ms", id, delay);
                                    sleep(Duration::from_millis(delay)).await;
                                }
                            }

                            // Execute problem-specific reduce work with error handling
                            let result = catch_unwind(AssertUnwindSafe(|| {
                                P::reduce_work(&assignment, &state);
                            }));

                            match result {
                                Ok(_) => {
                                    println!("Reducer {} finished", id);
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
