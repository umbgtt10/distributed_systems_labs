use crate::mapper::{Mapper, WorkAssignment};
use crate::reducer::{Reducer, ReducerAssignment};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

/// Orchestrator coordinates the map-reduce workflow
pub struct Orchestrator {
    cancellation_token: CancellationToken,
}

impl Orchestrator {
    pub fn new() -> Self {
        Self {
            cancellation_token: CancellationToken::new(),
        }
    }

    /// Returns a clone of the cancellation token for external control
    pub fn cancellation_token(&self) -> CancellationToken {
        self.cancellation_token.clone()
    }

    /// Runs the complete map-reduce workflow
    pub async fn run(
        &mut self,
        data_chunks: Vec<Vec<String>>,
        targets: Vec<String>,
        shared_map: Arc<Mutex<HashMap<String, Vec<i32>>>>,
    ) {
        println!("=== ORCHESTRATOR STARTED ===");

        // MAP PHASE - Assign work to 100 mappers
        println!("\n=== MAP PHASE ===");
        println!("Starting {} mappers...", data_chunks.len());

        let mut mapper_tasks: Vec<JoinHandle<()>> = Vec::new();

        // Start all mappers in parallel
        for (id, chunk) in data_chunks.into_iter().enumerate() {
            let assignment = WorkAssignment {
                chunk_id: id,
                data: chunk,
                targets: targets.clone(),
            };

            let mapper = Mapper::new(id, shared_map.clone(), self.cancellation_token.clone());
            let handle = mapper.start(assignment);
            mapper_tasks.push(handle);
        }

        // Wait for all mappers to complete
        println!("Waiting for all mappers to complete...");
        for (idx, task) in mapper_tasks.into_iter().enumerate() {
            if let Err(e) = task.await {
                eprintln!("Mapper {} task failed: {}", idx, e);
            }
        }
        println!("All mappers completed!");

        // REDUCE PHASE - Assign work to 10 reducers
        println!("\n=== REDUCE PHASE ===");
        println!("Starting 10 reducers...");

        let num_reducers = 10;
        let keys_per_reducer = targets.len() / num_reducers;
        let mut reducer_tasks: Vec<JoinHandle<()>> = Vec::new();

        // Partition the keys among reducers
        for reducer_id in 0..num_reducers {
            let start = reducer_id * keys_per_reducer;
            let end = if reducer_id == num_reducers - 1 {
                targets.len()
            } else {
                (reducer_id + 1) * keys_per_reducer
            };

            let assigned_keys = targets[start..end].to_vec();
            let assignment = ReducerAssignment {
                keys: assigned_keys,
            };

            let reducer = Reducer::new(reducer_id, shared_map.clone());
            let handle = reducer.reduce(assignment).await;
            reducer_tasks.push(handle);
        }

        // Wait for all reducers to complete
        println!("Waiting for all reducers to complete...");
        for (idx, task) in reducer_tasks.into_iter().enumerate() {
            if let Err(e) = task.await {
                eprintln!("Reducer {} task failed: {}", idx, e);
            }
        }
        println!("All reducers completed!");

        println!("\n=== ORCHESTRATOR FINISHED ===");
    }
}
