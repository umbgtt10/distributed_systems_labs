use crate::socket_completion_signaling::CompletionSender;
use crate::socket_work_channel::{SocketWorkChannel, SocketWorkReceiver};
use map_reduce_core::map_reduce_problem::MapReduceProblem;
use map_reduce_core::shutdown_signal::ShutdownSignal;
use map_reduce_core::state_access::StateAccess;
use map_reduce_core::worker::WorkerFactory;
use map_reduce_core::worker_runtime::WorkerRuntime;

pub type Reducer<P, S, W, R, SD> = map_reduce_core::reducer::Reducer<
    P,
    S,
    W,
    R,
    SD,
    SocketWorkReceiver<<P as MapReduceProblem>::ReduceAssignment, CompletionSender>,
    CompletionSender,
>;

pub struct ReducerFactory<P, S, R, SD> {
    state: S,
    shutdown: SD,
    failure_prob: u32,
    straggler_prob: u32,
    straggler_delay: u64,
    _phantom: std::marker::PhantomData<(P, R)>,
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
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<P, S, R, SD>
    WorkerFactory<
        Reducer<
            P,
            S,
            SocketWorkChannel<<P as MapReduceProblem>::ReduceAssignment, CompletionSender>,
            R,
            SD,
        >,
    > for ReducerFactory<P, S, R, SD>
where
    P: MapReduceProblem + 'static,
    S: StateAccess + Clone + Send + Sync + 'static,
    R: WorkerRuntime + Clone + Send + Sync + 'static,
    SD: ShutdownSignal + Clone + Send + Sync + 'static,
    P::ReduceAssignment:
        Send + Clone + serde::Serialize + for<'de> serde::Deserialize<'de> + 'static,
{
    fn create_worker(
        &mut self,
        id: usize,
    ) -> Reducer<
        P,
        S,
        SocketWorkChannel<<P as MapReduceProblem>::ReduceAssignment, CompletionSender>,
        R,
        SD,
    > {
        let (work_channel, work_rx) = SocketWorkChannel::create_pair(0);

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
