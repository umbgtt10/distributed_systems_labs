use serde::{Deserialize, Serialize};

/// Message types received by workers
#[derive(Serialize, Deserialize, Debug)]
pub enum WorkerMessage<A, C> {
    /// Initialization message containing the synchronization token
    Initialize(C),
    /// Work assignment
    Work(A, C),
}
