use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    pub num_strings: usize,
    pub max_string_length: usize,
    pub num_target_words: usize,
    pub target_word_length: usize,
    pub partition_size: usize,
    pub keys_per_reducer: usize,
    pub num_mappers: usize,
    pub num_reducers: usize,
    pub mapper_failure_probability: u32,
    pub reducer_failure_probability: u32,
    pub mapper_straggler_probability: u32,
    pub mapper_straggler_delay_ms: u64,
    pub reducer_straggler_probability: u32,
    pub reducer_straggler_delay_ms: u64,
    pub mapper_timeout_ms: u64,
    pub reducer_timeout_ms: u64,
    pub mapper_base_port: u16,
    pub reducer_base_port: u16,
    pub completion_base_port: u16,
}

impl Config {
    pub fn load(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let contents = fs::read_to_string(path)?;
        let config = serde_json::from_str(&contents)?;
        Ok(config)
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            num_strings: 500_000,
            max_string_length: 15,
            num_target_words: 1000,
            target_word_length: 3,
            partition_size: 5000,
            keys_per_reducer: 5,
            num_mappers: 15,
            num_reducers: 10,
            mapper_failure_probability: 0,
            reducer_failure_probability: 0,
            mapper_straggler_probability: 0,
            mapper_straggler_delay_ms: 1000,
            reducer_straggler_probability: 0,
            reducer_straggler_delay_ms: 1000,
            mapper_timeout_ms: 0,
            reducer_timeout_ms: 0,
            mapper_base_port: 9000,
            reducer_base_port: 9100,
            completion_base_port: 9200,
        }
    }
}

pub fn generate_random_string(rng: &mut impl rand::Rng, max_length: usize) -> String {
    let length = rng.random_range(1..=max_length);
    (0..length)
        .map(|_| {
            let c = rng.random_range(b'a'..=b'z');
            c as char
        })
        .collect()
}

pub fn generate_target_word(rng: &mut impl rand::Rng, length: usize) -> String {
    (0..length)
        .map(|_| {
            let c = rng.random_range(b'a'..=b'z');
            c as char
        })
        .collect()
}
