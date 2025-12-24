mod channel_completion_signaling;
mod completion_signaling;
mod config;
mod map_reduce_logic;
mod mapper;
mod orchestrator;
mod reducer;
mod state_access;
mod task_work_distributor;
mod work_channel;
mod work_distributor;
mod worker;
mod worker_runtime;

use channel_completion_signaling::ChannelCompletionSignaling;
use config::{Config, generate_random_string, generate_target_word};
use mapper::{Mapper, WorkAssignment};
use orchestrator::Orchestrator;
use reducer::{Reducer, ReducerAssignment};
use state_access::{LocalStateAccess, StateAccess};
use std::time::Instant;
use task_work_distributor::TaskWorkDistributor;
use work_channel::MpscWorkChannel;
use worker_runtime::{TokenShutdownSignal, TokioRuntime};

#[tokio::main]
async fn main() {
    let start_time = Instant::now();

    // Load configuration from JSON file
    let config = match Config::load("config.json") {
        Ok(cfg) => cfg,
        Err(e) => {
            eprintln!("Failed to load config.json: {}", e);
            eprintln!("Using default configuration...");
            Config::default()
        }
    };

    println!("=== MAP-REDUCE WORD SEARCH ===");
    println!("Configuration:");
    println!("  - Strings: {}", config.num_strings);
    println!("  - Max string length: {}", config.max_string_length);
    println!("  - Target words: {}", config.num_target_words);
    println!("  - Target word length: {}", config.target_word_length);
    println!("  - Partition size: {}", config.partition_size);
    println!("  - Keys per reducer: {}", config.keys_per_reducer);
    println!("  - Mappers: {}", config.num_mappers);
    println!("  - Reducers: {}", config.num_reducers);
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

    // Create state access layer
    let state = LocalStateAccess::new();
    state.initialize(targets.clone());

    // Partition data into chunks based on partition_size
    let mut data_chunks = Vec::new();
    let num_partitions = data.len().div_ceil(config.partition_size);

    for i in 0..num_partitions {
        let start = i * config.partition_size;
        let end = std::cmp::min(start + config.partition_size, data.len());
        data_chunks.push(data[start..end].to_vec());
    }

    println!(
        "Partitioned data into {} chunks for {} mappers",
        data_chunks.len(),
        config.num_mappers
    );
    println!("\nStarting MapReduce...");

    // Create cancellation token
    let cancel_token = tokio_util::sync::CancellationToken::new();
    let shutdown_signal = TokenShutdownSignal::new(cancel_token.clone());

    // Create mapper pool
    type MapperType = Mapper<
        LocalStateAccess,
        MpscWorkChannel<WorkAssignment, tokio::sync::mpsc::Sender<usize>>,
        TokioRuntime,
        TokenShutdownSignal,
    >;

    let mut mappers: Vec<MapperType> = Vec::new();
    for mapper_id in 0..config.num_mappers {
        let (work_channel, work_rx) = MpscWorkChannel::create_pair(10);
        let mapper = Mapper::new(
            mapper_id,
            state.clone(),
            shutdown_signal.clone(),
            work_rx,
            work_channel,
        );
        mappers.push(mapper);
    }

    // Create reducer pool
    type ReducerType = Reducer<
        LocalStateAccess,
        MpscWorkChannel<ReducerAssignment, tokio::sync::mpsc::Sender<usize>>,
        TokioRuntime,
        TokenShutdownSignal,
    >;

    let mut reducers: Vec<ReducerType> = Vec::new();
    for reducer_id in 0..config.num_reducers {
        let (work_channel, work_rx) = MpscWorkChannel::create_pair(10);
        let reducer = Reducer::new(
            reducer_id,
            state.clone(),
            shutdown_signal.clone(),
            work_rx,
            work_channel,
        );
        reducers.push(reducer);
    }

    // Create work distributors
    let mapper_distributor = TaskWorkDistributor::<MapperType, ChannelCompletionSignaling>::new();
    let reducer_distributor = TaskWorkDistributor::<ReducerType, ChannelCompletionSignaling>::new();

    // Create orchestrator with the distributors
    let orchestrator = Orchestrator::new(mapper_distributor, reducer_distributor);

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
        .run(
            mappers,
            reducers,
            data_chunks,
            targets,
            config.keys_per_reducer,
        )
        .await;

    // Extract final results from state
    let final_results_map = state.get_map();
    let final_results = final_results_map.lock().unwrap();

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
