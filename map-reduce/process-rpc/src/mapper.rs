use crate::grpc_completion_signaling::GrpcSynchronizationToken;
use crate::grpc_work_channel::{GrpcWorkChannel, GrpcWorkReceiver};
use async_trait::async_trait;
use map_reduce_core::map_reduce_job::MapReduceJob;
use map_reduce_core::mapper::MapperTask;
use map_reduce_core::shutdown_signal::ShutdownSignal;
use map_reduce_core::state_access::StateAccess;
use map_reduce_core::worker_factory::WorkerFactory;
use map_reduce_core::worker_runtime::WorkerRuntime;
use serde::{Deserialize, Serialize};
use std::marker::PhantomData;

pub type Mapper<P, S, W, R, SD> = map_reduce_core::mapper::Mapper<
    P,
    S,
    W,
    R,
    SD,
    GrpcWorkReceiver<<P as MapReduceJob>::MapAssignment, GrpcSynchronizationToken>,
    GrpcSynchronizationToken,
>;

pub struct MapperFactory<P, S, R, SD> {
    state: S,
    shutdown: SD,
    failure_prob: u32,
    straggler_prob: u32,
    straggler_delay: u64,
    _phantom: PhantomData<(P, R)>,
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
            _phantom: PhantomData,
        }
    }
}

#[async_trait]
impl<P, S, R, SD>
    WorkerFactory<
        Mapper<
            P,
            S,
            GrpcWorkChannel<<P as MapReduceJob>::MapAssignment, GrpcSynchronizationToken>,
            R,
            SD,
        >,
    > for MapperFactory<P, S, R, SD>
where
    P: MapReduceJob + 'static,
    S: StateAccess + Clone + Send + Sync + Serialize + for<'de> Deserialize<'de> + 'static,
    SD: ShutdownSignal + Clone + Send + Sync + Serialize + for<'de> Deserialize<'de> + 'static,
    P::MapAssignment: Send + Clone + Sync + Serialize + for<'de> Deserialize<'de> + 'static,
    R: WorkerRuntime<
            MapperTask<
                P,
                S,
                SD,
                GrpcWorkReceiver<<P as MapReduceJob>::MapAssignment, GrpcSynchronizationToken>,
                GrpcSynchronizationToken,
            >,
        > + Clone
        + Send
        + Sync
        + 'static,
{
    async fn create_worker(
        &mut self,
        id: usize,
    ) -> Mapper<
        P,
        S,
        GrpcWorkChannel<<P as MapReduceJob>::MapAssignment, GrpcSynchronizationToken>,
        R,
        SD,
    > {
        let port = crate::config::MAPPER_BASE_PORT + id as u16;
        let (work_channel, work_rx) = GrpcWorkChannel::create_pair(port).await;

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
