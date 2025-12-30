mod storage;
pub use storage::Storage;

mod storage_error;
pub use storage_error::StorageError;

mod key_value_server;
pub use key_value_server::KeyValueServer;

pub mod rpc {
    pub mod proto {
        include!("../.generated/kvservice.rs");
    }
}
