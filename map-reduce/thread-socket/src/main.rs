mod mapper;
mod reducer;
mod socket_work_sender;
mod socket_worker_runtime;
mod socket_worker_synchonization;

use map_reduce_core::config::Config;
use map_reduce_core::in_memory_state_store::LocalStateAccess;
use map_reduce_core::map_reduce_job::MapReduceJob;
use map_reduce_core::state_store::StateStore;
use map_reduce_core::utils::{generate_test_data, initialize_phase};
use map_reduce_word_search::{WordSearchContext, WordSearchProblem};
use mapper::{Mapper, MapperFactory};
use reducer::{Reducer, ReducerFactory};
use socket_work_sender::SocketWorkSender;
use socket_worker_runtime::{AtomicShutdownSignal, ThreadRuntime};
use socket_worker_synchonization::{SocketCompletionSignaling, SocketCompletionToken};
use std::time::Instant;

#[tokio::main]
async fn main() {
    let start_time = Instant::now();

    // Load configuration
    let config = Config::load("config.json").expect("Failed to load config.json");

    println!("=== MAP-REDUCE WORD SEARCH (Thread-Socket) ===");
    config.print_summary();

    let (data, targets) = generate_test_data(&config);

    // Create state
    let state = LocalStateAccess::new();
    state.initialize(targets.clone()).await;

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

    // Define types
    type MapperType = Mapper<
        WordSearchProblem,
        LocalStateAccess,
        SocketWorkSender<<WordSearchProblem as MapReduceJob>::MapAssignment, SocketCompletionToken>,
        ThreadRuntime,
        AtomicShutdownSignal,
    >;

    type ReducerType = Reducer<
        WordSearchProblem,
        LocalStateAccess,
        SocketWorkSender<
            <WordSearchProblem as MapReduceJob>::ReduceAssignment,
            SocketCompletionToken,
        >,
        ThreadRuntime,
        AtomicShutdownSignal,
    >;

    // Create mapper factory
    let mapper_factory = MapperFactory::<
        WordSearchProblem,
        LocalStateAccess,
        ThreadRuntime,
        AtomicShutdownSignal,
    >::new(
        state.clone(),
        shutdown_signal.clone(),
        config.mapper_failure_probability,
        config.mapper_straggler_probability,
        config.mapper_straggler_delay_ms,
    );

    // Initialize mapper phase
    let (mappers, mut mapper_executor) =
        initialize_phase::<MapperType, SocketCompletionSignaling, _>(
            config.num_mappers,
            mapper_factory,
            config.mapper_timeout_ms,
        )
        .await;

    // Create reducer factory
    let reducer_factory = ReducerFactory::<
        WordSearchProblem,
        LocalStateAccess,
        ThreadRuntime,
        AtomicShutdownSignal,
    >::new(
        state.clone(),
        shutdown_signal.clone(),
        config.reducer_failure_probability,
        config.reducer_straggler_probability,
        config.reducer_straggler_delay_ms,
    );

    // Initialize reducer phase
    let (reducers, mut reducer_executor) =
        initialize_phase::<ReducerType, SocketCompletionSignaling, _>(
            config.num_reducers,
            reducer_factory,
            config.reducer_timeout_ms,
        )
        .await;

    // Run map phase
    println!("\n=== MAP PHASE ===");
    println!("Distributing data to {} mappers...", config.num_mappers);
    let context = WordSearchContext {
        targets: targets.clone(),
    };
    let map_assignments =
        WordSearchProblem::create_map_assignments(data, context.clone(), config.partition_size);
    let mappers = mapper_executor
        .execute(mappers, map_assignments, &shutdown_signal)
        .await;
    println!("All mappers completed!");

    // Run reduce phase
    println!("\n=== REDUCE PHASE ===");
    println!("Starting {} reducers...", config.num_reducers);
    let reduce_assignments =
        WordSearchProblem::create_reduce_assignments(context, config.keys_per_reducer);
    let reducers = reducer_executor
        .execute(reducers, reduce_assignments, &shutdown_signal)
        .await;
    println!("All reducers completed!");

    // Shutdown signal and wait for workers to exit
    println!("\n=== SHUTTING DOWN ===");
    shutdown_signal.shutdown();

    // Drop workers to release resources
    drop(mappers);
    drop(reducers);

    // Give threads a moment to check shutdown flag and exit
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

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
