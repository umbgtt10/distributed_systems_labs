use tokio::sync::mpsc;

/// Trait for abstracting work assignment channels
/// Different implementations for mpsc, sockets, RPC, etc.
pub trait WorkChannel<A, C>: Clone + Send + 'static {
    /// Send work assignment with completion token
    fn send_work(&self, assignment: A, completion: C);
}

/// Tokio mpsc channel-based work channel
#[derive(Clone)]
pub struct MpscWorkChannel<A, C> {
    tx: mpsc::Sender<(A, C)>,
}

impl<A, C> MpscWorkChannel<A, C> {
    pub fn create_pair(buffer: usize) -> (Self, mpsc::Receiver<(A, C)>) {
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
        tokio::spawn(async move {
            let _ = tx.send((assignment, completion)).await;
        });
    }
}
