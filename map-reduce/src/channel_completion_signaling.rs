use crate::completion_signaling::CompletionSignaling;
use tokio::sync::mpsc::{self, Sender};
use tokio_stream::wrappers::ReceiverStream;
use tokio_stream::{StreamExt, StreamMap};

/// Channel-based completion signaling using tokio mpsc and StreamMap
pub struct ChannelCompletionSignaling {
    completion_txs: Vec<Sender<usize>>,
    completion_streams: StreamMap<usize, ReceiverStream<usize>>,
}

impl CompletionSignaling for ChannelCompletionSignaling {
    type Token = Sender<usize>;

    fn setup(num_workers: usize) -> Self {
        let mut completion_txs = Vec::new();
        let mut completion_streams = StreamMap::new();

        for worker_idx in 0..num_workers {
            let (tx, rx) = mpsc::channel::<usize>(10);
            completion_txs.push(tx);
            completion_streams.insert(worker_idx, ReceiverStream::new(rx));
        }

        Self {
            completion_txs,
            completion_streams,
        }
    }

    fn get_token(&self, worker_id: usize) -> Self::Token {
        self.completion_txs[worker_id].clone()
    }

    async fn wait_next(&mut self) -> Option<usize> {
        self.completion_streams
            .next()
            .await
            .map(|(_stream_idx, worker_id)| worker_id)
    }
}
