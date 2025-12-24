use map_reduce_core::work_channel::WorkChannel;
use tokio::sync::mpsc::{self, Receiver, Sender};
use tokio::task;

/// Tokio mpsc channel-based work channel
#[derive(Clone)]
pub struct MpscWorkChannel<A, C> {
    tx: Sender<(A, C)>,
}

impl<A, C> MpscWorkChannel<A, C> {
    pub fn create_pair(buffer: usize) -> (Self, Receiver<(A, C)>) {
        let (tx, rx) = mpsc::channel(buffer);
        (Self { tx }, rx)
    }
}

impl<A, C> WorkChannel<A, C> for MpscWorkChannel<A, C>
where
    A: Clone + Send + 'static,
    C: Clone + Send + 'static,
{
    fn send_work(&self, assignment: A, completion: C) {
        let tx = self.tx.clone();
        task::spawn(async move {
            let _ = tx.send((assignment, completion)).await;
        });
    }
}
