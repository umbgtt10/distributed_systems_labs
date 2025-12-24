use crate::rpc_completion_signaling::RpcCompletionToken;
use crate::rpc_work_channel::{RpcWorkChannel, RpcWorkReceiver};
use map_reduce_core::map_reduce_job::MapReduceJob;
use map_reduce_core::mapper::MapperTask;
use map_reduce_core::shutdown_signal::ShutdownSignal;
use map_reduce_core::state_access::StateAccess;
use map_reduce_core::worker_factory::WorkerFactory;
use map_reduce_core::worker_runtime::WorkerRuntime;
use serde::{Deserialize, Serialize};

pub type Mapper<P, S, W, R, SD> = map_reduce_core::mapper::Mapper<
    P,
    S,
    W,
    R,
    SD,
    RpcWorkReceiver<<P as MapReduceJob>::MapAssignment, RpcCompletionToken>,
    RpcCompletionToken,
>;

pub struct MapperFactory<P, S, R, SD> {
    state: S,
    shutdown: SD,
    failure_prob: u32,
    straggler_prob: u32,
    straggler_delay: u64,
    _phantom: std::marker::PhantomData<(P, R)>,
}

impl<P, S, R, SD> MapperFactory<P, S, R, SD> {
    pub fn new(
        state: S,
        shutdown: SD,
        failure_prob: u32,
        straggler_prob: u32,
        straggler_delay: u64,
    ) -> Self {
        Self {
            state,
            shutdown,
            failure_prob,
            straggler_prob,
            straggler_delay,
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<P, S, R, SD>
    WorkerFactory<
        Mapper<P, S, RpcWorkChannel<<P as MapReduceJob>::MapAssignment, RpcCompletionToken>, R, SD>,
    > for MapperFactory<P, S, R, SD>
where
    P: MapReduceJob + 'static,
    S: StateAccess + Clone + Send + Sync + Serialize + for<'de> Deserialize<'de> + 'static,
    SD: ShutdownSignal + Clone + Send + Sync + Serialize + for<'de> Deserialize<'de> + 'static,
    P::MapAssignment: Send + Clone + Serialize + for<'de> Deserialize<'de> + 'static,
    R: WorkerRuntime<
            MapperTask<
                P,
                S,
                SD,
                RpcWorkReceiver<<P as MapReduceJob>::MapAssignment, RpcCompletionToken>,
                RpcCompletionToken,
            >,
        > + Clone
        + Send
        + Sync
        + 'static,
{
    fn create_worker(
        &mut self,
        id: usize,
    ) -> Mapper<P, S, RpcWorkChannel<<P as MapReduceJob>::MapAssignment, RpcCompletionToken>, R, SD>
    {
        // Pick a random port between 10000 and 60000
        let port = rand::random::<u16>() % 50000 + 10000;
        let addr = format!("127.0.0.1:{}", port).parse().unwrap();

        let work_channel = RpcWorkChannel::new(addr);
        let work_rx = RpcWorkReceiver::new(port);

        map_reduce_core::mapper::Mapper::new(
            id,
            self.state.clone(),
            self.shutdown.clone(),
            work_rx,
            work_channel,
            self.failure_prob,
            self.straggler_prob,
            self.straggler_delay,
        )
    }
}
