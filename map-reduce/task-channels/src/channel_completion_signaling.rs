use crate::channel_wrappers::ChannelCompletionSender;
use map_reduce_core::completion_signaling::SynchronizationSignaling;
use tokio::sync::mpsc::{self, Sender};
use tokio_stream::wrappers::ReceiverStream;
use tokio_stream::{StreamExt, StreamMap};

/// Completion message: Ok for success, Err for failure
pub type CompletionMessage = Result<usize, ()>;

/// Channel-based completion signaling using tokio mpsc and StreamMap
pub struct ChannelCompletionSignaling {
    completion_txs: Vec<Sender<CompletionMessage>>,
    completion_streams: StreamMap<usize, ReceiverStream<CompletionMessage>>,
}

impl SynchronizationSignaling for ChannelCompletionSignaling {
    type Token = ChannelCompletionSender;

    fn setup(num_workers: usize) -> Self {
        let mut completion_txs = Vec::new();
        let mut completion_streams = StreamMap::new();

        for worker_idx in 0..num_workers {
            let (tx, rx) = mpsc::channel::<CompletionMessage>(10);
            completion_txs.push(tx);
            completion_streams.insert(worker_idx, ReceiverStream::new(rx));
        }

        Self {
            completion_txs,
            completion_streams,
        }
    }

    fn get_token(&self, worker_id: usize) -> Self::Token {
        ChannelCompletionSender {
            tx: self.completion_txs[worker_id].clone(),
        }
    }

    async fn wait_for_worker_ready(&self, _worker_id: usize) -> bool {
        true
    }

    async fn reset_worker(&mut self, worker_id: usize) -> Self::Token {
        // Remove old stream
        if let Some(mut stream) = self.completion_streams.remove(&worker_id) {
            // Drain pending messages
            while let Ok(Some(_)) =
                tokio::time::timeout(tokio::time::Duration::from_millis(10), stream.next()).await
            {
                // Discard
            }
        }

        // Create new channel
        let (tx, rx) = mpsc::channel::<CompletionMessage>(10);

        // Update tx and stream
        self.completion_txs[worker_id] = tx;
        self.completion_streams
            .insert(worker_id, ReceiverStream::new(rx));

        self.get_token(worker_id)
    }

    async fn wait_next(&mut self) -> Option<Result<usize, usize>> {
        self.completion_streams
            .next()
            .await
            .map(|(stream_idx, msg)| {
                match msg {
                    Ok(worker_id) => Ok(worker_id),
                    Err(_) => Err(stream_idx), // stream_idx is the failed worker_id
                }
            })
    }
}
