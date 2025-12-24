use crate::map_reduce_job::MapReduceJob;
use crate::shutdown_signal::ShutdownSignal;
use crate::state_access::StateAccess;
use crate::work_channel::WorkDistributor;
use crate::worker_io::{CompletionSender, WorkReceiver};
use crate::worker_runtime::{WorkerTask, WorkerRuntime};
use async_trait::async_trait;
use rand::Rng;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::marker::PhantomData;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::time::Duration;

#[derive(Serialize, Deserialize)]
#[serde(
    bound = "S: Serialize + DeserializeOwned, SD: Serialize + DeserializeOwned, WR: Serialize + DeserializeOwned"
)]
pub struct ReducerTask<P, S, SD, WR, CS> {
    pub id: usize,
    pub state: S,
    pub shutdown_signal: SD,
    pub work_rx: WR,
    pub failure_probability: u32,
    pub straggler_probability: u32,
    pub straggler_delay_ms: u64,
    #[serde(skip)]
    _phantom: PhantomData<(P, CS)>,
}

#[async_trait]
impl<P, S, SD, WR, CS> WorkerTask for ReducerTask<P, S, SD, WR, CS>
where
    P: MapReduceJob,
    S: StateAccess + Send + Sync + 'static,
    SD: ShutdownSignal + Send + 'static,
    WR: WorkReceiver<P::ReduceAssignment, CS> + 'static,
    CS: CompletionSender + 'static,
{
    type Output = ();

    async fn run(mut self) -> Self::Output {
        loop {
            // Check for shutdown
            if self.shutdown_signal.is_cancelled() {
                break;
            }

            // Try to receive work with timeout
            let work_result =
                tokio::time::timeout(Duration::from_millis(100), self.work_rx.recv()).await;

            match work_result {
                Ok(Some((assignment, completion_sender))) => {
                    // Simulate random failure
                    if self.failure_probability > 0 {
                        let random_value = rand::rng().random_range(0..100);
                        if random_value < self.failure_probability {
                            eprintln!("💥 Reducer {} simulated failure!", self.id);
                            completion_sender.send(Err(())).await;
                            continue;
                        }
                    }

                    // Simulate straggler
                    if self.straggler_probability > 0 {
                        let random_value = rand::rng().random_range(0..100);
                        if random_value < self.straggler_probability {
                            let delay = rand::rng().random_range(1..=self.straggler_delay_ms);
                            eprintln!(
                                "🐌 Reducer {} is a straggler! Delaying {}ms",
                                self.id, delay
                            );
                            tokio::time::sleep(Duration::from_millis(delay)).await;
                        }
                    }

                    // Execute work with error handling
                    let state = &self.state;
                    let result = catch_unwind(AssertUnwindSafe(|| {
                        P::reduce_work(&assignment, state);
                    }));

                    match result {
                        Ok(_) => {
                            if completion_sender.send(Ok(self.id)).await {
                                println!("Reducer {} finished work", self.id);
                            } else {
                                // Failed to send completion - likely a zombie worker
                            }
                        }
                        Err(_) => {
                            eprintln!("❌ Reducer {} panicked during work!", self.id);
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

/// Standard Reducer worker implementation
pub struct Reducer<P, S, W, R, SD, WR, CS>
where
    P: MapReduceJob,
    S: StateAccess,
    W: WorkDistributor<P::ReduceAssignment, CS>,
    R: WorkerRuntime<ReducerTask<P, S, SD, WR, CS>>,
    SD: ShutdownSignal,
    WR: WorkReceiver<P::ReduceAssignment, CS>,
    CS: CompletionSender,
{
    work_channel: W,
    task_handle: R::Handle,
    _phantom: PhantomData<(P, S, SD, WR, CS)>,
}

impl<P, S, W, R, SD, WR, CS> Reducer<P, S, W, R, SD, WR, CS>
where
    P: MapReduceJob,
    S: StateAccess + Send + Sync + 'static,
    W: WorkDistributor<P::ReduceAssignment, CS> + 'static,
    R: WorkerRuntime<ReducerTask<P, S, SD, WR, CS>>,
    SD: ShutdownSignal + Send + 'static,
    WR: WorkReceiver<P::ReduceAssignment, CS> + 'static,
    CS: CompletionSender + 'static,
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
        let task = ReducerTask {
            id,
            state,
            shutdown_signal,
            work_rx,
            failure_probability,
            straggler_probability,
            straggler_delay_ms,
            _phantom: PhantomData,
        };

        let task_handle = R::spawn(task);

        Self {
            work_channel,
            task_handle,
            _phantom: PhantomData,
        }
    }

    pub async fn wait(self) -> Result<(), R::Error> {
        R::join(self.task_handle).await
    }
}

impl<P, S, W, R, SD, WR, CS> crate::worker::Worker for Reducer<P, S, W, R, SD, WR, CS>
where
    P: MapReduceJob,
    S: StateAccess + Send + Sync + 'static,
    W: WorkDistributor<P::ReduceAssignment, CS> + 'static,
    R: WorkerRuntime<ReducerTask<P, S, SD, WR, CS>>,
    SD: ShutdownSignal + Send + 'static,
    WR: WorkReceiver<P::ReduceAssignment, CS> + 'static,
    CS: CompletionSender + 'static,
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
