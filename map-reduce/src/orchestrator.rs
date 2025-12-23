use crate::mapper::WorkAssignment;
use crate::reducer::ReducerAssignment;
use crate::worker::Worker;
use tokio::sync::mpsc::{self, Sender};
use tokio_stream::StreamExt;
use tokio_stream::StreamMap;
use tokio_stream::wrappers::ReceiverStream;

/// Orchestrator coordinates the map-reduce workflow
pub struct Orchestrator<M: Worker, R: Worker> {
    mappers: Vec<M>,
    reducers: Vec<R>,
}

impl<M: Worker, R: Worker> Orchestrator<M, R> {
    pub fn new(mappers: Vec<M>, reducers: Vec<R>) -> Self {
        Self { mappers, reducers }
    }

    /// Runs the complete map-reduce workflow
    pub async fn run(
        self,
        data_chunks: Vec<Vec<String>>,
        targets: Vec<String>,
        keys_per_reducer: usize,
    ) where
        M::Assignment: From<WorkAssignment>,
        M::Completion: From<Sender<usize>>,
        R::Assignment: From<ReducerAssignment>,
        R::Completion: From<Sender<usize>>,
    {
        println!("=== ORCHESTRATOR STARTED ===");

        // MAP PHASE - Distribute work to mappers
        println!("\n=== MAP PHASE ===");
        println!(
            "Distributing {} chunks to {} mappers...",
            data_chunks.len(),
            self.mappers.len()
        );

        // Create individual completion channels for each mapper
        let mut mapper_completion_txs = Vec::new();
        let mut mapper_streams = StreamMap::new();
        for mapper_idx in 0..self.mappers.len() {
            let (tx, rx) = mpsc::channel::<usize>(10);
            mapper_completion_txs.push(tx);
            mapper_streams.insert(mapper_idx, ReceiverStream::new(rx));
        }

        // Track which chunks have been assigned and which mappers are available
        let mut chunk_index = 0;
        let mut active_mappers = 0;

        // Assign initial work to all mappers
        for (mapper_idx, mapper) in self.mappers.iter().enumerate() {
            if chunk_index < data_chunks.len() {
                let assignment = WorkAssignment {
                    chunk_id: chunk_index,
                    data: data_chunks[chunk_index].clone(),
                    targets: targets.clone(),
                };
                let tx = mapper_completion_txs[mapper_idx].clone();
                mapper.send_work(assignment.into(), tx.into());
                chunk_index += 1;
                active_mappers += 1;
            }
        }

        // As mappers complete, assign them more work
        while active_mappers > 0 {
            if let Some((_stream_idx, mapper_id)) = mapper_streams.next().await {
                active_mappers -= 1;

                // Assign next chunk if available
                if chunk_index < data_chunks.len() {
                    let assignment = WorkAssignment {
                        chunk_id: chunk_index,
                        data: data_chunks[chunk_index].clone(),
                        targets: targets.clone(),
                    };
                    let tx = mapper_completion_txs[mapper_id].clone();
                    self.mappers[mapper_id].send_work(assignment.into(), tx.into());
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

        // Create individual completion channels for each reducer
        let mut reducer_completion_txs = Vec::new();
        let mut reducer_streams = StreamMap::new();
        for reducer_idx in 0..self.reducers.len() {
            let (tx, rx) = mpsc::channel::<usize>(10);
            reducer_completion_txs.push(tx);
            reducer_streams.insert(reducer_idx, ReceiverStream::new(rx));
        }

        // Assign initial work to all reducers
        for (reducer_idx, reducer) in self.reducers.iter().enumerate() {
            if partition_index < key_partitions.len() {
                let assignment = ReducerAssignment {
                    keys: key_partitions[partition_index].clone(),
                };
                let tx = reducer_completion_txs[reducer_idx].clone();
                reducer.send_work(assignment.into(), tx.into());
                partition_index += 1;
                active_reducers += 1;
            }
        }

        // As reducers complete, assign them more work
        while active_reducers > 0 {
            if let Some((_stream_idx, reducer_id)) = reducer_streams.next().await {
                active_reducers -= 1;

                // Assign next partition if available
                if partition_index < key_partitions.len() {
                    let assignment = ReducerAssignment {
                        keys: key_partitions[partition_index].clone(),
                    };
                    let tx = reducer_completion_txs[reducer_id].clone();
                    self.reducers[reducer_id].send_work(assignment.into(), tx.into());
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
