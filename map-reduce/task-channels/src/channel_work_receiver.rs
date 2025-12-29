use async_trait::async_trait;
use map_reduce_core::{work_receiver::WorkReceiver, worker_message::WorkerMessage};
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
