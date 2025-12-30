// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::config::Config;
use crate::executor::Executor;
use crate::worker::Worker;
use crate::worker_factory::WorkerFactory;
use crate::worker_synchronization::WorkerSynchronization;
use rand::Rng;

pub fn generate_random_string(rng: &mut impl Rng, max_length: usize) -> String {
    let length = rng.random_range(1..=max_length);
    (0..length)
        .map(|_| {
            let c = rng.random_range(b'a'..=b'z');
            c as char
        })
        .collect()
}

pub fn generate_target_word(rng: &mut impl Rng, length: usize) -> String {
    (0..length)
        .map(|_| {
            let c = rng.random_range(b'a'..=b'z');
            c as char
        })
        .collect()
}

pub fn generate_test_data(config: &Config) -> (Vec<String>, Vec<String>) {
    println!("\nGenerating data...");
    let mut rng = rand::rng();

    // Generate random strings
    let data: Vec<String> = (0..config.num_strings)
        .map(|_| generate_random_string(&mut rng, config.max_string_length))
        .collect();

    println!("Generated {} strings", data.len());

    // Generate random target words
    let targets: Vec<String> = (0..config.num_target_words)
        .map(|_| generate_target_word(&mut rng, config.target_word_length))
        .collect();

    println!("Generated {} target words", targets.len());

    (data, targets)
}

pub async fn initialize_phase<W, S, F>(
    num_workers: usize,
    mut factory: F,
    timeout_ms: u64,
) -> (Vec<W>, Executor<W, S, F>)
where
    W: Worker,
    S: WorkerSynchronization,
    F: WorkerFactory<W>,
{
    let mut workers = Vec::with_capacity(num_workers);
    for id in 0..num_workers {
        workers.push(factory.create_worker(id).await);
    }

    let executor = Executor::new(factory, timeout_ms);

    (workers, executor)
}

