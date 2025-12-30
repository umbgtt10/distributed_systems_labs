mod storage;
pub use storage::Storage;

mod storage_error;
pub use storage_error::StorageError;

mod key_value_server;
pub use key_value_server::KeyValueServer;

mod grpc_client;
pub use grpc_client::GrpcClient;

mod client_config;
pub use client_config::{ClientConfig, TestConfig};

pub mod rpc {
    pub mod proto {
        include!("../.generated/kvservice.rs");
    }
}
