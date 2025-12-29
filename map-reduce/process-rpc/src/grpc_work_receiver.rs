use crate::rpc::proto;
use async_trait::async_trait;
use map_reduce_core::work_receiver::WorkReceiver;
use map_reduce_core::worker_message::WorkerMessage;
use proto::work_service_server::{WorkService as WorkServiceTrait, WorkServiceServer};
use proto::{InitializeWorkerRequest, WorkAck, WorkMessage};
use serde::{Deserialize, Serialize};
use std::marker::PhantomData;
use std::sync::Arc;
use tokio::sync::mpsc::Receiver;
use tokio::sync::Mutex;
use tonic::transport::Server;
use tonic::{Request, Response, Status};

type WorkerMessageReceiver<A, C> = Arc<Mutex<Option<Receiver<WorkerMessage<A, C>>>>>;

/// gRPC Work Receiver
/// Receives work assignments from coordinator
#[derive(Serialize, Deserialize)]
pub struct GrpcWorkReceiver<A, C> {
    pub port: u16,
    #[serde(skip, default = "default_rx")]
    pub rx: WorkerMessageReceiver<A, C>,
}

fn default_rx<A, C>() -> WorkerMessageReceiver<A, C> {
    Arc::new(Mutex::new(None))
}

impl<A, C> GrpcWorkReceiver<A, C> {
    // Receivers are now created via GrpcWorkChannel::create_pair()
}

/// gRPC Work Service implementation
struct WorkServiceImpl<A, C> {
    tx: tokio::sync::mpsc::Sender<WorkerMessage<A, C>>,
    _phantom: PhantomData<(A, C)>,
}

impl<A, C> Clone for WorkServiceImpl<A, C> {
    fn clone(&self) -> Self {
        Self {
            tx: self.tx.clone(),
            _phantom: PhantomData,
        }
    }
}

#[tonic::async_trait]
impl<A, C> WorkServiceTrait for WorkServiceImpl<A, C>
where
    A: Send + Sync + for<'de> Deserialize<'de> + 'static,
    C: Send + Sync + for<'de> Deserialize<'de> + 'static,
{
    async fn initialize_worker(
        &self,
        request: Request<InitializeWorkerRequest>,
    ) -> Result<Response<WorkAck>, Status> {
        let msg = request.into_inner();

        let token: C = serde_json::from_str(&msg.synchronization_token_json)
            .map_err(|e| Status::invalid_argument(format!("Invalid token JSON: {}", e)))?;

        self.tx
            .send(WorkerMessage::Initialize(token))
            .await
            .map_err(|_| Status::internal("Failed to queue initialization"))?;

        Ok(Response::new(WorkAck { received: true }))
    }

    async fn receive_work(
        &self,
        request: Request<WorkMessage>,
    ) -> Result<Response<WorkAck>, Status> {
        let msg = request.into_inner();

        let assignment: A = serde_json::from_str(&msg.assignment_json)
            .map_err(|e| Status::invalid_argument(format!("Invalid assignment JSON: {}", e)))?;

        let completion: C = serde_json::from_str(&msg.completion_json)
            .map_err(|e| Status::invalid_argument(format!("Invalid completion JSON: {}", e)))?;

        self.tx
            .send(WorkerMessage::Work(assignment, completion))
            .await
            .map_err(|_| Status::internal("Failed to queue work"))?;

        Ok(Response::new(WorkAck { received: true }))
    }
}

#[async_trait]
impl<A, C> WorkReceiver<A, C> for GrpcWorkReceiver<A, C>
where
    A: Send + Sync + Serialize + for<'de> Deserialize<'de> + 'static,
    C: Send + Sync + Serialize + for<'de> Deserialize<'de> + 'static,
{
    async fn recv(&mut self) -> Option<WorkerMessage<A, C>> {
        let mut rx_guard = self.rx.lock().await;

        if rx_guard.is_none() {
            // Lazy initialization of the server
            let (tx, rx) = tokio::sync::mpsc::channel(10);
            *rx_guard = Some(rx);

            let port = self.port;
            let service = WorkServiceImpl::<A, C> {
                tx,
                _phantom: PhantomData,
            };

            tokio::spawn(async move {
                let addr_str = format!("127.0.0.1:{}", port);
                let socket_addr: std::net::SocketAddr = addr_str.parse().unwrap();

                // Use socket2 to enable SO_REUSEADDR
                let domain = socket2::Domain::for_address(socket_addr);
                let socket = match socket2::Socket::new(
                    domain,
                    socket2::Type::STREAM,
                    Some(socket2::Protocol::TCP),
                ) {
                    Ok(s) => s,
                    Err(e) => {
                        eprintln!("Failed to create socket: {}", e);
                        return;
                    }
                };

                if let Err(e) = socket.set_reuse_address(true) {
                    eprintln!("Failed to set reuse_address: {}", e);
                    return;
                }

                if let Err(e) = socket.bind(&socket_addr.into()) {
                    eprintln!("Failed to bind work service to {}: {}", socket_addr, e);
                    return;
                }

                if let Err(e) = socket.listen(1024) {
                    eprintln!("Failed to listen on {}: {}", socket_addr, e);
                    return;
                }

                let std_listener: std::net::TcpListener = socket.into();
                if let Err(e) = std_listener.set_nonblocking(true) {
                    eprintln!("Failed to set nonblocking: {}", e);
                    return;
                }

                match tokio::net::TcpListener::from_std(std_listener) {
                    Ok(listener) => {
                        let incoming = tokio_stream::wrappers::TcpListenerStream::new(listener);
                        if let Err(e) = Server::builder()
                            .add_service(WorkServiceServer::new(service))
                            .serve_with_incoming(incoming)
                            .await
                        {
                            eprintln!("Work service error on {}: {}", socket_addr, e);
                        }
                    }
                    Err(e) => {
                        eprintln!("Failed to create tokio listener on {}: {}", socket_addr, e);
                    }
                }
            });
        }

        if let Some(rx) = rx_guard.as_mut() {
            rx.recv().await
        } else {
            None
        }
    }
}
