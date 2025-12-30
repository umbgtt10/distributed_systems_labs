// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

mod storage;
pub use storage::Storage;

mod storage_error;
pub use storage_error::StorageError;

mod key_value_server;
pub use key_value_server::KeyValueServer;

mod packet_loss_wrapper;
pub use packet_loss_wrapper::PacketLossWrapper;

mod grpc_client;
pub use grpc_client::GrpcClient;

mod client_config;
pub use client_config::{ClientConfig, TestConfig};

mod server_runner;
pub use server_runner::ServerRunner;

pub mod rpc {
    pub mod proto {
        include!("../.generated/kvservice.rs");
    }
}

