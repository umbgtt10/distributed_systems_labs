// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::rpc::proto::kv_service_client::KvServiceClient;
use crate::{ClientConfig, FastrandRandom, GetOperation, PutOperation, Random, Timer, TokioTimer};
use tokio_util::sync::CancellationToken;

pub struct GrpcClient<T: Timer = TokioTimer, R: Random = FastrandRandom> {
    config: ClientConfig,
    server_address: String,
    max_retries: u32,
    cancellation_token: CancellationToken,
    timer: T,
    random: R,
}

impl<T: Timer, R: Random> GrpcClient<T, R> {
    pub fn new(
        config: ClientConfig,
        server_address: String,
        max_retries: u32,
        timer: T,
        random: R,
    ) -> Self {
        Self {
            config,
            server_address,
            max_retries,
            cancellation_token: CancellationToken::new(),
            timer,
            random,
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

            self.perform_operation(
                &mut client,
                &self.config,
                operation_count,
                self.max_retries,
                &self.cancellation_token,
            )
            .await;
        }

        println!("[{}] Client stopped", self.config.name);
        Ok(())
    }

    pub async fn perform_operation(
        &self,
        client: &mut KvServiceClient<tonic::transport::Channel>,
        config: &ClientConfig,
        op_num: u64,
        max_retries: u32,
        cancellation_token: &CancellationToken,
    ) {
        let key = &config.keys[self.random.usize(0..config.keys.len())];

        let is_get = self.random.bool();

        if is_get {
            let op = GetOperation::new(config, key, op_num, &self.timer, &self.random);
            op.execute(client).await;
        } else {
            let value = format!("value_{}", self.random.u32(0..u32::MAX));

            let op = PutOperation::new(
                config,
                key,
                value,
                op_num,
                max_retries,
                cancellation_token,
                &self.timer,
                &self.random,
            );
            let _ = op.execute(client).await;
        }
    }
}
