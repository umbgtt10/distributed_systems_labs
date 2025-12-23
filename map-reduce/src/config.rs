use rand::Rng;
use serde::Deserialize;
use std::fs;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub num_strings: usize,
    pub max_string_length: usize,
    pub num_target_words: usize,
    pub target_word_length: usize,
    pub partition_size: usize,
    pub keys_per_reducer: usize,
    pub num_mappers: usize,
    pub num_reducers: usize,
}

impl Config {
    pub fn load(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let contents = fs::read_to_string(path)?;
        let config: Config = serde_json::from_str(&contents)?;
        Ok(config)
    }

    pub fn default() -> Self {
        Self {
            num_strings: 1_000_000,
            max_string_length: 20,
            num_target_words: 100,
            target_word_length: 3,
            partition_size: 10_000,
            keys_per_reducer: 10,
            num_mappers: 100,
            num_reducers: 10,
        }
    }
}

/// Generate a random string of up to max_len characters
pub fn generate_random_string(rng: &mut impl Rng, max_len: usize) -> String {
    let len = rng.random_range(1..=max_len);
    (0..len)
        .map(|_| {
            let idx = rng.random_range(0..26);
            (b'a' + idx) as char
        })
        .collect()
}

/// Generate a random target word of specified length
pub fn generate_target_word(rng: &mut impl Rng, length: usize) -> String {
    (0..length)
        .map(|_| {
            let idx = rng.random_range(0..26);
            (b'a' + idx) as char
        })
        .collect()
}
