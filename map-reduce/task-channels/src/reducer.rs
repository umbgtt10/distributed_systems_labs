use crate::channel_wrappers::{ChannelCompletionSender, ChannelWorkReceiver};
use map_reduce_core::map_reduce_problem::MapReduceProblem;

pub type Reducer<P, S, W, R, SD> = map_reduce_core::reducer::Reducer<
    P,
    S,
    W,
    R,
    SD,
    ChannelWorkReceiver<<P as MapReduceProblem>::ReduceAssignment, ChannelCompletionSender>,
    ChannelCompletionSender,
>;
