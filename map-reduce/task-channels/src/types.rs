use crate::CompletionMessage;
use crate::LocalStateAccess;
use crate::MapReduceProblem;
use crate::Mapper;
use crate::MpscWorkChannel;
use crate::Reducer;
use crate::TokenShutdownSignal;
use crate::TokioRuntime;
use crate::WordSearchProblem;
use tokio::sync::mpsc::Sender;

pub type MapperType = Mapper<
    WordSearchProblem,
    LocalStateAccess,
    MpscWorkChannel<
        <WordSearchProblem as MapReduceProblem>::MapAssignment,
        Sender<CompletionMessage>,
    >,
    TokioRuntime,
    TokenShutdownSignal,
>;

pub type ReducerType = Reducer<
    WordSearchProblem,
    LocalStateAccess,
    MpscWorkChannel<
        <WordSearchProblem as MapReduceProblem>::ReduceAssignment,
        Sender<CompletionMessage>,
    >,
    TokioRuntime,
    TokenShutdownSignal,
>;
