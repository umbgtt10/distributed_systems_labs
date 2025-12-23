use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use crate::worker::Worker;

/// Reducer assignment - which keys this reducer is responsible for
#[derive(Clone)]
pub struct ReducerAssignment {
    pub keys: Vec<String>,
}

/// Reducer worker that sums up vectors into final counts
pub struct Reducer {
    work_tx: mpsc::Sender<(ReducerAssignment, mpsc::Sender<usize>)>,
    task_handle: JoinHandle<()>,
}

impl Reducer {
    pub fn new(
        id: usize,
        shared_map: Arc<Mutex<HashMap<String, Vec<i32>>>>,
        cancel_token: CancellationToken,
    ) -> Self {
        let (work_tx, work_rx) = mpsc::channel::<(ReducerAssignment, mpsc::Sender<usize>)>(10);

        let handle = tokio::spawn(Self::run_task(id, work_rx, shared_map, cancel_token));

        Self {
            work_tx,
            task_handle: handle,
        }
    }

    async fn run_task(
        id: usize,
        mut work_rx: mpsc::Receiver<(ReducerAssignment, mpsc::Sender<usize>)>,
        shared_map: Arc<Mutex<HashMap<String, Vec<i32>>>>,
        cancel_token: CancellationToken,
    ) {
        loop {
            tokio::select! {
                work = work_rx.recv() => {
                    match work {
                        Some((assignment, complete_tx)) => {
                            if id.is_multiple_of(2) {
                                println!("Reducer {} started for {} keys", id, assignment.keys.len());
                            }

                            for key in assignment.keys {
                                // Get the vector for this key and sum it
                                let count = {
                                    let map = shared_map.lock().unwrap();
                                    if let Some(vec) = map.get(&key) {
                                        vec.iter().sum::<i32>()
                                    } else {
                                        0
                                    }
                                };

                                // Update the shared map with the final count
                                let mut map = shared_map.lock().unwrap();
                                map.insert(key.clone(), vec![count]);
                            }

                            if id.is_multiple_of(2) {
                                println!("Reducer {} finished", id);
                            }

                            // Notify orchestrator that this reducer is done
                            let _ = complete_tx.send(id).await;
                        }
                        None => {
                            // Channel closed, exit
                            break;
                        }
                    }
                }
                _ = cancel_token.cancelled() => {
                    println!("Reducer {} cancelled", id);
                    break;
                }
            }
        }
    }

    /// Sends a work assignment to the reducer
    pub fn send_reduce_assignment(
        &self,
        assignment: ReducerAssignment,
        complete_tx: mpsc::Sender<usize>,
    ) {
        // Send work to the reducer task
        let _ = self.work_tx.try_send((assignment, complete_tx));
    }

    /// Waits for the reducer task to complete
    pub async fn wait(self) -> Result<(), tokio::task::JoinError> {
        drop(self.work_tx); // Close the channel to signal task to exit
        self.task_handle.await
    }
}

impl Worker for Reducer {
    type Assignment = ReducerAssignment;
    type Completion = mpsc::Sender<usize>;
    type Error = tokio::task::JoinError;

    fn send_work(&self, assignment: Self::Assignment, complete_tx: Self::Completion) {
        self.send_reduce_assignment(assignment, complete_tx);
    }

    async fn wait(self) -> Result<(), Self::Error> {
        Reducer::wait(self).await
    }
}
