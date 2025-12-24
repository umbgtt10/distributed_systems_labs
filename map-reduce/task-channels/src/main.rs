mod channel_completion_signaling;
mod config;
mod local_state_access;
mod mapper;
mod mpsc_work_channel;
mod reducer;
mod tokio_runtime;

use channel_completion_signaling::{ChannelCompletionSignaling, CompletionMessage};
use config::{generate_random_string, generate_target_word, Config};
use local_state_access::LocalStateAccess;
use map_reduce_core::completion_signaling::CompletionSignaling;
use map_reduce_core::default_phase_executor::DefaultPhaseExecutor;
use map_reduce_core::map_reduce_problem::MapReduceProblem;
use map_reduce_core::state_access::StateAccess;
use map_reduce_word_search::{WordSearchContext, WordSearchProblem};
use mapper::Mapper;
use mpsc_work_channel::MpscWorkChannel;
use reducer::Reducer;
use std::time::Instant;
use tokio::sync::mpsc::Sender;
use tokio::{signal, spawn};
use tokio_runtime::{TokenShutdownSignal, TokioRuntime};
use tokio_util::sync::CancellationToken;

#[tokio::main]
async fn main() {
    let start_time = Instant::now();

    // Load configuration from JSON file
    let config = match Config::load("config.json") {
        Ok(cfg) => cfg,
        Err(e) => {
            eprintln!("Failed to load task-channels/config.json: {}", e);
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
    if config.mapper_failure_probability > 0
        || config.reducer_failure_probability > 0
        || config.mapper_straggler_probability > 0
        || config.reducer_straggler_probability > 0
        || config.mapper_timeout_ms > 0
        || config.reducer_timeout_ms > 0
    {
        println!("\nFault Tolerance:");
        if config.mapper_failure_probability > 0 {
            println!(
                "  - Mapper failure probability: {}%",
                config.mapper_failure_probability
            );
        }
        if config.reducer_failure_probability > 0 {
            println!(
                "  - Reducer failure probability: {}%",
                config.reducer_failure_probability
            );
        }
        if config.mapper_straggler_probability > 0 {
            println!(
                "  - Mapper straggler probability: {}% (delay up to {}ms)",
                config.mapper_straggler_probability, config.mapper_straggler_delay_ms
            );
        }
        if config.reducer_straggler_probability > 0 {
            println!(
                "  - Reducer straggler probability: {}% (delay up to {}ms)",
                config.reducer_straggler_probability, config.reducer_straggler_delay_ms
            );
        }
        if config.mapper_timeout_ms > 0 {
            println!("  - Mapper timeout: {}ms", config.mapper_timeout_ms);
        }
        if config.reducer_timeout_ms > 0 {
            println!("  - Reducer timeout: {}ms", config.reducer_timeout_ms);
        }
    }
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

    println!("\nStarting MapReduce...");

    // Create cancellation token
    let cancel_token = CancellationToken::new();
    let shutdown_signal = TokenShutdownSignal::new(cancel_token.clone());

    // Define mapper type
    type MapperType = Mapper<
        WordSearchProblem,
        LocalStateAccess,
        MpscWorkChannel<
            <WordSearchProblem as MapReduceProblem>::MapAssignment,
            Sender<CompletionMessage>,
        >,
        TokioRuntime,
        TokenShutdownSignal,
    >;

    // Create mapper factory
    let state_for_mapper = state.clone();
    let shutdown_for_mapper = shutdown_signal.clone();
    let mapper_failure_prob = config.mapper_failure_probability;
    let mapper_straggler_prob = config.mapper_straggler_probability;
    let mapper_straggler_delay = config.mapper_straggler_delay_ms;
    let mapper_factory = move |mapper_id: usize| -> MapperType {
        let (work_channel, work_rx) = MpscWorkChannel::<
            <WordSearchProblem as MapReduceProblem>::MapAssignment,
            Sender<CompletionMessage>,
        >::create_pair(10);
        Mapper::new(
            mapper_id,
            state_for_mapper.clone(),
            shutdown_for_mapper.clone(),
            work_rx,
            work_channel,
            mapper_failure_prob,
            mapper_straggler_prob,
            mapper_straggler_delay,
        )
    };

    // Create initial mapper pool
    let mut mappers: Vec<MapperType> = Vec::new();
    for mapper_id in 0..config.num_mappers {
        let (work_channel, work_rx) = MpscWorkChannel::create_pair(10);
        let mapper = Mapper::new(
            mapper_id,
            state.clone(),
            shutdown_signal.clone(),
            work_rx,
            work_channel,
            config.mapper_failure_probability,
            config.mapper_straggler_probability,
            config.mapper_straggler_delay_ms,
        );
        mappers.push(mapper);
    }

    // Define reducer type
    type ReducerType = Reducer<
        WordSearchProblem,
        LocalStateAccess,
        MpscWorkChannel<
            <WordSearchProblem as MapReduceProblem>::ReduceAssignment,
            Sender<CompletionMessage>,
        >,
        TokioRuntime,
        TokenShutdownSignal,
    >;

    // Create reducer factory
    let state_for_reducer = state.clone();
    let shutdown_for_reducer = shutdown_signal.clone();
    let reducer_failure_prob = config.reducer_failure_probability;
    let reducer_straggler_prob = config.reducer_straggler_probability;
    let reducer_straggler_delay = config.reducer_straggler_delay_ms;
    let reducer_factory = move |reducer_id: usize| -> ReducerType {
        let (work_channel, work_rx) = MpscWorkChannel::<
            <WordSearchProblem as MapReduceProblem>::ReduceAssignment,
            Sender<CompletionMessage>,
        >::create_pair(10);
        Reducer::new(
            reducer_id,
            state_for_reducer.clone(),
            shutdown_for_reducer.clone(),
            work_rx,
            work_channel,
            reducer_failure_prob,
            reducer_straggler_prob,
            reducer_straggler_delay,
        )
    };

    // Create initial reducer pool
    let mut reducers: Vec<ReducerType> = Vec::new();
    for reducer_id in 0..config.num_reducers {
        let (work_channel, work_rx) = MpscWorkChannel::<
            <WordSearchProblem as MapReduceProblem>::ReduceAssignment,
            Sender<CompletionMessage>,
        >::create_pair(10);
        let reducer = Reducer::new(
            reducer_id,
            state.clone(),
            shutdown_signal.clone(),
            work_rx,
            work_channel,
            config.reducer_failure_probability,
            config.reducer_straggler_probability,
            config.reducer_straggler_delay_ms,
        );
        reducers.push(reducer);
    }

    // Create phase executors with factories and timeouts
    let mut mapper_executor = DefaultPhaseExecutor::<MapperType, ChannelCompletionSignaling, _>::new(
        mapper_factory,
        config.mapper_timeout_ms,
    );

    let mut reducer_executor =
        DefaultPhaseExecutor::<ReducerType, ChannelCompletionSignaling, _>::new(
            reducer_factory,
            config.reducer_timeout_ms,
        );

    // Setup Ctrl+C handler
    let ctrl_c_token = cancel_token.clone();
    spawn(async move {
        signal::ctrl_c().await.expect("Failed to listen for Ctrl+C");
        println!("\n\n=== Ctrl+C received, initiating shutdown ===");
        ctrl_c_token.cancel();
    });

    // Create problem context
    let context = WordSearchContext {
        targets: targets.clone(),
    };

    // Execute map phase
    println!("\n=== MAP PHASE ===");
    println!("Distributing data to {} mappers...", config.num_mappers);
    let map_assignments =
        WordSearchProblem::create_map_assignments(data, context.clone(), config.partition_size);
    let mapper_signaling = ChannelCompletionSignaling::setup(mappers.len());
    let mappers = mapper_executor
        .execute(mappers, map_assignments, mapper_signaling, |signaling, worker_id| {
            signaling.get_token(worker_id)
        })
        .await;
    println!("All mappers completed!");

    // Execute reduce phase
    println!("\n=== REDUCE PHASE ===");
    println!("Starting {} reducers...", config.num_reducers);
    let reduce_assignments =
        WordSearchProblem::create_reduce_assignments(context, config.keys_per_reducer);
    let reducer_signaling = ChannelCompletionSignaling::setup(reducers.len());
    let reducers = reducer_executor
        .execute(
            reducers,
            reduce_assignments,
            reducer_signaling,
            |signaling, worker_id| signaling.get_token(worker_id),
        )
        .await;
    println!("All reducers completed!");

    // Wait for all workers to shut down
    for (idx, worker) in mappers.into_iter().enumerate() {
        if let Err(e) = worker.wait().await {
            eprintln!("Mapper {} shutdown failed: {}", idx, e);
        }
    }
    for (idx, worker) in reducers.into_iter().enumerate() {
        if let Err(e) = worker.wait().await {
            eprintln!("Reducer {} shutdown failed: {}", idx, e);
        }
    }

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
