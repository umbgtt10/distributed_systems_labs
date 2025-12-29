# Core Abstractions

**[← Back to MapReduce README](../README.md)**

The `core` crate defines the trait boundaries that enable pluggable infrastructure. By programming to these interfaces, the MapReduce coordinator and worker logic remain independent of the underlying execution model.

---

## Purpose

This crate provides:
- **Trait definitions** that abstract over execution models (async tasks, threads, processes)
- **Common algorithms** (coordinator logic with fault tolerance, worker implementations)
- **Type-safe contracts** enforced at compile time
- **Problem-agnostic logic** - MapReduce mechanics without domain-specific work functions

---

## Key Design Principles

### 1. Abstraction Without Runtime Cost

Rust traits with associated types provide **zero-cost abstractions**:
```rust
pub trait Worker: Send {
    type Assignment: Send;
    type Completion;
    type Error: Display;

    fn send_work(&self, assignment: Self::Assignment, complete_tx: Self::Completion);
    fn wait(self) -> impl Future<Output = Result<(), Self::Error>> + Send;
}
```

At compile time, Rust monomorphizes these traits, generating optimized code for each implementation. There's **no vtable overhead** or dynamic dispatch.

### 2. Separation of Concerns

Each trait addresses a single responsibility:
- **`MapReduceJob`** - What computation to perform
- **`StateAccess`** - How to store/retrieve key-value pairs
- **`WorkSender`** - How to send work to workers
- **`WorkerSynchronization`** - How workers notify completion
- **`WorkerRuntime`** - How to spawn and manage workers

This separation allows **mixing and matching** implementations independently.

### 3. Implementation Agnostic

The `Executor` doesn't care if workers are:
- Tokio tasks communicating via channels
- OS threads communicating via sockets
- Separate processes communicating via RPC

All it requires is that types implement the traits.

---

## Core Traits

### `MapReduceJob`

Defines the business logic for a specific MapReduce problem.

```rust
pub trait MapReduceJob: Send + 'static {
    type Input: Send;
    type MapAssignment: Send + Clone;
    type ReduceAssignment: Send + Clone;
    type Context: Clone + Send;

    fn create_map_assignments(
        data: Self::Input,
        context: Self::Context,
        partition_size: usize,
    ) -> Vec<Self::MapAssignment>;

    fn create_reduce_assignments(
        context: Self::Context,
        keys_per_reducer: usize,
    ) -> Vec<Self::ReduceAssignment>;

    fn map_work<S>(assignment: &Self::MapAssignment, state: &S)
    where
        S: StateAccess;

    fn reduce_work<S>(assignment: &Self::ReduceAssignment, state: &S)
    where
        S: StateAccess;
}
```

**Implementations**: `WordSearchProblem` (in `word-search` crate)

---

### `StateAccess`

Abstracts key-value storage (local HashMap or remote state server).

```rust
pub trait StateAccess: Send + Sync {
    fn get<V: DeserializeOwned>(&self, key: &str) -> Option<V>;
    fn set<V: Serialize>(&self, key: &str, value: V);
    fn update<V, F>(&self, key: &str, default: V, update_fn: F)
    where
        V: Serialize + DeserializeOwned,
        F: FnOnce(&mut V);
}
```

**Implementations**:
- `LocalStateAccess` - In-memory HashMap with `Arc<Mutex<_>>`
- `RpcStateAccess` - TCP client to remote state server (process-rpc)

---

### `Worker`

Represents a worker (mapper or reducer) that can receive work and signal completion.

```rust
pub trait Worker: Send {
    type Assignment: Send;
    type Completion;
    type Error: Display;

    fn send_work(&self, assignment: Self::Assignment, complete_tx: Self::Completion);
    fn wait(self) -> impl Future<Output = Result<(), Self::Error>> + Send;
}
```

**Implementations**: `Mapper<...>` and `Reducer<...>` with various concrete types

---

### `WorkSender`

Abstracts the mechanism for distributing work to workers.

```rust
pub trait WorkSender<A, C>: Clone + Send + 'static {
    fn initialize(&self, sender: C);
    fn send_work(&self, assignment: A, completion: C);
}
```

**Implementations**:
- `ChannelWorkSender` - Tokio mpsc channels (task-channels)
- `SocketWorkSender` - TCP sockets (thread-socket)
- `GrpcWorkSender` - RPC over TCP (process-rpc)

---

### `WorkerSynchronization`

Abstracts how workers signal completion or failure.

```rust
pub trait WorkerSynchronization: Send {
    type StatusSender: Clone + Send;

    fn setup(num_workers: usize) -> Self;
    fn get_status_sender(&self, worker_id: usize) -> Self::StatusSender;
    fn wait_next(&mut self) -> impl Future<Output = Option<Result<usize, usize>>> + Send;
    fn reset_worker(&mut self, worker_id: usize) -> impl Future<Output = Self::StatusSender> + Send;
}
```

**Implementations**:
- `ChannelWorkerSynchronization` - Tokio channels (task-channels)
- `SocketWorkerSynchronization` - TCP listener (thread-socket)
- `GrpcWorkerSynchronization` - RPC completion tokens (process-rpc)

---

### `WorkerRuntime`

Abstracts how workers are spawned (tasks, threads, processes).

