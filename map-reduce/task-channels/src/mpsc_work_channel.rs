use map_reduce_core::work_channel::WorkDistributor;
use map_reduce_core::worker_io::WorkerMessage;
use tokio::sync::mpsc::{self, Receiver, Sender};
use tokio::task;

/// Tokio mpsc channel-based work channel
#[derive(Clone)]
pub struct MpscWorkChannel<A, C> {
    tx: Sender<WorkerMessage<A, C>>,
}

impl<A, C> MpscWorkChannel<A, C> {
    pub fn create_pair(buffer: usize) -> (Self, Receiver<WorkerMessage<A, C>>) {
        let (tx, rx) = mpsc::channel(buffer);
        (Self { tx }, rx)
    }
}

impl<A, C> WorkDistributor<A, C> for MpscWorkChannel<A, C>
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
