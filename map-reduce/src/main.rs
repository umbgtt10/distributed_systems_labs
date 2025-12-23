mod mapper;
mod orchestrator;
mod reducer;

use orchestrator::Orchestrator;
use rand::Rng;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// Generate a random string of up to max_len characters
fn generate_random_string(rng: &mut impl Rng, max_len: usize) -> String {
    let len = rng.gen_range(1..=max_len);
    (0..len)
        .map(|_| {
            let idx = rng.gen_range(0..26);
            (b'a' + idx) as char
        })
        .collect()
}

/// Generate a random 3-digit word (3 characters)
fn generate_3_digit_word(rng: &mut impl Rng) -> String {
    (0..3)
        .map(|_| {
            let idx = rng.gen_range(0..26);
            (b'a' + idx) as char
        })
        .collect()
}

#[tokio::main]
async fn main() {
    let start_time = Instant::now();

    println!("=== MAP-REDUCE WORD SEARCH ===");
    println!("Generating data...");

    let mut rng = rand::thread_rng();

    // Generate 1,000,000 random strings (up to 20 chars each)
    let data: Vec<String> = (0..1_000_000)
        .map(|_| generate_random_string(&mut rng, 20))
        .collect();

    println!("Generated {} strings", data.len());

    // Generate 100 random 3-character target words
    let targets: Vec<String> = (0..100).map(|_| generate_3_digit_word(&mut rng)).collect();

    println!("Generated {} target words", targets.len());

    // Create shared HashMap<String, Vec<i32>> for mappers to update
    let shared_map: Arc<Mutex<HashMap<String, Vec<i32>>>> = Arc::new(Mutex::new(
        targets
            .iter()
            .map(|word| (word.clone(), Vec::new()))
            .collect(),
    ));

    // Partition data into 100 chunks for 100 mappers
    let num_mappers = 100;
    let chunk_size = data.len() / num_mappers;
    let mut data_chunks = Vec::new();

    for i in 0..num_mappers {
        let start = i * chunk_size;
        let end = if i == num_mappers - 1 {
            data.len()
        } else {
            (i + 1) * chunk_size
        };
        data_chunks.push(data[start..end].to_vec());
    }

    println!("Partitioned data into {} chunks", data_chunks.len());
    println!("\nStarting MapReduce...");

    // Create orchestrator and run
    let mut orchestrator = Orchestrator::new();
    let cancel_token = orchestrator.cancellation_token();

    // Setup Ctrl+C handler
    let ctrl_c_token = cancel_token.clone();
    tokio::spawn(async move {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to listen for Ctrl+C");
        println!("\n\n=== Ctrl+C received, initiating shutdown ===");
        ctrl_c_token.cancel();
    });

    // Run the orchestrator
    orchestrator
        .run(data_chunks, targets, shared_map.clone())
        .await;

    // Extract final results
    let final_results = shared_map.lock().unwrap();

    // Display results
    println!("\n=== RESULTS ===");
    let mut sorted_results: Vec<_> = final_results.iter().collect();
    sorted_results.sort_by(|a, b| {
        let a_count = a.1.first().unwrap_or(&0);
        let b_count = b.1.first().unwrap_or(&0);
        b_count.cmp(a_count).then(a.0.cmp(b.0))
    });

    let mut total_occurrences = 0;
    for (word, count_vec) in sorted_results.iter().take(20) {
        let count = count_vec.first().unwrap_or(&0);
        println!("{}: {}", word, count);
        total_occurrences += count;
    }

    if sorted_results.len() > 20 {
        println!("... ({} more words)", sorted_results.len() - 20);
        for (_, count_vec) in sorted_results.iter().skip(20) {
            let count = count_vec.first().unwrap_or(&0);
            total_occurrences += count;
        }
    }

    println!("\nTotal occurrences found: {}", total_occurrences);

    let elapsed = start_time.elapsed();
    println!("\n=== PROGRAM COMPLETE ===");
    println!("Total time: {:.2}s", elapsed.as_secs_f64());
}
