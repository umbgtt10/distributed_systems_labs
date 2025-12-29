use async_trait::async_trait;
use map_reduce_core::work_receiver::WorkReceiver;
use map_reduce_core::worker_message::WorkerMessage;
use serde::Deserialize;
use std::marker::PhantomData;
use std::sync::Arc;
use tokio::io::AsyncReadExt;
use tokio::net::TcpListener;

/// Socket-based work receiver
pub struct SocketWorkReceiver<A, C> {
    pub listener: Arc<TcpListener>,
    pub _phantom: PhantomData<(A, C)>,
}

#[async_trait]
impl<A, C> WorkReceiver<A, C> for SocketWorkReceiver<A, C>
where
    A: for<'de> Deserialize<'de> + Send,
    C: for<'de> Deserialize<'de> + Send,
{
    async fn recv(&mut self) -> Option<WorkerMessage<A, C>> {
        if let Ok((mut stream, _)) = self.listener.accept().await {
            let mut len_bytes = [0u8; 4];
            if stream.read_exact(&mut len_bytes).await.is_ok() {
                let len = u32::from_be_bytes(len_bytes) as usize;
                let mut buffer = vec![0u8; len];
                if stream.read_exact(&mut buffer).await.is_ok() {
                    if let Ok(message) = serde_json::from_slice(&buffer) {
                        return Some(message);
                    }
                }
            }
        }
        None
    }
}
