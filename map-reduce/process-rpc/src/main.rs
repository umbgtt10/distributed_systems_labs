mod mapper;
mod process_runtime;
mod reducer;
mod rpc;
mod rpc_completion_signaling;
mod rpc_state_access;
mod rpc_work_channel;
mod state_server;

use clap::Parser;
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
use rpc_completion_signaling::{RpcCompletionSignaling, RpcCompletionToken};
use rpc_state_access::RpcStateAccess;
use rpc_work_channel::{RpcWorkChannel, RpcWorkReceiver};
use serde::{Deserialize, Serialize};
use state_server::StateServer;
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
                RpcStateAccess,
                DummyShutdownSignal,
                RpcWorkReceiver<
                    <WordSearchProblem as MapReduceJob>::MapAssignment,
                    RpcCompletionToken,
                >,
                RpcCompletionToken,
            > = serde_json::from_str(&task_json).expect("Failed to deserialize mapper task");
            task.run().await;
        }
        "reducer" => {
            let task: ReducerTask<
                WordSearchProblem,
                RpcStateAccess,
                DummyShutdownSignal,
                RpcWorkReceiver<
                    <WordSearchProblem as MapReduceJob>::ReduceAssignment,
                    RpcCompletionToken,
                >,
                RpcCompletionToken,
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

    println!("=== MAP-REDUCE WORD SEARCH (Process-RPC) ===");
    config.print_summary();

    let (data, targets) = generate_test_data(&config);

    // Start State Server
    let local_state = LocalStateAccess::new();
    local_state.initialize(targets.clone());

    // Pick random port for state server
    let state_port = rand::random::<u16>() % 10000 + 20000;
    let state_server = StateServer::new(local_state.clone(), state_port).await;
    // Force 127.0.0.1 for client connection
    let state_addr = format!("127.0.0.1:{}", state_server.local_addr().port()).parse().unwrap();

    tokio::spawn(state_server.run());

    let rpc_state = RpcStateAccess::new(state_addr);
    let shutdown_signal = DummyShutdownSignal;

    println!("\nStarting MapReduce...");

    // Define types
    type MapperType = Mapper<
        WordSearchProblem,
        RpcStateAccess,
        RpcWorkChannel<<WordSearchProblem as MapReduceJob>::MapAssignment, RpcCompletionToken>,
        MapperProcessRuntime,
        DummyShutdownSignal,
    >;

    type ReducerType = Reducer<
        WordSearchProblem,
        RpcStateAccess,
        RpcWorkChannel<
            <WordSearchProblem as MapReduceJob>::ReduceAssignment,
            RpcCompletionToken,
        >,
        ReducerProcessRuntime,
        DummyShutdownSignal,
    >;

    // Create mapper factory
    let mapper_factory = MapperFactory::<
        WordSearchProblem,
        RpcStateAccess,
        MapperProcessRuntime,
        DummyShutdownSignal,
    >::new(
        rpc_state.clone(),
        shutdown_signal.clone(),
        config.mapper_failure_probability,
        config.mapper_straggler_probability,
        config.mapper_straggler_delay_ms,
    );

    // Initialize mapper phase
    let (mappers, mut mapper_executor) = initialize_phase::<MapperType, RpcCompletionSignaling, _>(
        config.num_mappers,
        mapper_factory,
        config.mapper_timeout_ms,
    );

    // Create reducer factory
    let reducer_factory = ReducerFactory::<
        WordSearchProblem,
        RpcStateAccess,
        ReducerProcessRuntime,
        DummyShutdownSignal,
    >::new(
        rpc_state.clone(),
        shutdown_signal.clone(),
        config.reducer_failure_probability,
        config.reducer_straggler_probability,
        config.reducer_straggler_delay_ms,
    );

    // Initialize reducer phase
    let (reducers, mut reducer_executor) = initialize_phase::<ReducerType, RpcCompletionSignaling, _>(
        config.num_reducers,
        reducer_factory,
        config.reducer_timeout_ms,
    );

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
