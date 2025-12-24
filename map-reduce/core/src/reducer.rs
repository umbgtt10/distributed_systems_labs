use crate::map_reduce_problem::MapReduceProblem;
use crate::shutdown_signal::ShutdownSignal;
use crate::state_access::StateAccess;
use crate::work_channel::WorkChannel;
use crate::worker_io::{AsyncCompletionSender, AsyncWorkReceiver};
use crate::worker_runtime::WorkerRuntime;
use rand::Rng;
use std::marker::PhantomData;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::time::Duration;

/// Standard Reducer worker implementation
pub struct Reducer<P, S, W, R, SD, WR, CS>
where
    P: MapReduceProblem,
    S: StateAccess,
    W: WorkChannel<P::ReduceAssignment, CS>,
    R: WorkerRuntime,
    SD: ShutdownSignal,
    WR: AsyncWorkReceiver<P::ReduceAssignment, CS>,
    CS: AsyncCompletionSender,
{
    work_channel: W,
    task_handle: R::Handle,
    _phantom: PhantomData<(P, S, SD, WR, CS)>,
}

impl<P, S, W, R, SD, WR, CS> Reducer<P, S, W, R, SD, WR, CS>
where
    P: MapReduceProblem,
    S: StateAccess + Send + Sync + 'static,
    W: WorkChannel<P::ReduceAssignment, CS> + 'static,
    R: WorkerRuntime,
    SD: ShutdownSignal + Send + 'static,
    WR: AsyncWorkReceiver<P::ReduceAssignment, CS> + 'static,
    CS: AsyncCompletionSender + 'static,
{
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: usize,
        state: S,
        shutdown_signal: SD,
        work_rx: WR,
        work_channel: W,
        failure_probability: u32,
        straggler_probability: u32,
        straggler_delay_ms: u64,
    ) -> Self {
        let task_handle = R::spawn(move || {
            Self::run(
                id,
                state,
                shutdown_signal,
                work_rx,
                failure_probability,
                straggler_probability,
                straggler_delay_ms,
            )
        });

        Self {
            work_channel,
            task_handle,
            _phantom: PhantomData,
        }
    }

    pub async fn wait(self) -> Result<(), R::Error> {
        R::join(self.task_handle).await
    }

    async fn run(
        id: usize,
        state: S,
        shutdown_signal: SD,
        mut work_rx: WR,
        failure_probability: u32,
        straggler_probability: u32,
        straggler_delay_ms: u64,
    ) {
        loop {
            // Check for shutdown
            if shutdown_signal.is_cancelled() {
                break;
            }

            // Try to receive work with timeout
            let work_result =
                tokio::time::timeout(Duration::from_millis(100), work_rx.recv()).await;

            match work_result {
                Ok(Some((assignment, completion_sender))) => {
                    // Simulate random failure
                    if failure_probability > 0 {
                        let random_value = rand::rng().random_range(0..100);
                        if random_value < failure_probability {
                            eprintln!("💥 Reducer {} simulated failure!", id);
                            completion_sender.send(Err(())).await;
                            continue;
                        }
                    }

                    // Simulate straggler
                    if straggler_probability > 0 {
                        let random_value = rand::rng().random_range(0..100);
                        if random_value < straggler_probability {
                            let delay = rand::rng().random_range(1..=straggler_delay_ms);
                            eprintln!("🐌 Reducer {} is a straggler! Delaying {}ms", id, delay);
                            tokio::time::sleep(Duration::from_millis(delay)).await;
                        }
                    }

                    // Execute work with error handling
                    let result = catch_unwind(AssertUnwindSafe(|| {
                        P::reduce_work(&assignment, &state);
                    }));

                    match result {
                        Ok(_) => {
                            if completion_sender.send(Ok(id)).await {
                                println!("Reducer {} finished work", id);
                            } else {
                                // Failed to send completion - likely a zombie worker
                            }
                        }
                        Err(_) => {
                            eprintln!("❌ Reducer {} panicked during work!", id);
                            let _ = completion_sender.send(Err(())).await;
                        }
                    }
                }
                Ok(None) => {
                    // Channel closed
                    break;
                }
                Err(_) => {
                    // Timeout, loop again
                }
            }
        }
    }
}

impl<P, S, W, R, SD, WR, CS> crate::worker::Worker for Reducer<P, S, W, R, SD, WR, CS>
where
    P: MapReduceProblem,
    S: StateAccess + Send + Sync + 'static,
    W: WorkChannel<P::ReduceAssignment, CS> + 'static,
    R: WorkerRuntime,
    SD: ShutdownSignal + Send + 'static,
    WR: AsyncWorkReceiver<P::ReduceAssignment, CS> + 'static,
    CS: AsyncCompletionSender + 'static,
{
    type Assignment = P::ReduceAssignment;
    type Completion = CS;
    type Error = R::Error;

    fn send_work(&self, assignment: Self::Assignment, complete_tx: Self::Completion) {
        self.work_channel.send_work(assignment, complete_tx);
    }

    async fn wait(self) -> Result<(), Self::Error> {
        self.wait().await
    }
}
