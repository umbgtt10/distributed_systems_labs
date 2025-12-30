// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

/// Trait for abstracting work distribution to workers
/// Different implementations for mpsc, sockets, RPC, etc.
pub trait WorkSender<A, C>: Clone + Send + 'static {
    /// Send initialization sender to worker
    fn initialize(&self, sender: C);

    /// Send work assignment with completion sender
    fn send_work(&self, assignment: A, completion: C);
}

