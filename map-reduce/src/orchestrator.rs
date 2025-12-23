use crate::mapper::WorkAssignment;
use crate::reducer::ReducerAssignment;
use crate::work_distributor::WorkDistributor;

/// Orchestrator coordinates the map-reduce workflow
pub struct Orchestrator<MD: WorkDistributor, RD: WorkDistributor> {
    mapper_distributor: MD,
    reducer_distributor: RD,
}

impl<MD: WorkDistributor, RD: WorkDistributor> Orchestrator<MD, RD> {
    pub fn new(mapper_distributor: MD, reducer_distributor: RD) -> Self {
        Self {
            mapper_distributor,
            reducer_distributor,
        }
    }

    /// Runs the complete map-reduce workflow
    pub async fn run(
        mut self,
        mappers: Vec<MD::Worker>,
        reducers: Vec<RD::Worker>,
        data_chunks: Vec<Vec<String>>,
        targets: Vec<String>,
        keys_per_reducer: usize,
    ) where
        MD::Worker: crate::worker::Worker<Assignment = WorkAssignment>,
        RD::Worker: crate::worker::Worker<Assignment = ReducerAssignment>,
    {
        println!("=== ORCHESTRATOR STARTED ===");

        // MAP PHASE - Distribute work to mappers
        println!("\n=== MAP PHASE ===");
        println!(
            "Distributing {} chunks to {} mappers...",
            data_chunks.len(),
            mappers.len()
        );

        // Create mapper assignments
        let mapper_assignments: Vec<WorkAssignment> = data_chunks
            .into_iter()
            .enumerate()
            .map(|(chunk_id, data)| WorkAssignment {
                chunk_id,
                data,
                targets: targets.clone(),
            })
            .collect();

        // Distribute work to mappers
        self.mapper_distributor
            .distribute(mappers, mapper_assignments)
            .await;
        println!("All mappers completed!");

        // REDUCE PHASE - Assign work to reducers
        println!("\n=== REDUCE PHASE ===");
        println!("Starting {} reducers...", reducers.len());

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
            reducers.len()
        );

        // Create reducer assignments
        let reducer_assignments: Vec<ReducerAssignment> = key_partitions
            .into_iter()
            .map(|keys| ReducerAssignment { keys })
            .collect();

        // Distribute work to reducers
        self.reducer_distributor
            .distribute(reducers, reducer_assignments)
            .await;
        println!("All reducers completed!");

        println!("\n=== ORCHESTRATOR FINISHED ===");
    }
}
