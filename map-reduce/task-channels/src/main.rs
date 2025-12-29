mod channel_status_sender;
mod channel_work_receiver;
mod channel_work_sender;
mod channel_worker_runtime;
mod channel_worker_synchronization;
mod mapper;
mod reducer;

use channel_status_sender::ChannelStatusSender;
use channel_work_sender::MpscWorkChannel;
use channel_worker_runtime::{TokenShutdownSignal, TokioRuntime};
use channel_worker_synchronization::ChannelCompletionSignaling;
use map_reduce_core::config::Config;
use map_reduce_core::in_memory_state_store::LocalStateAccess;
use map_reduce_core::map_reduce_job::MapReduceJob;
use map_reduce_core::state_store::StateStore;
use map_reduce_core::utils::{generate_test_data, initialize_phase};
use map_reduce_word_search::{WordSearchContext, WordSearchProblem};
use mapper::{Mapper, MapperFactory};
use reducer::{Reducer, ReducerFactory};
use std::time::Instant;
use tokio::{signal, spawn};
use tokio_util::sync::CancellationToken;

#[tokio::main]
async fn main() {
    let start_time = Instant::now();

    // Load configuration from JSON file
    let config = Config::load("config.json").expect("Failed to load config.json");

    println!("=== MAP-REDUCE WORD SEARCH ===");
    config.print_summary();

    let (data, targets) = generate_test_data(&config);

    // Create state access layer
    let state = LocalStateAccess::new();
    state.initialize(targets.clone()).await;

    println!("\nStarting MapReduce...");

    // Create cancellation token
    let cancel_token = CancellationToken::new();
    let shutdown_signal = TokenShutdownSignal::new(cancel_token.clone());

    // Define mapper type
    type MapperType = Mapper<
        WordSearchProblem,
        LocalStateAccess,
        MpscWorkChannel<<WordSearchProblem as MapReduceJob>::MapAssignment, ChannelStatusSender>,
        TokioRuntime,
        TokenShutdownSignal,
    >;

    // Create mapper factory
    let mapper_factory = MapperFactory::<
        WordSearchProblem,
        LocalStateAccess,
        TokioRuntime,
        TokenShutdownSignal,
    >::new(
        state.clone(),
        shutdown_signal.clone(),
        config.mapper_failure_probability,
        config.mapper_straggler_probability,
        config.mapper_straggler_delay_ms,
    );

    // Create initial mapper pool
    let (mappers, mut mapper_executor) =
        initialize_phase::<MapperType, ChannelCompletionSignaling, _>(
            config.num_mappers,
            mapper_factory,
            config.mapper_timeout_ms,
        )
        .await;

    // Define reducer type
    type ReducerType = Reducer<
        WordSearchProblem,
        LocalStateAccess,
        MpscWorkChannel<<WordSearchProblem as MapReduceJob>::ReduceAssignment, ChannelStatusSender>,
        TokioRuntime,
        TokenShutdownSignal,
    >;

    // Create reducer factory
    let reducer_factory = ReducerFactory::<
        WordSearchProblem,
        LocalStateAccess,
        TokioRuntime,
        TokenShutdownSignal,
    >::new(
        state.clone(),
        shutdown_signal.clone(),
        config.reducer_failure_probability,
        config.reducer_straggler_probability,
        config.reducer_straggler_delay_ms,
    );

    // Create initial reducer pool
    let (reducers, mut reducer_executor) =
        initialize_phase::<ReducerType, ChannelCompletionSignaling, _>(
            config.num_reducers,
            reducer_factory,
            config.reducer_timeout_ms,
        )
        .await;

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
    let mappers = mapper_executor
        .execute(mappers, map_assignments, &shutdown_signal)
        .await;
    println!("All mappers completed!");

    // Execute reduce phase
    println!("\n=== REDUCE PHASE ===");
    println!("Starting {} reducers...", config.num_reducers);
    let reduce_assignments =
        WordSearchProblem::create_reduce_assignments(context, config.keys_per_reducer);
    let reducers = reducer_executor
        .execute(reducers, reduce_assignments, &shutdown_signal)
        .await;
    println!("All reducers completed!");

    // Initiate shutdown
    println!("\n=== SHUTTING DOWN ===");
    cancel_token.cancel();

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
