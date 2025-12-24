use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    pub num_strings: usize,
    pub max_string_length: usize,
    pub num_target_words: usize,
    pub target_word_length: usize,
    pub partition_size: usize,
    pub keys_per_reducer: usize,
    pub num_mappers: usize,
    pub num_reducers: usize,
    /// Probability (0-100) that a mapper fails during execution
    #[serde(default)]
    pub mapper_failure_probability: u32,
    /// Probability (0-100) that a reducer fails during execution
    #[serde(default)]
    pub reducer_failure_probability: u32,
    /// Maximum allowed execution time for a mapper in milliseconds (0 = no timeout)
    #[serde(default)]
    pub mapper_timeout_ms: u64,
    /// Maximum allowed execution time for a reducer in milliseconds (0 = no timeout)
    #[serde(default)]
    pub reducer_timeout_ms: u64,
    /// Probability (0-100) that a mapper becomes a straggler (slow)
    #[serde(default)]
    pub mapper_straggler_probability: u32,
    /// Maximum delay in milliseconds for a mapper straggler
    #[serde(default = "default_straggler_delay")]
    pub mapper_straggler_delay_ms: u64,
    /// Probability (0-100) that a reducer becomes a straggler (slow)
    #[serde(default)]
    pub reducer_straggler_probability: u32,
    /// Maximum delay in milliseconds for a reducer straggler
    #[serde(default = "default_straggler_delay")]
    pub reducer_straggler_delay_ms: u64,
}

fn default_straggler_delay() -> u64 {
    1000
}

impl Config {
    pub fn load(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let contents = fs::read_to_string(path)?;
        let config: Config = serde_json::from_str(&contents)?;
        Ok(config)
    }

    pub fn print_summary(&self) {
        println!("Configuration:");
        println!("  - Strings: {}", self.num_strings);
        println!("  - Max string length: {}", self.max_string_length);
        println!("  - Target words: {}", self.num_target_words);
        println!("  - Target word length: {}", self.target_word_length);
        println!("  - Partition size: {}", self.partition_size);
        println!("  - Keys per reducer: {}", self.keys_per_reducer);
        println!("  - Mappers: {}", self.num_mappers);
        println!("  - Reducers: {}", self.num_reducers);

        if self.mapper_failure_probability > 0
            || self.reducer_failure_probability > 0
            || self.mapper_straggler_probability > 0
            || self.reducer_straggler_probability > 0
            || self.mapper_timeout_ms > 0
            || self.reducer_timeout_ms > 0
        {
            println!("\nFault Tolerance:");
            if self.mapper_failure_probability > 0 {
                println!(
                    "  - Mapper failure probability: {}%",
                    self.mapper_failure_probability
                );
            }
            if self.reducer_failure_probability > 0 {
                println!(
                    "  - Reducer failure probability: {}%",
                    self.reducer_failure_probability
                );
            }
            if self.mapper_straggler_probability > 0 {
                println!(
                    "  - Mapper straggler probability: {}% (delay up to {}ms)",
                    self.mapper_straggler_probability, self.mapper_straggler_delay_ms
                );
            }
            if self.reducer_straggler_probability > 0 {
                println!(
                    "  - Reducer straggler probability: {}% (delay up to {}ms)",
                    self.reducer_straggler_probability, self.reducer_straggler_delay_ms
                );
            }
            if self.mapper_timeout_ms > 0 {
                println!("  - Mapper timeout: {}ms", self.mapper_timeout_ms);
            }
            if self.reducer_timeout_ms > 0 {
                println!("  - Reducer timeout: {}ms", self.reducer_timeout_ms);
            }
        }
    }
}
