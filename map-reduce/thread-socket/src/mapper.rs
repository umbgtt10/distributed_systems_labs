use map_reduce_core::map_reduce_problem::MapReduceProblem;
use map_reduce_core::shutdown_signal::ShutdownSignal;
use map_reduce_core::state_access::StateAccess;
use map_reduce_core::work_channel::WorkChannel;
use map_reduce_core::worker::Worker;
use map_reduce_core::worker_runtime::WorkerRuntime;
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::marker::PhantomData;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::thread;
use std::time::Duration;

use crate::socket_completion_signaling::CompletionSender;
use crate::socket_work_channel::SocketWorkReceiver;

/// Mapper worker using threads
pub struct Mapper<P, S, W, R, SD>
where
    P: MapReduceProblem,
    S: StateAccess,
    W: WorkChannel<P::MapAssignment, CompletionSender>,
    R: WorkerRuntime,
    SD: ShutdownSignal,
{
    work_channel: W,
    task_handle: R::Handle,
    _phantom: PhantomData<(P, S, SD)>,
}

impl<P, S, W, R, SD> Mapper<P, S, W, R, SD>
where
    P: MapReduceProblem,
    S: StateAccess + Send + Sync + 'static,
    W: WorkChannel<P::MapAssignment, CompletionSender> + 'static,
    R: WorkerRuntime,
    SD: ShutdownSignal + Send + 'static,
    P::MapAssignment: for<'de> Deserialize<'de> + Serialize,
{
    pub fn new(
        id: usize,
        state: S,
        shutdown_signal: SD,
        work_rx: SocketWorkReceiver<P::MapAssignment, CompletionSender>,
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

    async fn run(
        id: usize,
        state: S,
        shutdown_signal: SD,
        work_rx: SocketWorkReceiver<P::MapAssignment, CompletionSender>,
        failure_probability: u32,
        straggler_probability: u32,
        straggler_delay_ms: u64,
    ) {
        let mut last_completion_sender: Option<CompletionSender> = None;
        loop {
            // Check for shutdown
            if shutdown_signal.is_cancelled() {
                if let Some(sender) = last_completion_sender {
                    // Send completion to unblock distributor
                    sender.send(Ok(id));
                }
                break;
            }

            // Try to receive work with timeout
            if let Some((assignment, completion_sender)) = work_rx.recv() {
                last_completion_sender = Some(completion_sender.clone());
                // Simulate random failure
                if failure_probability > 0 {
                    let random_value = rand::rng().random_range(0..100);
                    if random_value < failure_probability {
                        eprintln!("💥 Mapper {} simulated failure!", id);
                        completion_sender.send(Err(()));
                        continue;
                    }
                }

                // Simulate straggler
                if straggler_probability > 0 {
                    let random_value = rand::rng().random_range(0..100);
                    if random_value < straggler_probability {
                        let delay = rand::rng().random_range(1..=straggler_delay_ms);
                        if id % 5 == 0 {
                            eprintln!("🐌 Mapper {} is a straggler! Delaying {}ms", id, delay);
                        }
                        thread::sleep(Duration::from_millis(delay));
                    }
                }

                // Execute work with error handling
                let result = catch_unwind(AssertUnwindSafe(|| {
                    P::map_work(&assignment, &state);
                }));

                match result {
                    Ok(_) => {
                        if id % 5 == 0 {
                            println!("Mapper {} finished work", id);
                        }
                        completion_sender.send(Ok(id));
                    }
                    Err(_) => {
                        eprintln!("❌ Mapper {} panicked during work!", id);
                        completion_sender.send(Err(()));
                    }
                }
            } else {
                // No work available, check shutdown more frequently
                thread::sleep(Duration::from_millis(50));
            }
        }
    }
}

impl<P, S, W, R, SD> Worker for Mapper<P, S, W, R, SD>
where
    P: MapReduceProblem,
    S: StateAccess,
    W: WorkChannel<P::MapAssignment, CompletionSender>,
    R: WorkerRuntime,
    SD: ShutdownSignal,
{
    type Assignment = P::MapAssignment;
    type Completion = CompletionSender;
    type Error = R::Error;

    fn send_work(&self, assignment: Self::Assignment, complete_tx: Self::Completion) {
        self.work_channel.send_work(assignment, complete_tx);
    }

    async fn wait(self) -> Result<(), Self::Error> {
        R::join(self.task_handle).await
    }
}
