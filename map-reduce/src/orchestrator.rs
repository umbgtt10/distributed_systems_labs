use crate::mapper::{Mapper, WorkAssignment};
use crate::reducer::{Reducer, ReducerAssignment};
use tokio::sync::mpsc;

/// Orchestrator coordinates the map-reduce workflow
pub struct Orchestrator {
    mappers: Vec<Mapper>,
    reducers: Vec<Reducer>,
}

impl Orchestrator {
    pub fn new(mappers: Vec<Mapper>, reducers: Vec<Reducer>) -> Self {
        Self { mappers, reducers }
    }

    /// Runs the complete map-reduce workflow
    pub async fn run(
        self,
        data_chunks: Vec<Vec<String>>,
        targets: Vec<String>,
        keys_per_reducer: usize,
    ) {
        println!("=== ORCHESTRATOR STARTED ===");

        // MAP PHASE - Distribute work to mappers
        println!("\n=== MAP PHASE ===");
        println!(
            "Distributing {} chunks to {} mappers...",
            data_chunks.len(),
            self.mappers.len()
        );

        // Create completion channel
        let (complete_tx, mut complete_rx) = mpsc::channel::<usize>(self.mappers.len());

        // Track which chunks have been assigned and which mappers are available
        let mut chunk_index = 0;
        let mut active_mappers = 0;

        // Assign initial work to all mappers
        for mapper in self.mappers.iter() {
            if chunk_index < data_chunks.len() {
                let assignment = WorkAssignment {
                    chunk_id: chunk_index,
                    data: data_chunks[chunk_index].clone(),
                    targets: targets.clone(),
                };
                let tx = complete_tx.clone();
                mapper.send_map_assignment(assignment, tx);
                chunk_index += 1;
                active_mappers += 1;
            }
        }

        // As mappers complete, assign them more work
        while active_mappers > 0 {
            if let Some(mapper_id) = complete_rx.recv().await {
                active_mappers -= 1;

                // Assign next chunk if available
                if chunk_index < data_chunks.len() {
                    let assignment = WorkAssignment {
                        chunk_id: chunk_index,
                        data: data_chunks[chunk_index].clone(),
                        targets: targets.clone(),
                    };
                    let tx = complete_tx.clone();
                    self.mappers[mapper_id].send_map_assignment(assignment, tx);
                    chunk_index += 1;
                    active_mappers += 1;
                }
            }
        }

        // Wait for all mappers to fully shut down
        println!("Waiting for all mappers to complete...");
        for (idx, mapper) in self.mappers.into_iter().enumerate() {
            if let Err(e) = mapper.wait().await {
                eprintln!("Mapper {} task failed: {}", idx, e);
            }
        }
        println!("All mappers completed!");

        // REDUCE PHASE - Assign work to reducers
        println!("\n=== REDUCE PHASE ===");
        println!("Starting {} reducers...", self.reducers.len());

        // Create completion channel for reducers
        let (reduce_complete_tx, mut reduce_complete_rx) =
            mpsc::channel::<usize>(self.reducers.len());

        // Partition the keys among reducers based on keys_per_reducer
        let num_key_partitions = targets.len().div_ceil(keys_per_reducer);

        // Create all key partition assignments upfront
        let mut key_partitions = Vec::new();
        for partition_id in 0..num_key_partitions {
            let start = partition_id * keys_per_reducer;
            let end = std::cmp::min(start + keys_per_reducer, targets.len());
            key_partitions.push(targets[start..end].to_vec());
        }

        println!(
            "Distributing {} key partitions to {} reducers...",
            key_partitions.len(),
            self.reducers.len()
        );

        // Track which partitions have been assigned and which reducers are available
        let mut partition_index = 0;
        let mut active_reducers = 0;

        // Assign initial work to all reducers
        for reducer in self.reducers.iter() {
            if partition_index < key_partitions.len() {
                let assignment = ReducerAssignment {
                    keys: key_partitions[partition_index].clone(),
                };
                let tx = reduce_complete_tx.clone();
                reducer.send_reduce_assignment(assignment, tx);
                partition_index += 1;
                active_reducers += 1;
            }
        }

        // As reducers complete, assign them more work
        while active_reducers > 0 {
            if let Some(reducer_id) = reduce_complete_rx.recv().await {
                active_reducers -= 1;

                // Assign next partition if available
                if partition_index < key_partitions.len() {
                    let assignment = ReducerAssignment {
                        keys: key_partitions[partition_index].clone(),
                    };
                    let tx = reduce_complete_tx.clone();
                    self.reducers[reducer_id].send_reduce_assignment(assignment, tx);
                    partition_index += 1;
                    active_reducers += 1;
                }
            }
        }

        // Wait for all reducers to fully shut down
        println!("Waiting for all reducers to complete...");
        for (idx, reducer) in self.reducers.into_iter().enumerate() {
            if let Err(e) = reducer.wait().await {
                eprintln!("Reducer {} task failed: {}", idx, e);
            }
        }
        println!("All reducers completed!");

        println!("\n=== ORCHESTRATOR FINISHED ===");
    }
}
