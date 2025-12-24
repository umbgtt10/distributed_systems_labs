use map_reduce_core::map_reduce_problem::MapReduceProblem;
use map_reduce_core::state_access::StateAccess;
use std::collections::HashMap;

/// Word search problem definition - searches for target words in text data
pub struct WordSearchProblem;

/// Map assignment: chunk of data with target words to search for
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct MapWorkAssignment {
    pub chunk_id: usize,
    pub data: Vec<String>,
    pub targets: Vec<String>,
}

/// Reduce assignment: keys to aggregate
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct ReduceWorkAssignment {
    pub keys: Vec<String>,
}

/// Problem context: list of target words to search for
#[derive(Clone)]
pub struct WordSearchContext {
    pub targets: Vec<String>,
}

impl MapReduceProblem for WordSearchProblem {
    type Input = Vec<String>;
    type MapAssignment = MapWorkAssignment;
    type ReduceAssignment = ReduceWorkAssignment;
    type Context = WordSearchContext;

    fn create_map_assignments(
        data: Self::Input,
        context: Self::Context,
        partition_size: usize,
    ) -> Vec<Self::MapAssignment> {
        let num_partitions = data.len().div_ceil(partition_size);
        let mut data_chunks = Vec::new();

        for i in 0..num_partitions {
            let start = i * partition_size;
            let end = std::cmp::min(start + partition_size, data.len());
            data_chunks.push(data[start..end].to_vec());
        }

        data_chunks
            .into_iter()
            .enumerate()
            .map(|(chunk_id, data)| MapWorkAssignment {
                chunk_id,
                data,
                targets: context.targets.clone(),
            })
            .collect()
    }

    fn create_reduce_assignments(
        context: Self::Context,
        keys_per_reducer: usize,
    ) -> Vec<Self::ReduceAssignment> {
        let targets = context.targets;
        let num_key_partitions = targets.len().div_ceil(keys_per_reducer);
        let mut key_partitions = Vec::new();

        for partition_id in 0..num_key_partitions {
            let start = partition_id * keys_per_reducer;
            let end = std::cmp::min(start + keys_per_reducer, targets.len());
            key_partitions.push(targets[start..end].to_vec());
        }

        key_partitions
            .into_iter()
            .map(|keys| ReduceWorkAssignment { keys })
            .collect()
    }

    fn map_work<S>(assignment: &Self::MapAssignment, state: &S)
    where
        S: StateAccess,
    {
        let results = map_logic(&assignment.data, &assignment.targets);

        // Write results to shared state
        for (key, value) in results {
            if value > 0 {
                state.update(key, value);
            }
        }
    }

    fn reduce_work<S>(assignment: &Self::ReduceAssignment, state: &S)
    where
        S: StateAccess,
    {
        for key in &assignment.keys {
            let values = state.get(key);
            let sum: i32 = values.iter().sum();
            state.replace(key.clone(), sum);
        }
    }
}

/// Pure business logic for mapping phase
/// Searches for target words in data and returns counts
fn map_logic(data: &[String], targets: &[String]) -> HashMap<String, i32> {
    let mut results = HashMap::new();

    for target in targets {
        let mut count = 0;
        for text in data {
            if text.contains(target) {
                count += 1;
            }
        }
        results.insert(target.clone(), count);
    }

    results
}
