use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Reducer assignment - which keys this reducer is responsible for
pub struct ReducerAssignment {
    pub keys: Vec<String>,
}

/// Reducer worker that sums up vectors into final counts
pub struct Reducer {
    id: usize,
    shared_map: Arc<Mutex<HashMap<String, Vec<i32>>>>,
}

impl Reducer {
    pub fn new(id: usize, shared_map: Arc<Mutex<HashMap<String, Vec<i32>>>>) -> Self {
        Self { id, shared_map }
    }

    /// Spawns a task that reduces values for assigned keys
    pub async fn reduce(self, assignment: ReducerAssignment) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            if self.id.is_multiple_of(2) {
                println!(
                    "Reducer {} started for {} keys",
                    self.id,
                    assignment.keys.len()
                );
            }

            for key in assignment.keys {
                // Get the vector for this key and sum it
                let count = {
                    let map = self.shared_map.lock().unwrap();
                    if let Some(vec) = map.get(&key) {
                        vec.iter().sum::<i32>()
                    } else {
                        0
                    }
                };

                // Update the shared map with the final count
                // We replace Vec<i32> with the summed count by storing it as a single-element vec
                let mut map = self.shared_map.lock().unwrap();
                map.insert(key.clone(), vec![count]);
            }

            if self.id.is_multiple_of(2) {
                println!("Reducer {} finished", self.id);
            }
        })
    }
}
