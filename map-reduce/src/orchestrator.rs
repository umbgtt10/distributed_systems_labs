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
    pub async fn run(self, data_chunks: Vec<Vec<String>>, targets: Vec<String>) {
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
                mapper.map_assignment(assignment, tx);
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
                    self.mappers[mapper_id].map_assignment(assignment, tx);
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

        let keys_per_reducer = targets.len() / self.reducers.len();

        // Create completion channel for reducers
        let (reduce_complete_tx, mut reduce_complete_rx) =
            mpsc::channel::<usize>(self.reducers.len());

        // Partition the keys among reducers
        for (reducer_id, reducer) in self.reducers.iter().enumerate() {
            let start = reducer_id * keys_per_reducer;
            let end = if reducer_id == self.reducers.len() - 1 {
                targets.len()
            } else {
                (reducer_id + 1) * keys_per_reducer
            };

            let assigned_keys = targets[start..end].to_vec();
            let assignment = ReducerAssignment {
                keys: assigned_keys,
            };

            reducer.reduce_assignment(assignment, reduce_complete_tx.clone());
        }

        // Wait for all reducers to signal completion
        drop(reduce_complete_tx);
        let mut completed_reducers = 0;
        while completed_reducers < self.reducers.len() {
            if let Some(_reducer_id) = reduce_complete_rx.recv().await {
                completed_reducers += 1;
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
