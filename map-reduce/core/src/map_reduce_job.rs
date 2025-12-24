use crate::state_access::StateAccess;

/// Trait that defines a specific MapReduce job
/// Abstracts the job domain from the execution model
pub trait MapReduceJob: Send + 'static {
    /// The input data type for the map phase
    type Input: Send;

    /// The assignment type for mappers
    type MapAssignment: Send + Clone;

    /// The assignment type for reducers
    type ReduceAssignment: Send + Clone;

    /// Problem-specific context (e.g., search targets, configuration)
    type Context: Clone + Send;

    /// Create map assignments from input data
    fn create_map_assignments(
        data: Self::Input,
        context: Self::Context,
        partition_size: usize,
    ) -> Vec<Self::MapAssignment>;

    /// Create reduce assignments from context
    fn create_reduce_assignments(
        context: Self::Context,
        keys_per_reducer: usize,
    ) -> Vec<Self::ReduceAssignment>;

    /// Execute map work for a given assignment
    fn map_work<S>(assignment: &Self::MapAssignment, state: &S)
    where
        S: StateAccess;

    /// Execute reduce work for a given assignment
    fn reduce_work<S>(assignment: &Self::ReduceAssignment, state: &S)
    where
        S: StateAccess;
}
