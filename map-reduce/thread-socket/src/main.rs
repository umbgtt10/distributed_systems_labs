mod config;
mod local_state_access;
mod mapper;
mod reducer;
mod socket_completion_signaling;
mod socket_work_channel;
mod socket_work_distributor;
mod thread_runtime;

use config::{generate_random_string, generate_target_word, Config};
use local_state_access::LocalStateAccess;
use map_reduce_core::map_reduce_problem::MapReduceProblem;
use map_reduce_core::state_access::StateAccess;
use map_reduce_core::worker::Worker;
use map_reduce_word_search::{WordSearchContext, WordSearchProblem};
use mapper::Mapper;
use reducer::Reducer;
use socket_completion_signaling::{CompletionSender, SocketCompletionSignaling};
use socket_work_channel::SocketWorkChannel;
use socket_work_distributor::SocketWorkDistributor;
use std::time::Instant;
use thread_runtime::{AtomicShutdownSignal, ThreadRuntime};

fn main() {
    let start_time = Instant::now();

    // Load configuration
    let config = match Config::load("config.json") {
        Ok(cfg) => cfg,
        Err(e) => {
            eprintln!("Failed to load config.json: {}", e);
            eprintln!("Using default configuration...");
            Config::default()
        }
    };

    println!("=== MAP-REDUCE WORD SEARCH (Thread-Socket) ===");
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

    // Generate data
    let data: Vec<String> = (0..config.num_strings)
        .map(|_| generate_random_string(&mut rng, config.max_string_length))
        .collect();
    println!("Generated {} strings", data.len());

    let targets: Vec<String> = (0..config.num_target_words)
        .map(|_| generate_target_word(&mut rng, config.target_word_length))
        .collect();
    println!("Generated {} target words", targets.len());

    // Create state
    let state = LocalStateAccess::new();
    state.initialize(targets.clone());

    println!("\nStarting MapReduce...");

    // Create shutdown signal
    let shutdown_signal = AtomicShutdownSignal::new();

    // Setup Ctrl+C handler
    let shutdown_for_handler = shutdown_signal.clone();
    ctrlc::set_handler(move || {
        println!("\n\n=== Ctrl+C received, initiating shutdown ===");
        shutdown_for_handler.shutdown();
    })
    .expect("Error setting Ctrl-C handler");

    // Create completion signaling
    let mapper_signaling =
        SocketCompletionSignaling::new(config.completion_base_port, config.num_mappers);
    let reducer_signaling =
        SocketCompletionSignaling::new(config.completion_base_port + 100, config.num_reducers);

    // Define types
    type MapperType = Mapper<
        WordSearchProblem,
        LocalStateAccess,
        SocketWorkChannel<<WordSearchProblem as MapReduceProblem>::MapAssignment, CompletionSender>,
        ThreadRuntime,
        AtomicShutdownSignal,
    >;

    type ReducerType = Reducer<
        WordSearchProblem,
        LocalStateAccess,
        SocketWorkChannel<
            <WordSearchProblem as MapReduceProblem>::ReduceAssignment,
            CompletionSender,
        >,
        ThreadRuntime,
        AtomicShutdownSignal,
    >;

    // Create mappers
    let mut mapper_port = config.mapper_base_port;
    let mut mappers = Vec::new();
    for mapper_id in 0..config.num_mappers {
        let (work_channel, work_rx) = SocketWorkChannel::create_pair(mapper_port);
        mapper_port += 1;
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

    // Create mapper factory
    let state_for_mapper = state.clone();
    let shutdown_for_mapper = shutdown_signal.clone();
    let mut next_mapper_port = mapper_port;
    let mapper_failure_prob = config.mapper_failure_probability;
    let mapper_straggler_prob = config.mapper_straggler_probability;
    let mapper_straggler_delay = config.mapper_straggler_delay_ms;
    let mapper_factory = move |mapper_id: usize| -> MapperType {
        let (work_channel, work_rx) = SocketWorkChannel::create_pair(next_mapper_port);
        next_mapper_port += 1;
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

    // Create mapper distributor
    let mut mapper_distributor =
        SocketWorkDistributor::with_timeout(mapper_factory, config.mapper_timeout_ms);

    // Create reducers
    let mut reducer_port = config.reducer_base_port;
    let mut reducers = Vec::new();
    for reducer_id in 0..config.num_reducers {
        let (work_channel, work_rx) = SocketWorkChannel::create_pair(reducer_port);
        reducer_port += 1;
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

    // Create reducer factory
    let state_for_reducer = state.clone();
    let shutdown_for_reducer = shutdown_signal.clone();
    let mut next_reducer_port = reducer_port;
    let reducer_failure_prob = config.reducer_failure_probability;
    let reducer_straggler_prob = config.reducer_straggler_probability;
    let reducer_straggler_delay = config.reducer_straggler_delay_ms;
    let reducer_factory = move |reducer_id: usize| -> ReducerType {
        let (work_channel, work_rx) = SocketWorkChannel::create_pair(next_reducer_port);
        next_reducer_port += 1;
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

    // Create reducer distributor
    let mut reducer_distributor =
        SocketWorkDistributor::with_timeout(reducer_factory, config.reducer_timeout_ms);

    // Run map phase
    println!("\n=== MAP PHASE ===");
    println!("Distributing data to {} mappers...", config.num_mappers);
    let context = WordSearchContext {
        targets: targets.clone(),
    };
    let map_assignments =
        WordSearchProblem::create_map_assignments(data, context.clone(), config.partition_size);
    let mappers = mapper_distributor.distribute_work(
        mappers,
        map_assignments,
        &mapper_signaling,
        &shutdown_signal,
    );
    println!("All mappers completed!");

    // Run reduce phase
    println!("\n=== REDUCE PHASE ===");
    println!("Starting {} reducers...", config.num_reducers);
    let reduce_assignments =
        WordSearchProblem::create_reduce_assignments(context, config.keys_per_reducer);
    let reducers = reducer_distributor.distribute_work(
        reducers,
        reduce_assignments,
        &reducer_signaling,
        &shutdown_signal,
    );
    println!("All reducers completed!");

    // Shutdown signal and wait for workers to exit
    println!("\n=== SHUTTING DOWN ===");
    shutdown_signal.shutdown();

    // Drop signaling to close all listeners
    drop(mapper_signaling);
    drop(reducer_signaling);

    // Drop all workers (threads will exit when they check shutdown signal)
    drop(mappers);
    drop(reducers);

    // Give threads a moment to check shutdown and exit
    std::thread::sleep(std::time::Duration::from_millis(100));

    println!("All workers terminated gracefully");

    // Extract results
    let final_results_map = state.get_map();
    let final_results = final_results_map.lock().unwrap();

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
