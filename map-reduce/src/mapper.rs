use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use crate::worker::Worker;

/// Work assignment for a mapper - describes what chunk to process
#[derive(Clone)]
pub struct WorkAssignment {
    pub chunk_id: usize,
    pub data: Vec<String>,
    pub targets: Vec<String>,
}

/// Mapper worker that searches for target words in its data chunk
pub struct Mapper {
    work_tx: mpsc::Sender<(WorkAssignment, mpsc::Sender<usize>)>,
    task_handle: JoinHandle<()>,
}

impl Mapper {
    pub fn new(
        id: usize,
        shared_map: Arc<Mutex<HashMap<String, Vec<i32>>>>,
        cancel_token: CancellationToken,
    ) -> Self {
        let (work_tx, work_rx) = mpsc::channel::<(WorkAssignment, mpsc::Sender<usize>)>(10);

        let handle = tokio::spawn(Self::run_task(id, work_rx, shared_map, cancel_token));

        Self {
            work_tx,
            task_handle: handle,
        }
    }

    /// Sends a work assignment to the mapper
    pub fn send_map_assignment(
        &self,
        assignment: WorkAssignment,
        complete_tx: mpsc::Sender<usize>,
    ) {
        // Send work to the mapper task
        let _ = self.work_tx.try_send((assignment, complete_tx));
    }

    /// Waits for the mapper task to complete
    pub async fn wait(self) -> Result<(), tokio::task::JoinError> {
        drop(self.work_tx); // Close the channel to signal task to exit
        self.task_handle.await
    }

    async fn run_task(
        id: usize,
        mut work_rx: mpsc::Receiver<(WorkAssignment, mpsc::Sender<usize>)>,
        shared_map: Arc<Mutex<HashMap<String, Vec<i32>>>>,
        cancel_token: CancellationToken,
    ) {
        loop {
            tokio::select! {
                work = work_rx.recv() => {
                    match work {
                        Some((assignment, complete_tx)) => {
                            if id.is_multiple_of(10) {
                                println!("Mapper {} processing chunk {}", id, assignment.chunk_id);
                            }

                            // Process each string in the chunk
                            for text in assignment.data {
                                // Check for cancellation
                                if cancel_token.is_cancelled() {
                                    println!("Mapper {} cancelled", id);
                                    return;
                                }

                                // Search for each target word in the text
                                for target in &assignment.targets {
                                    if text.contains(target.as_str()) {
                                        // Found a match! Add 1 to the vector for this target
                                        let mut map = shared_map.lock().unwrap();
                                        if let Some(vec) = map.get_mut(target) {
                                            vec.push(1);
                                        }
                                    }
                                }
                            }

                            if id.is_multiple_of(10) {
                                println!("Mapper {} finished chunk {}", id, assignment.chunk_id);
                            }

                            // Notify orchestrator that this mapper is done
                            let _ = complete_tx.send(id).await;
                        }
                        None => {
                            // Channel closed, exit
                            break;
                        }
                    }
                }
                _ = cancel_token.cancelled() => {
                    println!("Mapper {} cancelled", id);
                    break;
                }
            }
        }
    }
}

impl Worker for Mapper {
    type Assignment = WorkAssignment;
    type Completion = mpsc::Sender<usize>;
    type Error = tokio::task::JoinError;

    fn send_work(&self, assignment: Self::Assignment, complete_tx: Self::Completion) {
        self.send_map_assignment(assignment, complete_tx);
    }

    async fn wait(self) -> Result<(), Self::Error> {
        Mapper::wait(self).await
    }
}
