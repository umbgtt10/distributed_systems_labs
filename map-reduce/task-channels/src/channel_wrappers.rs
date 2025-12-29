use async_trait::async_trait;
use map_reduce_core::worker_io::{SynchronizationSender, WorkReceiver, WorkerMessage};
use tokio::sync::mpsc;

pub struct ChannelWorkReceiver<A, C> {
    pub rx: mpsc::Receiver<WorkerMessage<A, C>>,
}

#[async_trait]
impl<A, C> WorkReceiver<A, C> for ChannelWorkReceiver<A, C>
where
    A: Send,
    C: Send,
{
    async fn recv(&mut self) -> Option<WorkerMessage<A, C>> {
        self.rx.recv().await
    }
}

#[derive(Clone)]
pub struct ChannelCompletionSender {
    pub tx: mpsc::Sender<Result<usize, ()>>,
}

#[async_trait]
impl SynchronizationSender for ChannelCompletionSender {
    async fn register(&self, _worker_id: usize) -> bool {
        true
    }

    async fn send(&self, result: Result<usize, ()>) -> bool {
        self.tx.send(result).await.is_ok()
    }
}
