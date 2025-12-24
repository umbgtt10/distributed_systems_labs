use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

/// Completion message type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CompletionMessage {
    Success(usize),
    Failure(usize),
}

/// Socket-based completion signaling
pub struct SocketCompletionSignaling {
    base_port: u16,
    listeners: Arc<Mutex<HashMap<usize, Arc<TcpListener>>>>,
}

impl SocketCompletionSignaling {
    pub fn new(base_port: u16, num_workers: usize) -> Self {
        let mut listeners = HashMap::new();
        for i in 0..num_workers {
            let port = base_port + i as u16;
            let addr = format!("127.0.0.1:{}", port);
            let listener = TcpListener::bind(&addr).expect("Failed to bind completion listener");
            listener
                .set_nonblocking(true)
                .expect("Failed to set nonblocking");
            listeners.insert(i, Arc::new(listener));
        }
        Self {
            base_port,
            listeners: Arc::new(Mutex::new(listeners)),
        }
    }

    pub fn get_sender(&self, worker_id: usize) -> CompletionSender {
        CompletionSender {
            port: self.base_port + worker_id as u16,
        }
    }

    pub fn wait_next(&self) -> Option<Result<usize, usize>> {
        loop {
            {
                let listeners = self.listeners.lock().unwrap();
                for (_worker_id, listener) in listeners.iter() {
                    match listener.accept() {
                        Ok((mut stream, _)) => {
                            drop(listeners);
                            let mut len_bytes = [0u8; 4];
                            if stream.read_exact(&mut len_bytes).is_ok() {
                                let len = u32::from_be_bytes(len_bytes) as usize;
                                let mut buffer = vec![0u8; len];
                                if stream.read_exact(&mut buffer).is_ok() {
                                    if let Ok(msg) =
                                        serde_json::from_slice::<CompletionMessage>(&buffer)
                                    {
                                        return Some(match msg {
                                            CompletionMessage::Success(id) => Ok(id),
                                            CompletionMessage::Failure(id) => Err(id),
                                        });
                                    }
                                }
                            }
                            return None;
                        }
                        Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            // No data available on this listener, continue
                        }
                        Err(_) => {
                            // Error occurred, skip this listener
                        }
                    }
                }
            }
            thread::sleep(Duration::from_millis(10));
        }
    }

    pub fn drain_worker(&self, worker_id: usize) {
        if let Some(listener) = self.listeners.lock().unwrap().get(&worker_id) {
            // Try to drain any pending messages with a short timeout
            let start = std::time::Instant::now();
            while start.elapsed() < Duration::from_millis(50) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        // Read and discard the message
                        let mut len_bytes = [0u8; 4];
                        if stream.read_exact(&mut len_bytes).is_ok() {
                            let len = u32::from_be_bytes(len_bytes) as usize;
                            let mut buffer = vec![0u8; len];
                            let _ = stream.read_exact(&mut buffer);
                        }
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        break; // No more messages
                    }
                    Err(_) => break,
                }
            }
        }
    }
}

/// Completion sender
#[derive(Clone, Serialize, Deserialize)]
pub struct CompletionSender {
    port: u16,
}

impl CompletionSender {
    pub fn send(&self, result: Result<usize, ()>) {
        let addr = format!("127.0.0.1:{}", self.port);
        let message = match result {
            Ok(id) => CompletionMessage::Success(id),
            Err(_) => CompletionMessage::Failure(0), // Will be filled by receiver
        };
        if let Ok(mut stream) = TcpStream::connect(&addr) {
            if let Ok(serialized) = serde_json::to_vec(&message) {
                let len = serialized.len() as u32;
                let _ = stream.write_all(&len.to_be_bytes());
                let _ = stream.write_all(&serialized);
            }
        }
    }
}

impl From<CompletionSender> for Result<usize, ()> {
    fn from(_sender: CompletionSender) -> Self {
        Ok(0) // Placeholder
    }
}
