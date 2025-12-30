// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientConfig {
    pub name: String,
    pub success_sleep_ms: u64,
    pub error_sleep_ms: u64,
    #[serde(default)]
    pub client_packet_loss_rate: f32,
    pub keys: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestConfig {
    pub test_duration_seconds: u64,
    #[serde(default)]
    pub server_packet_loss_rate: f32,
    #[serde(default = "default_max_retries_server_packet_loss")]
    pub max_retries_server_packet_loss: u32,
    pub clients: Vec<ClientConfig>,
}

fn default_max_retries_server_packet_loss() -> u32 {
    10
}

impl TestConfig {
    pub fn from_file(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path)?;
        let config: TestConfig = serde_json::from_str(&content)?;
        Ok(config)
    }
}