```rust
pub trait WorkerRuntime<Task>: Send + 'static {
    type Handle: Send;
    type Error: Display + Send;

    fn spawn(task: Task) -> Self::Handle;
    fn join(handle: Self::Handle) -> impl Future<Output = Result<(), Self::Error>> + Send;
}
```

**Implementations**:
- `TokioRuntime` - Spawns Tokio tasks (task-channels)
- `ThreadRuntime` - Spawns OS threads (thread-socket)
- `ProcessRuntime` - Spawns OS processes (process-rpc)

---

### `WorkerFactory`

Creates workers with specific configurations.

```rust
pub trait WorkerFactory<W>: Send {
    fn create_worker(&mut self, id: usize) -> W;
}
```

**Implementations**: Various factory structs in each implementation crate

---

## Common Algorithms

### `Executor` - Fault-Tolerant Phase Execution

The `Executor` implements the core MapReduce coordinator logic with:
- **Fault tolerance**: Restarts failed workers
- **Straggler detection**: Detects slow workers via timeout
- **Work assignment tracking**: Reassigns work from failed/slow workers
- **Graceful shutdown**: Responds to shutdown signals

```rust
pub struct Executor<W, CS, F>
where
    W: Worker,
    CS: WorkerSynchronization,
    F: WorkerFactory<W>,
{
    // ...
}

impl<W, CS, F> Executor<W, CS, F> {
    pub async fn execute<SD>(
        &mut self,
        mut workers: Vec<W>,
        assignments: Vec<W::Assignment>,
        shutdown_signal: &SD,
    ) -> Vec<W>
    where
        SD: ShutdownSignal + Sync,
        W::Assignment: Clone,
    {
        // Coordinator algorithm with fault tolerance
    }
}
```

**Key Features**:
- Distributes work to N workers
- Waits for completions using `WorkerSynchronization`
- Detects failures via completion signals
- Detects stragglers via configurable timeout
- Reassigns work to new workers
- Handles shutdown gracefully

---

## Usage Example

The beauty of this design is that **the same high-level code** works with any implementation:

```rust
// 1. Define your problem (business logic)
impl MapReduceJob for WordSearchProblem {
    fn map_work<S>(assignment: &MapAssignment, state: &S) { /* ... */ }
    fn reduce_work<S>(assignment: &ReduceAssignment, state: &S) { /* ... */ }
}

// 2. Choose your infrastructure (pick traits)
let state = LocalStateAccess::new();           // or RpcStateAccess
let shutdown = Arc::new(AtomicBool::new(false));
let mapper_factory = MapperFactory::new(/* ... */);
let reducer_factory = ReducerFactory::new(/* ... */);

// 3. Execute (same code for all implementations!)
let mut map_executor = Executor::new(mapper_factory, timeout_ms);
let mappers = map_executor.execute(mappers, map_assignments, &shutdown).await;

let mut reduce_executor = Executor::new(reducer_factory, timeout_ms);
let reducers = reduce_executor.execute(reducers, reduce_assignments, &shutdown).await;
```

**The implementation details are hidden behind the traits.**

---

## Architecture Benefits

### Compile-Time Guarantees

Wrong implementations are caught at compile time:
```rust
// ❌ Compile error: SocketWorkChannel doesn't implement Send
// ❌ Compile error: Wrong assignment type
// ✅ Type safety enforced by traits
```

### Testability

Mock implementations for testing:
```rust
struct MockStateAccess { /* ... */ }
impl StateAccess for MockStateAccess { /* ... */ }

// Test business logic with mock state
WordSearchProblem::map_work(&assignment, &MockStateAccess);
```

### Independent Evolution

Change one layer without affecting others:
- Add a new `WorkDistributor` implementation (e.g., gRPC)
- Add a new `StateAccess` implementation (e.g., Redis)
- Add a new `MapReduceJob` (e.g., inverted index)

**The coordinator and worker logic remain unchanged.**

---

## Project Structure

```
core/
├── src/
│   ├── lib.rs                     # Module exports
│   ├── map_reduce_job.rs          # Problem definition trait
│   ├── state_access.rs            # Storage abstraction
│   ├── local_state_access.rs      # In-memory implementation
│   ├── worker.rs                  # Worker trait
│   ├── worker_factory.rs          # Worker creation trait
│   ├── worker_runtime.rs          # Execution model traits
│   ├── work_channel.rs            # Work distribution traits
│   ├── worker_io.rs               # Worker I/O traits
│   ├── completion_signaling.rs    # Completion notification trait
│   ├── shutdown_signal.rs         # Shutdown coordination trait
│   ├── mapper.rs                  # Mapper implementation
│   ├── reducer.rs                 # Reducer implementation
│   ├── executor.rs                # Fault-tolerant coordinator
│   ├── utils.rs                   # Helper functions
│   └── config.rs                  # Configuration types
└── Cargo.toml
```

---

## Dependencies

- **`tokio`** - For async runtime (all implementations use Tokio for I/O)
- **`serde`** / **`serde_json`** - For serialization (used by socket/RPC implementations)
- **`async-trait`** - For async trait methods
- **`rand`** - For failure/straggler injection

---

## Next Steps

- **[Explore task-channels](../task-channels/README.md)** - See how these traits enable async task execution
- **[Explore thread-socket](../thread-socket/README.md)** - See how traits enable multi-threaded TCP
- **[Explore process-rpc](../process-rpc/README.md)** - See how traits enable multi-process RPC
- **[Back to Main README](../README.md)**
