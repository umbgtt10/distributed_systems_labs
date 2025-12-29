# task-channels - In-Process Async Implementation

**[← Back to MapReduce README](../README.md)**

This implementation uses **Tokio async tasks** with **mpsc channels** for communication. All workers run as lightweight tasks within a single process, providing the **fastest execution** with zero serialization overhead.

---

## Overview

**Execution Model**: Tokio async tasks
**Communication**: `tokio::sync::mpsc` channels (in-memory)
**State Storage**: `LocalStateAccess` (shared HashMap)
**Isolation**: None (single process, shared memory)
**Performance**: ⭐⭐⭐ Fastest (0.75-1.51s)

---

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    Main Async Task                           │
│                   (Coordinator/Executor)                     │
└────────────┬────────────────────────────┬───────────────────┘
             │                            │
    ┌────────▼────────┐          ┌────────▼────────┐
    │ mpsc channel    │          │ mpsc channel    │
    │  (work queue)   │          │  (work queue)   │
    └────────┬────────┘          └────────┬────────┘
             │                            │
       ┌─────▼──────┐              ┌─────▼──────┐
       │   Mapper   │              │   Reducer  │
       │   Task 1   │              │   Task 1   │
       └─────┬──────┘              └─────┬──────┘
             │                            │
       ┌─────▼──────┐              ┌─────▼──────┐
       │   Mapper   │              │   Reducer  │
       │   Task N   │              │   Task N   │
       └─────┬──────┘              └─────┬──────┘
             │                            │
             └──────────┬─────────────────┘
                        │
               ┌────────▼────────┐
               │ LocalStateAccess│
               │   (Arc<Mutex>)  │
               └─────────────────┘
