// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::channel_status_sender::ChannelStatusSender;
use crate::channel_work_receiver::ChannelWorkReceiver;
use crate::channel_work_sender::ChannelWorkSender;
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
    ChannelWorkReceiver<<P as MapReduceJob>::ReduceAssignment, ChannelStatusSender>,
    ChannelStatusSender,
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
            ChannelWorkSender<<P as MapReduceJob>::ReduceAssignment, ChannelStatusSender>,
            R,
            SD,
        >,
    > for ReducerFactory<P, S, R, SD>
where
    P: MapReduceJob + 'static,
    S: StateStore + Clone + Send + Sync + 'static,
    SD: ShutdownSignal + Clone + Send + Sync + 'static,
    P::ReduceAssignment: Send + Clone + 'static,
    R: WorkerRuntime<
            ReducerTask<
                P,
                S,
                SD,
                ChannelWorkReceiver<<P as MapReduceJob>::ReduceAssignment, ChannelStatusSender>,
                ChannelStatusSender,
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
        ChannelWorkSender<<P as MapReduceJob>::ReduceAssignment, ChannelStatusSender>,
        R,
        SD,
    > {
        let (work_channel, work_rx) = ChannelWorkSender::<
            <P as MapReduceJob>::ReduceAssignment,
            ChannelStatusSender,
        >::create_pair(10);
        let wrapped_rx = ChannelWorkReceiver { rx: work_rx };

        map_reduce_core::reducer::Reducer::new(
            id,
            self.state.clone(),
            self.shutdown.clone(),
            wrapped_rx,
            work_channel,
            self.failure_prob,
            self.straggler_prob,
            self.straggler_delay,
        )
    }
}

