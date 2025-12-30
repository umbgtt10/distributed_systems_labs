// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::channel_status_sender::ChannelStatusSender;
use map_reduce_core::worker_synchronization::WorkerSynchronization;
use tokio::sync::mpsc::{self, Sender};
use tokio_stream::wrappers::ReceiverStream;
use tokio_stream::{StreamExt, StreamMap};

/// Completion message: Ok for success, Err for failure
pub type CompletionMessage = Result<usize, ()>;

/// Channel-based completion signaling using tokio mpsc and StreamMap
pub struct ChannelWorkerSynchronization {
    completion_txs: Vec<Sender<CompletionMessage>>,
    completion_streams: StreamMap<usize, ReceiverStream<CompletionMessage>>,
}

impl WorkerSynchronization for ChannelWorkerSynchronization {
    type StatusSender = ChannelStatusSender;

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

    fn get_status_sender(&self, worker_id: usize) -> Self::StatusSender {
        ChannelStatusSender {
            tx: self.completion_txs[worker_id].clone(),
        }
    }

    async fn wait_for_worker_ready(&self, _worker_id: usize) -> bool {
        true
    }

    async fn reset_worker(&mut self, worker_id: usize) -> Self::StatusSender {
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

        self.get_status_sender(worker_id)
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

