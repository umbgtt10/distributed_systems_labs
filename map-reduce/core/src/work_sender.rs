/// Trait for abstracting work distribution to workers
/// Different implementations for mpsc, sockets, RPC, etc.
pub trait WorkSender<A, C>: Clone + Send + 'static {
    /// Send initialization token to worker
    fn initialize(&self, token: C);

    /// Send work assignment with completion token
    fn send_work(&self, assignment: A, completion: C);
}
