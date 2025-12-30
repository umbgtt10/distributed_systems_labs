// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

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
    max_retries: u32,
    cancellation_token: CancellationToken,
}

impl GrpcClient {
    pub fn new(config: ClientConfig, server_address: String, max_retries: u32) -> Self {
        Self {
            config,
            server_address,
            max_retries,
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
                perform_put(
                    &mut client,
                    &self.config,
                    key,
                    operation_count,
                    self.max_retries,
                    &self.cancellation_token,
                )
                .await;
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
    // Simulate client-side packet loss BEFORE sending request
    if fastrand::f32() < (config.client_packet_loss_rate / 100.0) {
        println!(
            "[{}][{}] GET '{}' -> CLIENT PACKET LOSS (request not sent)",
            config.name, op_num, key
        );
        sleep(Duration::from_millis(config.error_sleep_ms)).await;
        return;
    }

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
                config.name,
                op_num,
                key,
                status.message()
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
    max_retries: u32,
    cancellation_token: &CancellationToken,
) {
    let value = format!("value_{}", fastrand::u32(..));

    // Start with version 0 (create), will be adjusted on version mismatch
    let mut version = 0;
    let mut network_retry_count = 0;

    loop {
        // Simulate client-side packet loss BEFORE sending request
        if fastrand::f32() < (config.client_packet_loss_rate / 100.0) {
            network_retry_count += 1;
            println!(
                "[{}][{}] PUT '{}' -> CLIENT PACKET LOSS (request not sent)",
                config.name, op_num, key
            );

            if network_retry_count >= max_retries {
                println!(
                    "[{}][{}] PUT '{}' -> CLIENT PACKET LOSS after {} attempts, giving up",
                    config.name, op_num, key, network_retry_count
                );
                sleep(Duration::from_millis(config.error_sleep_ms)).await;
                return;
            }

            if cancellation_token.is_cancelled() {
                println!(
                    "[{}][{}] PUT '{}' -> CANCELLED during client packet loss retry",
                    config.name, op_num, key
                );
                return;
            }

            sleep(Duration::from_millis(config.error_sleep_ms)).await;
            continue;
        }

        let request = tonic::Request::new(PutRequest {
            key: key.to_string(),
            value: value.clone(),
            version,
        });

        match client.put(request).await {
            Ok(response) => {
                // Save network retry count before resetting for recovery detection
                let had_network_errors = network_retry_count > 0;
                let retry_count_for_log = network_retry_count;

                // Network is working - reset retry counter
                network_retry_count = 0;

                let result = response.into_inner().result;
                match result {
                    Some(put_response::Result::Success(success)) => {
                        let operation = if version == 0 { "CREATE" } else { "UPDATE" };
                        if had_network_errors {
                            let retry_word = if retry_count_for_log == 1 {
                                "retry"
                            } else {
                                "retries"
                            };
                            println!(
                                "[{}][{}] PUT '{}' -> {} RECOVERED after {} network {} (value='{}', new_version={})",
                                config.name, op_num, key, operation, retry_count_for_log, retry_word, value, success.new_version
                            );
                        } else {
                            println!(
                                "[{}][{}] PUT '{}' -> {} (value='{}', new_version={})",
                                config.name, op_num, key, operation, value, success.new_version
                            );
                        }
                        sleep(Duration::from_millis(config.success_sleep_ms)).await;
                        return;
                    }
                    Some(put_response::Result::Error(error)) => {
                        let error_type =
                            ErrorType::try_from(error.error_type).unwrap_or(ErrorType::KeyNotFound);

                        match error_type {
                            ErrorType::VersionMismatch => {
                                // Use the structured actual_version field from the error
                                if let Some(actual_version) = error.actual_version {
                                    if had_network_errors {
                                        let retry_word = if retry_count_for_log == 1 {
                                            "retry"
                                        } else {
                                            "retries"
                                        };
                                        println!(
                                            "[{}][{}] PUT '{}' -> RECOVERED after {} network {} (write succeeded, detected via version_mismatch, server version={})",
                                            config.name, op_num, key, retry_count_for_log, retry_word, actual_version
                                        );
                                        // Recovery detected - the previous write succeeded, we're done!
                                        sleep(Duration::from_millis(config.success_sleep_ms)).await;
                                        return;
                                    }
                                    version = actual_version;
                                    println!("[{}][{}] PUT '{}' -> RETRY (version_mismatch, using version={})", config.name, op_num, key, version);
                                    continue;
                                } else {
                                    println!(
                                        "[{}][{}] PUT '{}' -> ERROR (VersionMismatch without actual_version)",
                                        config.name, op_num, key
                                    );
                                }
                            }
                            ErrorType::KeyAlreadyExists => {
                                // Key exists but we tried to create - fetch actual version with GET
                                println!(
                                    "[{}][{}] PUT '{}' -> KEY_EXISTS (fetching current version)",
                                    config.name, op_num, key
                                );

                                // Do a GET to fetch the current version
                                let get_request = tonic::Request::new(GetRequest {
                                    key: key.to_string(),
                                });

                                match client.get(get_request).await {
                                    Ok(get_response) => {
                                        if let Some(get_response::Result::Success(success)) =
                                            get_response.into_inner().result
                                        {
                                            version = success.version;
                                            println!(
                                                "[{}][{}] PUT '{}' -> Fetched version={}, switching to update mode",
                                                config.name, op_num, key, version
                                            );
                                            continue;
                                        }
                                    }
                                    Err(_) => {
                                        // If GET fails, fall back to version 1
                                        version = 1;
                                        continue;
                                    }
                                }
                            }
                            ErrorType::KeyNotFound => {
                                // Key doesn't exist, try to create
                                println!(
                                    "[{}][{}] PUT '{}' -> KEY_NOT_FOUND (switching to create mode)",
                                    config.name, op_num, key
                                );
                                version = 0;
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
                network_retry_count += 1;
                if network_retry_count >= max_retries {
                    println!(
                        "[{}][{}] PUT '{}' -> NETWORK ERROR after {} retries ({})",
                        config.name,
                        op_num,
                        key,
                        network_retry_count,
                        status.message()
                    );
                    sleep(Duration::from_millis(config.error_sleep_ms)).await;
                    return;
                }

                // Check for cancellation before retrying
                if cancellation_token.is_cancelled() {
                    println!(
                        "[{}][{}] PUT '{}' -> CANCELLED during network retry",
                        config.name, op_num, key
                    );
                    return;
                }

                println!(
                    "[{}][{}] PUT '{}' -> NETWORK ERROR, retrying... (attempt {}/{}): {}",
                    config.name,
                    op_num,
                    key,
                    network_retry_count,
                    max_retries,
                    status.message()
                );
                sleep(Duration::from_millis(config.error_sleep_ms)).await;
                continue;
            }
        }
    }
}

