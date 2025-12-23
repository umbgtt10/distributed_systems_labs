use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio_util::sync::CancellationToken;

/// Work assignment for a mapper - describes what chunk to process
#[derive(Clone)]
pub struct WorkAssignment {
    pub chunk_id: usize,
    pub data: Vec<String>,
    pub targets: Vec<String>,
}

/// Mapper worker that searches for target words in its data chunk
pub struct Mapper {
    id: usize,
    shared_map: Arc<Mutex<HashMap<String, Vec<i32>>>>,
    cancel_token: CancellationToken,
}

impl Mapper {
    pub fn new(
        id: usize,
        shared_map: Arc<Mutex<HashMap<String, Vec<i32>>>>,
        cancel_token: CancellationToken,
    ) -> Self {
        Self {
            id,
            shared_map,
            cancel_token,
        }
    }

    /// Spawns a task that processes its assigned data chunk
    pub fn start(self, assignment: WorkAssignment) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            if self.id.is_multiple_of(10) {
                println!(
                    "Mapper {} processing {} items from chunk {}",
                    self.id,
                    assignment.data.len(),
                    assignment.chunk_id
                );
            }

            // Process each string in the chunk
            for text in assignment.data {
                // Check for cancellation
                if self.cancel_token.is_cancelled() {
                    println!("Mapper {} cancelled", self.id);
                    return;
                }

                // Search for each target word in the text
                for target in &assignment.targets {
                    if text.contains(target.as_str()) {
                        // Found a match! Add 1 to the vector for this target
                        let mut map = self.shared_map.lock().unwrap();
                        if let Some(vec) = map.get_mut(target) {
                            vec.push(1);
                        }
                    }
                }
            }

            if self.id.is_multiple_of(10) {
                println!("Mapper {} finished chunk {}", self.id, assignment.chunk_id);
            }
        })
    }
}
