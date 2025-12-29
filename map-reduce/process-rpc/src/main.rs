pub mod config;
mod grpc_completion_signaling;
mod grpc_state_access;
mod grpc_state_server;
mod grpc_work_channel;
mod mapper;
mod process_runtime;
mod reducer;
pub mod rpc;

use clap::Parser;
use grpc_completion_signaling::GrpcCompletionSignaling;
use grpc_state_access::GrpcStateAccess;
use grpc_state_server::start_state_server;
use map_reduce_core::config::Config;
use map_reduce_core::local_state_access::LocalStateAccess;
use map_reduce_core::map_reduce_job::MapReduceJob;
use map_reduce_core::mapper::MapperTask;
use map_reduce_core::reducer::ReducerTask;
use map_reduce_core::shutdown_signal::ShutdownSignal;
use map_reduce_core::state_access::StateAccess;
use map_reduce_core::utils::{generate_test_data, initialize_phase};
use map_reduce_core::worker_runtime::WorkerTask;
use map_reduce_word_search::{WordSearchContext, WordSearchProblem};
use mapper::{Mapper, MapperFactory};
use process_runtime::{MapperProcessRuntime, ReducerProcessRuntime};
use reducer::{Reducer, ReducerFactory};
use serde::{Deserialize, Serialize};
use std::time::Instant;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[arg(long)]
    worker: bool,

    #[arg(long)]
    r#type: Option<String>,

    #[arg(long)]
    task: Option<String>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct DummyShutdownSignal;
impl ShutdownSignal for DummyShutdownSignal {
    fn is_cancelled(&self) -> bool {
        false
    }
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    if cli.worker {
        run_worker(cli).await;
    } else {
        run_coordinator().await;
    }
}

async fn run_worker(cli: Cli) {
    let task_json = cli.task.expect("Task JSON required for worker");
    let worker_type = cli.r#type.expect("Worker type required");

    match worker_type.as_str() {
        "mapper" => {
            let task: MapperTask<
                WordSearchProblem,
                GrpcStateAccess,
                DummyShutdownSignal,
                grpc_work_channel::GrpcWorkReceiver<
                    <WordSearchProblem as MapReduceJob>::MapAssignment,
                    grpc_completion_signaling::GrpcCompletionToken,
                >,
                grpc_completion_signaling::GrpcCompletionToken,
            > = serde_json::from_str(&task_json).expect("Failed to deserialize mapper task");
            task.run().await;
        }
        "reducer" => {
            let task: ReducerTask<
                WordSearchProblem,
                GrpcStateAccess,
                DummyShutdownSignal,
                grpc_work_channel::GrpcWorkReceiver<
                    <WordSearchProblem as MapReduceJob>::ReduceAssignment,
                    grpc_completion_signaling::GrpcCompletionToken,
                >,
                grpc_completion_signaling::GrpcCompletionToken,
            > = serde_json::from_str(&task_json).expect("Failed to deserialize reducer task");
            task.run().await;
        }
        _ => panic!("Unknown worker type: {}", worker_type),
    }
}

async fn run_coordinator() {
    let start_time = Instant::now();

    // Load configuration
    let config = Config::load("config.json").expect("Failed to load config.json");

    println!("=== MAP-REDUCE WORD SEARCH (Proto-RPC-Tonic/gRPC) ===");
    config.print_summary();

    let (data, targets) = generate_test_data(&config);

    // Start State Server with gRPC
    let local_state = LocalStateAccess::new();
    local_state.initialize(targets.clone()).await;

    // Pick random port for state server
    let state_port = rand::random::<u16>() % 10000 + 20000;
    let _state_handle = start_state_server(local_state.clone(), state_port)
        .await
        .expect("Failed to start gRPC state server");

    let grpc_state = GrpcStateAccess::new(format!("127.0.0.1:{}", state_port));
    let shutdown_signal = DummyShutdownSignal;

    println!("\nStarting MapReduce with gRPC...");

    // Define types
    type MapperType = Mapper<
        WordSearchProblem,
        GrpcStateAccess,
        grpc_work_channel::GrpcWorkChannel<
            <WordSearchProblem as MapReduceJob>::MapAssignment,
            grpc_completion_signaling::GrpcCompletionToken,
        >,
        MapperProcessRuntime,
        DummyShutdownSignal,
    >;

    type ReducerType = Reducer<
        WordSearchProblem,
        GrpcStateAccess,
        grpc_work_channel::GrpcWorkChannel<
            <WordSearchProblem as MapReduceJob>::ReduceAssignment,
            grpc_completion_signaling::GrpcCompletionToken,
        >,
        ReducerProcessRuntime,
        DummyShutdownSignal,
    >;

    // Create mapper factory
    let mapper_factory = MapperFactory::<
        WordSearchProblem,
        GrpcStateAccess,
        MapperProcessRuntime,
        DummyShutdownSignal,
    >::new(
        grpc_state.clone(),
        shutdown_signal.clone(),
        config.mapper_failure_probability,
        config.mapper_straggler_probability,
        config.mapper_straggler_delay_ms,
    );

    // Initialize mapper phase
    let (mappers, mut mapper_executor) = initialize_phase::<MapperType, GrpcCompletionSignaling, _>(
        config.num_mappers,
        mapper_factory,
        config.mapper_timeout_ms,
    );

    println!("Workers initialized, starting map phase...");

    // Create reducer factory
    let reducer_factory = ReducerFactory::<
        WordSearchProblem,
        GrpcStateAccess,
        ReducerProcessRuntime,
        DummyShutdownSignal,
    >::new(
        grpc_state.clone(),
        shutdown_signal.clone(),
        config.reducer_failure_probability,
        config.reducer_straggler_probability,
        config.reducer_straggler_delay_ms,
    );

    // Initialize reducer phase
    let (reducers, mut reducer_executor) =
        initialize_phase::<ReducerType, GrpcCompletionSignaling, _>(
            config.num_reducers,
            reducer_factory,
            config.reducer_timeout_ms,
        );

    println!("Reducers initialized, starting reduce phase...");

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
        WordSearchProblem::create_reduce_assignments(context.clone(), config.keys_per_reducer);
    let reducers = reducer_executor
        .execute(reducers, reduce_assignments, &shutdown_signal)
        .await;
    println!("All reducers completed!");

    drop(mappers);
    drop(reducers);

    // Extract final results from state
    let final_results_map = local_state.get_map();
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
