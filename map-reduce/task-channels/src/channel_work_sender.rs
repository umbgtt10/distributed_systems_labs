// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use map_reduce_core::work_sender::WorkSender;
use map_reduce_core::worker_message::WorkerMessage;
use tokio::sync::mpsc::{self, Receiver, Sender};
use tokio::task;

/// Tokio mpsc channel-based work channel
#[derive(Clone)]
pub struct ChannelWorkSender<A, C> {
    tx: Sender<WorkerMessage<A, C>>,
}

impl<A, C> ChannelWorkSender<A, C> {
    pub fn create_pair(buffer: usize) -> (Self, Receiver<WorkerMessage<A, C>>) {
        let (tx, rx) = mpsc::channel(buffer);
        (Self { tx }, rx)
    }
}

impl<A, C> WorkSender<A, C> for ChannelWorkSender<A, C>
where
    A: Clone + Send + 'static,
    C: Clone + Send + 'static,
{
    fn initialize(&self, token: C) {
        let tx = self.tx.clone();
        task::spawn(async move {
            let _ = tx.send(WorkerMessage::Initialize(token)).await;
        });
    }

    fn send_work(&self, assignment: A, completion: C) {
        let tx = self.tx.clone();
        task::spawn(async move {
            let _ = tx.send(WorkerMessage::Work(assignment, completion)).await;
        });
    }
}

