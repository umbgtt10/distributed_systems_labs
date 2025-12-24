use crate::mapper::Mapper;
use crate::socket_completion_signaling::CompletionSender;
use crate::socket_work_channel::SocketWorkChannel;
use crate::socket_work_distributor::SocketWorkDistributor;
use map_reduce_core::map_reduce_problem::MapReduceProblem;
use map_reduce_core::shutdown_signal::ShutdownSignal;
use map_reduce_core::state_access::StateAccess;
use map_reduce_core::worker_pool_factory::{FaultConfig, WorkerPoolFactory};
use map_reduce_core::worker_runtime::WorkerRuntime;
use std::marker::PhantomData;

/// Factory for creating mapper worker pools in the thread-socket implementation
pub struct SocketMapperFactory<P, S, R, SD>
where
    P: MapReduceProblem,
    S: StateAccess,
    R: WorkerRuntime,
    SD: ShutdownSignal,
{
    state: S,
    shutdown_signal: SD,
    base_port: u16,
    _phantom: PhantomData<(P, R)>,
}

impl<P, S, R, SD> SocketMapperFactory<P, S, R, SD>
where
    P: MapReduceProblem,
    S: StateAccess,
    R: WorkerRuntime,
    SD: ShutdownSignal,
{
    pub fn new(state: S, shutdown_signal: SD, base_port: u16) -> Self {
        Self {
            state,
            shutdown_signal,
            base_port,
            _phantom: PhantomData,
        }
    }
}

impl<P, S, R, SD> SocketMapperFactory<P, S, R, SD>
where
    P: MapReduceProblem + 'static + Sync,
    P::MapAssignment: serde::Serialize + for<'de> serde::Deserialize<'de>,
    S: StateAccess,
    R: WorkerRuntime + Sync,
    SD: ShutdownSignal + Sync,
{
    /// Create a pool of mappers and a work distributor with the given timeout.
    /// This is a convenience method that combines create_pool_and_factory with SocketWorkDistributor creation.
    pub fn create_pool_and_distributor(
        &self,
        pool_size: usize,
        fault_config: FaultConfig,
        timeout_ms: u64,
    ) -> (
        Vec<Mapper<P, S, SocketWorkChannel<P::MapAssignment, CompletionSender>, R, SD>>,
        SocketWorkDistributor<
            Mapper<P, S, SocketWorkChannel<P::MapAssignment, CompletionSender>, R, SD>,
            Box<
                dyn FnMut(
                        usize,
                    ) -> Mapper<
                        P,
                        S,
                        SocketWorkChannel<P::MapAssignment, CompletionSender>,
                        R,
                        SD,
                    > + Send,
            >,
        >,
    ) {
        let (workers, factory) = self.create_pool_and_factory(pool_size, fault_config);
        let distributor = SocketWorkDistributor::with_timeout(factory, timeout_ms);
        (workers, distributor)
    }
}

impl<P, S, R, SD> WorkerPoolFactory for SocketMapperFactory<P, S, R, SD>
where
    P: MapReduceProblem + 'static + Sync,
    P::MapAssignment: serde::Serialize + for<'de> serde::Deserialize<'de>,
    S: StateAccess,
    R: WorkerRuntime + Sync,
    SD: ShutdownSignal + Sync,
{
    type Worker = Mapper<P, S, SocketWorkChannel<P::MapAssignment, CompletionSender>, R, SD>;

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
        let mut port = self.base_port;
        for worker_id in 0..pool_size {
            let (work_channel, work_rx) = SocketWorkChannel::create_pair(port);
            port += 1;
            let worker = Mapper::new(
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
        let mut next_port = port;
        let factory = Box::new(move |worker_id: usize| -> Self::Worker {
            let (work_channel, work_rx) = SocketWorkChannel::create_pair(next_port);
            next_port += 1;
            Mapper::new(
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
