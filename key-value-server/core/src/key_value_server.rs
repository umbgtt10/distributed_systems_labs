use crate::rpc::proto::{
    get_response, kv_service_server::KvService, put_response, ErrorType, GetError, GetRequest,
    GetResponse, GetSuccess, PutError, PutRequest, PutResponse, PutSuccess,
};
use crate::{Storage, StorageError};
use std::sync::Arc;
use tonic::{Request, Response, Status};

pub struct KeyValueServer<S: Storage> {
    storage: Arc<S>,
}

impl<S: Storage> KeyValueServer<S> {
    pub fn new(storage: S) -> Self {
        Self {
            storage: Arc::new(storage),
        }
    }
}

#[tonic::async_trait]
impl<S: Storage + 'static> KvService for KeyValueServer<S> {
    async fn get(&self, request: Request<GetRequest>) -> Result<Response<GetResponse>, Status> {
        let key = request.into_inner().key;

        match self.storage.get(&key).await {
            Ok((value, version)) => Ok(Response::new(GetResponse {
                result: Some(get_response::Result::Success(GetSuccess { value, version })),
            })),
            Err(StorageError::KeyNotFound(_)) => Ok(Response::new(GetResponse {
                result: Some(get_response::Result::Error(GetError {
                    error_type: ErrorType::KeyNotFound as i32,
                    message: format!("Key '{}' not found", key),
                })),
            })),
            Err(e) => Ok(Response::new(GetResponse {
                result: Some(get_response::Result::Error(GetError {
                    error_type: ErrorType::KeyNotFound as i32,
                    message: e.to_string(),
                })),
            })),
        }
    }

    async fn put(&self, request: Request<PutRequest>) -> Result<Response<PutResponse>, Status> {
        let req = request.into_inner();

        match self.storage.put(&req.key, req.value, req.version).await {
            Ok(new_version) => Ok(Response::new(PutResponse {
                result: Some(put_response::Result::Success(PutSuccess { new_version })),
            })),
            Err(StorageError::KeyAlreadyExists(_)) => Ok(Response::new(PutResponse {
                result: Some(put_response::Result::Error(PutError {
                    error_type: ErrorType::KeyAlreadyExists as i32,
                    message: format!("Key '{}' already exists", req.key),
                })),
            })),
            Err(StorageError::VersionMismatch { expected, actual }) => {
                Ok(Response::new(PutResponse {
                    result: Some(put_response::Result::Error(PutError {
                        error_type: ErrorType::VersionMismatch as i32,
                        message: format!("Version mismatch: expected {}, got {}", actual, expected),
                    })),
                }))
            }
            Err(StorageError::KeyNotFound(_)) => Ok(Response::new(PutResponse {
                result: Some(put_response::Result::Error(PutError {
                    error_type: ErrorType::KeyNotFound as i32,
                    message: format!("Key '{}' not found", req.key),
                })),
            })),
        }
    }
}
