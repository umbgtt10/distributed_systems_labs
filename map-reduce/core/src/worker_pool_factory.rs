use crate::worker::Worker;

/// Configuration for fault injection in workers
#[derive(Clone, Copy)]
pub struct FaultConfig {
    pub failure_probability: u32,
    pub straggler_probability: u32,
    pub straggler_delay_ms: u64,
}

/// Factory for creating pools of workers and worker factories
///
/// This abstracts the creation of:
/// 1. Initial worker pool (Vec<W>)
/// 2. Factory closure for spawning new workers on demand
///
/// Different implementations can vary:
/// - Communication mechanism (channels vs sockets)
/// - Runtime (async vs threads)
/// - Worker type (mapper vs reducer)
pub trait WorkerPoolFactory: Send + Sync {
    /// The type of worker this factory creates
    type Worker: Worker;

    /// Creates both an initial pool of workers and a factory function for creating new ones
    ///
    /// # Arguments
    /// * `pool_size` - Number of workers to create in the initial pool
    /// * `fault_config` - Configuration for simulating failures and stragglers
    ///
    /// # Returns
    /// A tuple of:
    /// - Vector of initially created workers
    /// - Factory closure that can create new workers on demand (for respawning)
    fn create_pool_and_factory(
        &self,
        pool_size: usize,
        fault_config: FaultConfig,
    ) -> (
        Vec<Self::Worker>,
        Box<dyn FnMut(usize) -> Self::Worker + Send>,
    );
}