```

---

## Key Components

### 1. `ChannelWorkSender` - Work Distribution

Uses Tokio's bounded `mpsc::channel` for work distribution with backpressure support.

**Characteristics**:
- **Zero-copy**: No serialization, passes data directly
- **Bounded**: Configurable buffer size prevents unbounded memory growth
- **Fast**: In-memory queue, ~nanosecond latency

**Distributed System Trade-off**:
While extremely fast, this approach is **limited to a single machine**. In a real distributed system, work distribution requires network serialization (like TCP or RPC), which introduces latency but allows scaling across multiple nodes.

---

### 2. `ChannelWorkerSynchronization` - Completion Notifications

Uses Tokio's `select!` with `FusedFuture` to multiplex completion signals from multiple workers. Each worker has a dedicated channel that can be reset independently for fault tolerance.

**Characteristics**:
- **Non-blocking**: Async wait for completions
- **Per-worker channels**: Independent failure recovery
- **Type-safe**: Rust's type system prevents wrong usage

---

### 3. `TokioRuntime` - Task Spawning

Spawns lightweight Tokio tasks using `tokio::spawn`.

**Characteristics**:
- **Lightweight**: Tasks are ~2KB each, thousands can run concurrently
- **Cooperative**: Tasks yield at `.await` points
- **Work-stealing**: Tokio's scheduler load-balances across CPU cores

---

### 4. `MapperFactory` / `ReducerFactory` - Worker Creation

Factory pattern for creating configured workers.

**Characteristics**:
- **Dependency injection**: State and config passed at construction
- **Fault injection**: Configurable failure/straggler rates
- **Type-safe**: Generic over problem type `P`

---

## Advantages

### ✅ Performance

- **No serialization**: Data passed by reference/move
- **No IPC overhead**: Everything in same address space
- **Async efficiency**: Minimal context switching, work-stealing scheduler
- **Fastest implementation**: Typically 0.75-1.51s for stress test

### ✅ Simplicity

- **Minimal code**: Fewest lines of any implementation
- **No complex protocols**: Just Rust async/await
- **No external dependencies**: Only Tokio (already used by core)

### ✅ Development Experience

- **Fast compile times**: No additional network code
- **Easy debugging**: Single process, standard debugger works
- **Clear error messages**: Rust's native async error handling

---

## Disadvantages

### ❌ No Fault Isolation

If any worker **panics**, it can:
- Poison shared state (Mutex)
- Crash the entire process
- Leave work incomplete

**Mitigation**: Use `catch_unwind` in worker tasks (implemented in `MapperTask`/`ReducerTask`)

### ❌ Not Distributed

All workers must run on:
- Same machine
- Same process
- Same memory space

**Not suitable for**:
- True distributed systems
- Worker fault tolerance testing
- Cross-machine deployments

### ❌ Limited Realism

Doesn't model:
- Network latency
- Serialization costs
- Process crashes
- Cross-machine coordination

---

## Configuration

**File**: `config.json`

```json
{
  "num_strings": 500000,
  "max_string_length": 15,
  "num_target_words": 100,
  "target_word_length": 3,
  "partition_size": 5000,
  "keys_per_reducer": 5,
  "num_mappers": 15,
  "num_reducers": 10,
  "mapper_failure_probability": 2,
  "reducer_failure_probability": 2,
  "mapper_straggler_probability": 2,
  "reducer_straggler_probability": 2,
  "mapper_straggler_delay_ms": 2000,
  "reducer_straggler_delay_ms": 2000,
  "mapper_timeout_ms": 500,
  "reducer_timeout_ms": 1000
}
```

**Key Settings**:
- `num_strings` - Number of random strings to generate
- `partition_size` - Strings per mapper assignment
- `keys_per_reducer` - Keys per reducer assignment
- `num_mappers` / `num_reducers` - Number of concurrent tasks
- `mapper_timeout_ms` / `reducer_timeout_ms` - Straggler detection threshold
- `mapper_failure_probability` / `reducer_failure_probability` - Percent chance of worker failure
- `mapper_straggler_probability` / `reducer_straggler_probability` - Percent chance of slow worker

---

## Running the Implementation

### Basic Run

```bash
cd task-channels
cargo run
```

### With Custom Data

```bash
# Place .txt files in data/
cargo run
```

---

## Testing Fault Tolerance

Even in single-process mode, you can test fault tolerance:

```json
{
  "mapper_failure_probability": 20,
  "reducer_failure_probability": 20,
  "mapper_straggler_probability": 10,
  "reducer_straggler_probability": 10,
  "mapper_straggler_delay_ms": 3000,
  "reducer_straggler_delay_ms": 3000
}
```

**Behavior**:
- Failed workers: Executor detects failure, spawns replacement
- Straggler workers: Executor times out, spawns backup worker
- Both complete same work, state merges results

---

## Performance Characteristics

**Performance Factors**:
- ✅ Zero serialization overhead
- ✅ In-memory channels (nanosecond latency)
- ✅ Work-stealing scheduler
- ✅ Shared state (no RPC)
- ⚠️ Contention on `Arc<Mutex<HashMap>>` under high load

### Scaling Characteristics

- **More workers**: Linear speedup until CPU saturation
- **Larger data**: Constant per-item cost
- **Memory usage**: Low (tasks are ~2KB each, state is shared)

---

## Code Organization

```
task-channels/
├── src/
│   ├── main.rs                           # Entry point
│   ├── mapper.rs                         # Mapper implementation
│   ├── reducer.rs                        # Reducer implementation
│   ├── mpsc_work_channel.rs              # Channel-based work distribution
│   ├── channel_wrappers.rs               # WorkReceiver/CompletionSender wrappers
│   ├── channel_completion_signaling.rs   # Channel-based completion
│   └── tokio_runtime.rs                  # Tokio task spawning
├── config.json                           # Configuration
├── data/                                 # Text files (not in repo)
└── Cargo.toml
```

---

## Key Takeaways

### When to Use

✅ **Perfect for**:
- Development and testing
- Single-machine workloads
- Maximum performance
- Learning async Rust

❌ **Not suitable for**:
- Distributed systems
- Fault isolation requirements
- Cross-language interop
- Realistic failure testing

### Design Lessons

1. **Async is fast**: Zero-cost abstractions make this the fastest implementation
2. **Channels are simple**: Tokio's mpsc makes coordination trivial
3. **Shared state works**: `Arc<Mutex<HashMap>>` is sufficient for moderate contention
4. **Type safety matters**: Generic traits catch errors at compile time

---

## Next Steps

- **[Compare with thread-socket](../thread-socket/README.md)** - See TCP socket implementation
- **[Compare with process-rpc](../process-rpc/README.md)** - See multi-process RPC implementation
- **[Explore core traits](../core/README.md)** - Understand the abstraction layer
- **[Back to Main README](../README.md)**
