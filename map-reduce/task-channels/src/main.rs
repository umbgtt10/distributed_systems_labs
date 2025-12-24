mod channel_completion_signaling;
mod config;
mod local_state_access;
mod mapper;
mod mpsc_work_channel;
mod reducer;
mod task_work_distributor;
mod tokio_runtime;

use channel_completion_signaling::ChannelCompletionSignaling;
use config::{generate_random_string, generate_target_word, Config};
use local_state_access::LocalStateAccess;
use map_reduce_core::map_reduce_problem::MapReduceProblem;
use map_reduce_core::orchestrator::Orchestrator;
use map_reduce_core::state_access::StateAccess;
use map_reduce_word_search::{WordSearchProblem, WordSearchContext};
use mapper::Mapper;
use mpsc_work_channel::MpscWorkChannel;
use reducer::Reducer;
use std::time::Instant;
use task_work_distributor::TaskWorkDistributor;
use tokio_runtime::{TokioRuntime, TokenShutdownSignal};

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

    println!("\nStarting MapReduce...");

    // Create cancellation token
    let cancel_token = tokio_util::sync::CancellationToken::new();
    let shutdown_signal = TokenShutdownSignal::new(cancel_token.clone());

    // Define mapper type
    type MapperType = Mapper<
        WordSearchProblem,
        LocalStateAccess,
        MpscWorkChannel<<WordSearchProblem as map_reduce_core::map_reduce_problem::MapReduceProblem>::MapAssignment, tokio::sync::mpsc::Sender<channel_completion_signaling::CompletionMessage>>,
        TokioRuntime,
        TokenShutdownSignal,
    >;

    // Create mapper factory
    let state_for_mapper = state.clone();
    let shutdown_for_mapper = shutdown_signal.clone();
    let mapper_factory = move |mapper_id: usize| -> MapperType {
        let (work_channel, work_rx) = MpscWorkChannel::<
            <WordSearchProblem as MapReduceProblem>::MapAssignment,
            tokio::sync::mpsc::Sender<channel_completion_signaling::CompletionMessage>
        >::create_pair(10);
        Mapper::new(
            mapper_id,
            state_for_mapper.clone(),
            shutdown_for_mapper.clone(),
            work_rx,
            work_channel,
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
        );
        mappers.push(mapper);
    }

    // Define reducer type
    type ReducerType = Reducer<
        WordSearchProblem,
        LocalStateAccess,
        MpscWorkChannel<<WordSearchProblem as map_reduce_core::map_reduce_problem::MapReduceProblem>::ReduceAssignment, tokio::sync::mpsc::Sender<channel_completion_signaling::CompletionMessage>>,
        TokioRuntime,
        TokenShutdownSignal,
    >;

    // Create reducer factory
    let state_for_reducer = state.clone();
    let shutdown_for_reducer = shutdown_signal.clone();
    let reducer_factory = move |reducer_id: usize| -> ReducerType {
        let (work_channel, work_rx) = MpscWorkChannel::<
            <WordSearchProblem as MapReduceProblem>::ReduceAssignment,
            tokio::sync::mpsc::Sender<channel_completion_signaling::CompletionMessage>
        >::create_pair(10);
        Reducer::new(
            reducer_id,
            state_for_reducer.clone(),
            shutdown_for_reducer.clone(),
            work_rx,
            work_channel,
        )
    };

    // Create initial reducer pool
    let mut reducers: Vec<ReducerType> = Vec::new();
    for reducer_id in 0..config.num_reducers {
        let (work_channel, work_rx) = MpscWorkChannel::<
            <WordSearchProblem as MapReduceProblem>::ReduceAssignment,
            tokio::sync::mpsc::Sender<channel_completion_signaling::CompletionMessage>
        >::create_pair(10);
        let reducer = Reducer::new(
            reducer_id,
            state.clone(),
            shutdown_signal.clone(),
            work_rx,
            work_channel,
        );
        reducers.push(reducer);
    }

    // Create work distributors with factories
    let mapper_distributor = TaskWorkDistributor::<MapperType, ChannelCompletionSignaling, _>::new(mapper_factory);
    let reducer_distributor = TaskWorkDistributor::<ReducerType, ChannelCompletionSignaling, _>::new(reducer_factory);

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

    // Run the orchestrator with factory functions from the problem
    let context = WordSearchContext { targets: targets.clone() };

    orchestrator
        .run(
            mappers,
            reducers,
            |data, context, partition_size| {
                WordSearchProblem::create_map_assignments(data, context, partition_size)
            },
            |context, keys_per_reducer| {
                WordSearchProblem::create_reduce_assignments(context, keys_per_reducer)
            },
            data,
            context,
            config.partition_size,
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
