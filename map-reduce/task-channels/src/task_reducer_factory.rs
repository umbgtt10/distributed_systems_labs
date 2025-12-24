use crate::channel_completion_signaling::{ChannelCompletionSignaling, CompletionMessage};
use crate::mpsc_work_channel::MpscWorkChannel;
use crate::reducer::Reducer;
use crate::task_work_distributor::TaskWorkDistributor;
use map_reduce_core::map_reduce_problem::MapReduceProblem;
use map_reduce_core::shutdown_signal::ShutdownSignal;
use map_reduce_core::state_access::StateAccess;
use map_reduce_core::worker_pool_factory::{FaultConfig, WorkerPoolFactory};
use map_reduce_core::worker_runtime::WorkerRuntime;
use std::marker::PhantomData;
use tokio::sync::mpsc::Sender;

/// Factory for creating reducer worker pools in the task-channels implementation
pub struct TaskReducerFactory<P, S, R, SD>
where
    P: MapReduceProblem,
    S: StateAccess,
    R: WorkerRuntime,
    SD: ShutdownSignal,
{
    state: S,
    shutdown_signal: SD,
    _phantom: PhantomData<(P, R)>,
}

impl<P, S, R, SD> TaskReducerFactory<P, S, R, SD>
where
    P: MapReduceProblem,
    S: StateAccess,
    R: WorkerRuntime,
    SD: ShutdownSignal,
{
    pub fn new(state: S, shutdown_signal: SD) -> Self {
        Self {
            state,
            shutdown_signal,
            _phantom: PhantomData,
        }
    }
}

impl<P, S, R, SD> TaskReducerFactory<P, S, R, SD>
where
    P: MapReduceProblem + 'static + Sync,
    S: StateAccess,
    R: WorkerRuntime + Sync,
    SD: ShutdownSignal + Sync,
{
    /// Create a pool of reducers and a work distributor with the given timeout.
    /// This is a convenience method that combines create_pool_and_factory with TaskWorkDistributor creation.
    pub fn create_pool_and_distributor(
        &self,
        pool_size: usize,
        fault_config: FaultConfig,
        timeout_ms: u64,
    ) -> (
        Vec<Reducer<P, S, MpscWorkChannel<P::ReduceAssignment, Sender<CompletionMessage>>, R, SD>>,
        TaskWorkDistributor<
            Reducer<P, S, MpscWorkChannel<P::ReduceAssignment, Sender<CompletionMessage>>, R, SD>,
            ChannelCompletionSignaling,
            Box<
                dyn FnMut(
                        usize,
                    ) -> Reducer<
                        P,
                        S,
                        MpscWorkChannel<P::ReduceAssignment, Sender<CompletionMessage>>,
                        R,
                        SD,
                    > + Send,
            >,
        >,
    ) {
        let (workers, factory) = self.create_pool_and_factory(pool_size, fault_config);
        let distributor = TaskWorkDistributor::new(factory, timeout_ms);
        (workers, distributor)
    }
}

impl<P, S, R, SD> WorkerPoolFactory for TaskReducerFactory<P, S, R, SD>
where
    P: MapReduceProblem + 'static + Sync,
    S: StateAccess,
    R: WorkerRuntime + Sync,
    SD: ShutdownSignal + Sync,
{
    type Worker =
        Reducer<P, S, MpscWorkChannel<P::ReduceAssignment, Sender<CompletionMessage>>, R, SD>;

    fn create_pool_and_factory(
        &self,
        pool_size: usize,
        fault_config: FaultConfig,
    ) -> (
        Vec<Self::Worker>,
        Box<dyn FnMut(usize) -> Self::Worker + Send>,
    ) {
        // Create initial pool
        let mut workers = Vec::new();
        for worker_id in 0..pool_size {
            let (work_channel, work_rx) = MpscWorkChannel::create_pair(10);
            let worker = Reducer::new(
                worker_id,
                self.state.clone(),
                self.shutdown_signal.clone(),
                work_rx,
                work_channel,
                fault_config.failure_probability,
                fault_config.straggler_probability,
                fault_config.straggler_delay_ms,
            );
            workers.push(worker);
        }

        // Create factory closure
        let state = self.state.clone();
        let shutdown_signal = self.shutdown_signal.clone();
        let factory = Box::new(move |worker_id: usize| -> Self::Worker {
            let (work_channel, work_rx) = MpscWorkChannel::create_pair(10);
            Reducer::new(
                worker_id,
                state.clone(),
                shutdown_signal.clone(),
                work_rx,
                work_channel,
                fault_config.failure_probability,
                fault_config.straggler_probability,
                fault_config.straggler_delay_ms,
            )
        });

        (workers, factory)
    }
}
