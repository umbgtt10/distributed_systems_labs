use crate::socket_status_sender::SocketStatusSender;
use crate::socket_work_receiver::SocketWorkReceiver;
use crate::socket_work_sender::SocketWorkSender;
use async_trait::async_trait;
use map_reduce_core::map_reduce_job::MapReduceJob;
use map_reduce_core::reducer::ReducerTask;
use map_reduce_core::shutdown_signal::ShutdownSignal;
use map_reduce_core::state_store::StateStore;
use map_reduce_core::worker_factory::WorkerFactory;
use map_reduce_core::worker_runtime::WorkerRuntime;
use std::marker::PhantomData;

pub type Reducer<P, S, W, R, SD> = map_reduce_core::reducer::Reducer<
    P,
    S,
    W,
    R,
    SD,
    SocketWorkReceiver<<P as MapReduceJob>::ReduceAssignment, SocketStatusSender>,
    SocketStatusSender,
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
            SocketWorkSender<<P as MapReduceJob>::ReduceAssignment, SocketStatusSender>,
            R,
            SD,
        >,
    > for ReducerFactory<P, S, R, SD>
where
    P: MapReduceJob + 'static,
    S: StateStore + Clone + Send + Sync + 'static,
    SD: ShutdownSignal + Clone + Send + Sync + 'static,
    P::ReduceAssignment:
        Send + Clone + serde::Serialize + for<'de> serde::Deserialize<'de> + 'static,
    R: WorkerRuntime<
            ReducerTask<
                P,
                S,
                SD,
                SocketWorkReceiver<<P as MapReduceJob>::ReduceAssignment, SocketStatusSender>,
                SocketStatusSender,
            >,
        > + Clone
        + Send
        + Sync
        + 'static,
{
    async fn create_worker(
        &mut self,
        id: usize,
    ) -> Reducer<
        P,
        S,
        SocketWorkSender<<P as MapReduceJob>::ReduceAssignment, SocketStatusSender>,
        R,
        SD,
    > {
        let (work_channel, work_rx) = SocketWorkSender::create_pair(0);

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
