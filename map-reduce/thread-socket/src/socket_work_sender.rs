// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::socket_work_receiver::SocketWorkReceiver;
use map_reduce_core::work_sender::WorkSender;
use map_reduce_core::worker_message::WorkerMessage;
use serde::Serialize;
use std::io::Write;
use std::marker::PhantomData;
use std::sync::Arc;
use std::thread;
use tokio::net::TcpListener;

/// Socket-based work channel
#[derive(Clone)]
pub struct SocketWorkSender<A, C> {
    addr: Arc<String>,
    _phantom: PhantomData<(A, C)>,
}

impl<A, C> SocketWorkSender<A, C> {
    pub fn create_pair(port: u16) -> (Self, SocketWorkReceiver<A, C>) {
        let addr = format!("127.0.0.1:{}", port);
        let std_listener = std::net::TcpListener::bind(&addr).expect("Failed to bind");
        std_listener
            .set_nonblocking(true)
            .expect("Failed to set nonblocking");
        let actual_addr = std_listener
            .local_addr()
            .expect("Failed to get local address");

        let listener = TcpListener::from_std(std_listener).expect("Failed to convert listener");

        let channel = Self {
            addr: Arc::new(actual_addr.to_string()),
            _phantom: PhantomData,
        };
        let receiver = SocketWorkReceiver {
            listener: Arc::new(listener),
            _phantom: PhantomData,
        };
        (channel, receiver)
    }
}

impl<A, C> WorkSender<A, C> for SocketWorkSender<A, C>
where
    A: Clone + Send + Serialize + 'static,
    C: Clone + Send + Serialize + 'static,
{
    fn initialize(&self, token: C) {
        let addr = self.addr.clone();
        thread::spawn(move || {
            if let Ok(mut stream) = std::net::TcpStream::connect(addr.as_str()) {
                let message = WorkerMessage::<A, C>::Initialize(token);
                if let Ok(serialized) = serde_json::to_vec(&message) {
                    let len = serialized.len() as u32;
                    let _ = stream.write_all(&len.to_be_bytes());
                    let _ = stream.write_all(&serialized);
                }
            }
        });
    }

    fn send_work(&self, assignment: A, completion: C) {
        let addr = self.addr.clone();
        thread::spawn(move || {
            if let Ok(mut stream) = std::net::TcpStream::connect(addr.as_str()) {
                let message = WorkerMessage::Work(assignment, completion);
                if let Ok(serialized) = serde_json::to_vec(&message) {
                    let len = serialized.len() as u32;
                    let _ = stream.write_all(&len.to_be_bytes());
                    let _ = stream.write_all(&serialized);
                }
            }
        });
    }
}

