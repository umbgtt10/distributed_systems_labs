use crate::rpc::proto::{
    get_response, kv_service_client::KvServiceClient, put_response, ErrorType, GetRequest,
    PutRequest,
};
use crate::ClientConfig;
use std::time::Duration;
use tokio::time::sleep;
use tokio_util::sync::CancellationToken;

pub struct GrpcClient {
    config: ClientConfig,
    server_address: String,
    cancellation_token: CancellationToken,
}

impl GrpcClient {
    pub fn new(config: ClientConfig, server_address: String) -> Self {
        Self {
            config,
            server_address,
            cancellation_token: CancellationToken::new(),
        }
    }

    pub fn cancellation_token(&self) -> CancellationToken {
        self.cancellation_token.clone()
    }

    pub async fn start(self) -> Result<(), Box<dyn std::error::Error>> {
        let mut client = KvServiceClient::connect(self.server_address.clone()).await?;
        println!(
            "[{}] Connected to KV Server at {}",
            self.config.name, self.server_address
        );
        println!(
            "[{}] Running stress test with {} keys...\n",
            self.config.name,
            self.config.keys.len()
        );

        let mut operation_count = 0;

        loop {
            // Check for cancellation
            if self.cancellation_token.is_cancelled() {
                println!("\n[{}] Shutting down client...", self.config.name);
                break;
            }

            operation_count += 1;

            // Randomly select a key from config
            let key = &self.config.keys[fastrand::usize(0..self.config.keys.len())];

            // Randomly choose between get and put
            let is_get = fastrand::bool();

            if is_get {
                perform_get(&mut client, &self.config, key, operation_count).await;
            } else {
                perform_put(&mut client, &self.config, key, operation_count).await;
            }
        }

        println!("[{}] Client stopped", self.config.name);
        Ok(())
    }
}

async fn perform_get(
    client: &mut KvServiceClient<tonic::transport::Channel>,
    config: &ClientConfig,
    key: &str,
    op_num: u64,
) {
    let request = tonic::Request::new(GetRequest {
        key: key.to_string(),
    });

    match client.get(request).await {
        Ok(response) => {
            let result = response.into_inner().result;
            match result {
                Some(get_response::Result::Success(success)) => {
                    println!(
                        "[{}][{}] GET '{}' -> OK (value='{}', version={})",
                        config.name, op_num, key, success.value, success.version
                    );
                    sleep(Duration::from_millis(config.success_sleep_ms)).await;
                }
                Some(get_response::Result::Error(error)) => {
                    let error_type =
                        ErrorType::try_from(error.error_type).unwrap_or(ErrorType::KeyNotFound);
                    println!(
                        "[{}][{}] GET '{}' -> ERROR ({:?}: {})",
                        config.name, op_num, key, error_type, error.message
                    );
                    sleep(Duration::from_millis(config.error_sleep_ms)).await;
                }
                None => {
                    println!(
                        "[{}][{}] GET '{}' -> ERROR (No result)",
                        config.name, op_num, key
                    );
                    sleep(Duration::from_millis(config.error_sleep_ms)).await;
                }
            }
        }
        Err(status) => {
            println!(
                "[{}][{}] GET '{}' -> NETWORK ERROR ({})",
                config.name, op_num, key, status
            );
            sleep(Duration::from_millis(config.error_sleep_ms)).await;
        }
    }
}

async fn perform_put(
    client: &mut KvServiceClient<tonic::transport::Channel>,
    config: &ClientConfig,
    key: &str,
    op_num: u64,
) {
    let value = format!("value_{}", fastrand::u32(..));

    // Start with version 0 (create), will be adjusted on version mismatch
    let mut version = 0;
    let mut retry_count = 0;
    const MAX_RETRIES: u32 = 3;

    loop {
        let request = tonic::Request::new(PutRequest {
            key: key.to_string(),
            value: value.clone(),
            version,
        });

        match client.put(request).await {
            Ok(response) => {
                let result = response.into_inner().result;
                match result {
                    Some(put_response::Result::Success(success)) => {
                        let operation = if version == 0 { "CREATE" } else { "UPDATE" };
                        println!(
                            "[{}][{}] PUT '{}' -> {} (value='{}', new_version={})",
                            config.name, op_num, key, operation, value, success.new_version
                        );
                        sleep(Duration::from_millis(config.success_sleep_ms)).await;
                        return;
                    }
                    Some(put_response::Result::Error(error)) => {
                        let error_type =
                            ErrorType::try_from(error.error_type).unwrap_or(ErrorType::KeyNotFound);

                        match error_type {
                            ErrorType::VersionMismatch => {
                                retry_count += 1;
                                if retry_count >= MAX_RETRIES {
                                    println!(
                                        "[{}][{}] PUT '{}' -> FAILED after {} retries ({})",
                                        config.name, op_num, key, retry_count, error.message
                                    );
                                    sleep(Duration::from_millis(config.error_sleep_ms)).await;
                                    return;
                                }
                                // Extract actual version from error message and increment
                                // Message format: "Version mismatch: expected X, got Y"
                                if let Some(actual_version) = extract_actual_version(&error.message)
                                {
                                    version = actual_version;
                                    println!("[{}][{}] PUT '{}' -> RETRY (version_mismatch, using version={})", config.name, op_num, key, version);
                                    continue;
                                }
                            }
                            ErrorType::KeyAlreadyExists => {
                                // Key exists but we tried to create, switch to update
                                println!(
                                    "[{}][{}] PUT '{}' -> KEY_EXISTS (switching to update mode)",
                                    config.name, op_num, key
                                );
                                version = 1; // Start with version 1 and retry
                                retry_count += 1;
                                continue;
                            }
                            ErrorType::KeyNotFound => {
                                // Key doesn't exist, try to create
                                println!(
                                    "[{}][{}] PUT '{}' -> KEY_NOT_FOUND (switching to create mode)",
                                    config.name, op_num, key
                                );
                                version = 0;
                                retry_count += 1;
                                continue;
                            }
                        }

                        println!(
                            "[{}][{}] PUT '{}' -> ERROR ({:?}: {})",
                            config.name, op_num, key, error_type, error.message
                        );
                        sleep(Duration::from_millis(config.error_sleep_ms)).await;
                        return;
                    }
                    None => {
                        println!(
                            "[{}][{}] PUT '{}' -> ERROR (No result)",
                            config.name, op_num, key
                        );
                        sleep(Duration::from_millis(config.error_sleep_ms)).await;
                        return;
                    }
                }
            }
            Err(status) => {
                println!(
                    "[{}][{}] PUT '{}' -> NETWORK ERROR ({})",
                    config.name, op_num, key, status
                );
                sleep(Duration::from_millis(config.error_sleep_ms)).await;
                return;
            }
        }
    }
}

fn extract_actual_version(message: &str) -> Option<u64> {
    // Parse "Version mismatch: expected X, got Y" to extract Y (actual version)
    let parts: Vec<&str> = message.split(", got ").collect();
    if parts.len() == 2 {
        parts[1].parse::<u64>().ok()
    } else {
        None
    }
}
