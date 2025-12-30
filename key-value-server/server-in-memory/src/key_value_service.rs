use crate::key_value_service::rpc::proto::{
    get_response, kv_service_server::KvService, put_response, ErrorType, GetError, GetRequest,
    GetResponse, GetSuccess, PutError, PutRequest, PutResponse, PutSuccess,
};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::Mutex;
use tonic::{Request, Response, Status};

pub mod rpc {
    pub mod proto {
        include!("../.generated/kvservice.rs");
    }
}

type Storage = Arc<Mutex<HashMap<String, (String, u64)>>>;

pub struct KvServiceImpl {
    storage: Storage,
}

impl KvServiceImpl {
    pub fn new() -> Self {
        Self {
            storage: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

#[tonic::async_trait]
impl KvService for KvServiceImpl {
    async fn get(&self, request: Request<GetRequest>) -> Result<Response<GetResponse>, Status> {
        let key = request.into_inner().key;
        let storage = self.storage.lock().await;

        if let Some((value, version)) = storage.get(&key) {
            Ok(Response::new(GetResponse {
                result: Some(get_response::Result::Success(GetSuccess {
                    value: value.clone(),
                    version: *version,
                })),
            }))
        } else {
            Ok(Response::new(GetResponse {
                result: Some(get_response::Result::Error(GetError {
                    error_type: ErrorType::KeyNotFound as i32,
                    message: format!("Key '{}' not found", key),
                })),
            }))
        }
    }

    async fn put(&self, request: Request<PutRequest>) -> Result<Response<PutResponse>, Status> {
        let req = request.into_inner();
        let mut storage = self.storage.lock().await;

        if req.version == 0 {
            // Create new key
            if storage.contains_key(&req.key) {
                return Ok(Response::new(PutResponse {
                    result: Some(put_response::Result::Error(PutError {
                        error_type: ErrorType::KeyAlreadyExists as i32,
                        message: format!("Key '{}' already exists", req.key),
                    })),
                }));
            }
            storage.insert(req.key, (req.value, 1));
            Ok(Response::new(PutResponse {
                result: Some(put_response::Result::Success(PutSuccess { new_version: 1 })),
            }))
        } else {
            // Update existing key
            if let Some((_, current_version)) = storage.get(&req.key) {
                if *current_version == req.version {
                    let new_version = req.version + 1;
                    storage.insert(req.key, (req.value, new_version));
                    Ok(Response::new(PutResponse {
                        result: Some(put_response::Result::Success(PutSuccess { new_version })),
                    }))
                } else {
                    Ok(Response::new(PutResponse {
                        result: Some(put_response::Result::Error(PutError {
                            error_type: ErrorType::VersionMismatch as i32,
                            message: format!(
                                "Version mismatch: expected {}, got {}",
                                current_version, req.version
                            ),
                        })),
                    }))
                }
            } else {
                Ok(Response::new(PutResponse {
                    result: Some(put_response::Result::Error(PutError {
                        error_type: ErrorType::KeyNotFound as i32,
                        message: format!("Key '{}' not found", req.key),
                    })),
                }))
            }
        }
    }
}
