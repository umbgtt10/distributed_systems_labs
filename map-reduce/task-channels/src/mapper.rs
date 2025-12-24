use crate::channel_wrappers::{ChannelCompletionSender, ChannelWorkReceiver};
use map_reduce_core::map_reduce_problem::MapReduceProblem;

pub type Mapper<P, S, W, R, SD> = map_reduce_core::mapper::Mapper<
    P,
    S,
    W,
    R,
    SD,
    ChannelWorkReceiver<<P as MapReduceProblem>::MapAssignment, ChannelCompletionSender>,
    ChannelCompletionSender,
>;
