use async_trait::async_trait;
use map_reduce_core::status_sender::StatusSender;
use tokio::sync::mpsc;

#[derive(Clone)]
pub struct ChannelStatusSender {
    pub tx: mpsc::Sender<Result<usize, ()>>,
}

#[async_trait]
impl StatusSender for ChannelStatusSender {
    async fn register(&self, _worker_id: usize) -> bool {
        true
    }

    async fn send(&self, result: Result<usize, ()>) -> bool {
        self.tx.send(result).await.is_ok()
    }
}
