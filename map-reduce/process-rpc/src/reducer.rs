use crate::grpc_work_sender::GrpcWorkSender;
use crate::{grpc_status_sender::GrpcStatusSender, grpc_work_receiver::GrpcWorkReceiver};
use async_trait::async_trait;
use map_reduce_core::map_reduce_job::MapReduceJob;
use map_reduce_core::reducer::ReducerTask;
use map_reduce_core::shutdown_signal::ShutdownSignal;
use map_reduce_core::state_store::StateStore;
use map_reduce_core::worker_factory::WorkerFactory;
use map_reduce_core::worker_runtime::WorkerRuntime;
use serde::{Deserialize, Serialize};
use std::marker::PhantomData;

pub type Reducer<P, S, W, R, SD> = map_reduce_core::reducer::Reducer<
    P,
    S,
    W,
    R,
    SD,
    GrpcWorkReceiver<<P as MapReduceJob>::ReduceAssignment, GrpcStatusSender>,
    GrpcStatusSender,
>;

pub struct ReducerFactory<P, S, R, SD> {
    state: S,
    shutdown: SD,
    failure_prob: u32,
    straggler_prob: u32,
    straggler_delay: u64,
    _phantom: PhantomData<(P, R)>,
}

impl<P, S, R, SD> ReducerFactory<P, S, R, SD> {
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
        Reducer<
            P,
            S,
            GrpcWorkSender<<P as MapReduceJob>::ReduceAssignment, GrpcStatusSender>,
            R,
            SD,
        >,
    > for ReducerFactory<P, S, R, SD>
where
    P: MapReduceJob + 'static,
    S: StateStore + Clone + Send + Sync + Serialize + for<'de> Deserialize<'de> + 'static,
    SD: ShutdownSignal + Clone + Send + Sync + Serialize + for<'de> Deserialize<'de> + 'static,
    P::ReduceAssignment: Send + Clone + Sync + Serialize + for<'de> Deserialize<'de> + 'static,
    R: WorkerRuntime<
            ReducerTask<
                P,
                S,
                SD,
                GrpcWorkReceiver<<P as MapReduceJob>::ReduceAssignment, GrpcStatusSender>,
                GrpcStatusSender,
            >,
        > + Clone
        + Send
        + Sync
        + 'static,
{
    async fn create_worker(
        &mut self,
        id: usize,
    ) -> Reducer<P, S, GrpcWorkSender<<P as MapReduceJob>::ReduceAssignment, GrpcStatusSender>, R, SD>
    {
        let port = crate::config::REDUCER_BASE_PORT + id as u16;
        let (work_channel, work_rx) = GrpcWorkSender::create_pair(port).await;

        map_reduce_core::reducer::Reducer::new(
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
