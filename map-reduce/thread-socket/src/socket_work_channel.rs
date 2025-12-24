use map_reduce_core::work_channel::WorkChannel;
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::marker::PhantomData;
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::thread;

/// Socket-based work channel
#[derive(Clone)]
pub struct SocketWorkChannel<A, C> {
    addr: Arc<String>,
    _phantom: PhantomData<(A, C)>,
}

impl<A, C> SocketWorkChannel<A, C> {
    pub fn create_pair(port: u16) -> (Self, SocketWorkReceiver<A, C>) {
        let addr = format!("127.0.0.1:{}", port);
        let channel = Self {
            addr: Arc::new(addr.clone()),
            _phantom: PhantomData,
        };
        let receiver = SocketWorkReceiver {
            listener: Arc::new(TcpListener::bind(&addr).expect("Failed to bind")),
            _phantom: PhantomData,
        };
        (channel, receiver)
    }
}

impl<A, C> WorkChannel<A, C> for SocketWorkChannel<A, C>
where
    A: Clone + Send + Serialize + 'static,
    C: Clone + Send + Serialize + 'static,
{
    fn send_work(&self, assignment: A, completion: C) {
        let addr = self.addr.clone();
        thread::spawn(move || {
            if let Ok(mut stream) = TcpStream::connect(addr.as_str()) {
                let message = (assignment, completion);
                if let Ok(serialized) = serde_json::to_vec(&message) {
                    let len = serialized.len() as u32;
                    let _ = stream.write_all(&len.to_be_bytes());
                    let _ = stream.write_all(&serialized);
                }
            }
        });
    }
}

/// Socket-based work receiver
pub struct SocketWorkReceiver<A, C> {
    listener: Arc<TcpListener>,
    _phantom: PhantomData<(A, C)>,
}

impl<A, C> SocketWorkReceiver<A, C>
where
    A: for<'de> Deserialize<'de>,
    C: for<'de> Deserialize<'de>,
{
    pub fn recv(&self) -> Option<(A, C)> {
        if let Ok((mut stream, _)) = self.listener.accept() {
            let mut len_bytes = [0u8; 4];
            if stream.read_exact(&mut len_bytes).is_ok() {
                let len = u32::from_be_bytes(len_bytes) as usize;
                let mut buffer = vec![0u8; len];
                if stream.read_exact(&mut buffer).is_ok() {
                    if let Ok(message) = serde_json::from_slice(&buffer) {
                        return Some(message);
                    }
                }
            }
        }
        None
    }

    pub fn set_nonblocking(&self, nonblocking: bool) -> std::io::Result<()> {
        self.listener.set_nonblocking(nonblocking)
    }
}
