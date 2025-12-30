# MapReduce: Pluggable Architecture Demonstration

A Rust adaptation of [MIT 6.824 Lab 1: MapReduce](https://pdos.csail.mit.edu/6.824/labs/lab-mr.html), demonstrating **architectural abstraction** through three working implementations that share identical business logic.

**[← Back to Main README](../README.md)**

---

## What This Project Demonstrates

### MIT's Original Lab
- **One implementation** in Go using Go's RPC
- **Learning goal**: Implement MapReduce correctly
- **Fixed decisions**: Go routines, Go RPC, local state

### This Rust Adaptation
- **Three implementations** with pluggable infrastructure
- **Learning goals**: MapReduce + abstraction design + Rust patterns
- **Pluggable decisions**: Execution model, communication layer, state backend

**Key Innovation**: The same `WordSearchProblem` logic runs unchanged across all three implementations, proving the abstractions work.

---

## Architecture Overview

This workspace uses a **trait-based plugin architecture**:

```
┌─────────────────────────────────────────────────────────────┐
│                    Business Logic (Fixed)                   │
│                   WordSearchProblem trait                   │
│               (same across all implementations)             │
└─────────────────────────────────────────────────────────────┘
                              ▲
                              │
┌─────────────────────────────────────────────────────────────┐
│              Core Abstractions (Trait Boundaries)           │
│   StateAccess | WorkChannel | CompletionSignaling | ...     │
└─────────────────────────────────────────────────────────────┘
                              ▲
                              │
          ┌───────────────────┼─────────────────────┐
          │                   │                     │
    ┌─────▼───────┐    ┌──────▼───────┐      ┌──────▼──────┐
    │task-channels│    │thread-socket │      │process-rpc  │
    │   (Tokio)   │    │  (OS threads)│      │(Multi-proc) │
    └─────────────┘    └──────────────┘      └─────────────┘
```

---

## Project Structure

### [`core/`](core/README.md) - Core Abstractions 🧩

**Role**: Defines the trait boundaries that make infrastructure pluggable

**Key Traits**:
- `MapReduceJob<K, V>` - Business logic contract (map/reduce functions)
- `StateAccess<S, K, V>` - Key-value storage abstraction (local or remote)
- `WorkSender<A, C>` - Work distribution to workers
- `WorkerSynchronization` - Worker completion notifications
- `WorkerRuntime` - Worker lifecycle management

**Why it matters**: These traits enforce compile-time guarantees that implementations satisfy the contract. Changing infrastructure doesn't break the coordinator/worker algorithm.

→ [**Explore Core Abstractions**](core/README.md)

---

### [`task-channels/`](task-channels/README.md) - In-Process Async 🚀

**Execution Model**: Tokio async tasks
**Communication**: `mpsc` channels (memory)
**State**: `LocalStateAccess` (HashMap)
**Isolation**: None (single process)

**Use Cases**:
- ✅ Development and testing
- ✅ High-performance single-machine workloads
- ✅ Learning Tokio async patterns
- ❌ Not suitable for fault isolation

**Performance**: 0.75-1.51s (fastest, no serialization overhead)

→ [**Explore task-channels Implementation**](task-channels/README.md)

---

### [`thread-socket/`](thread-socket/README.md) - Multi-Threaded TCP 🧵

**Execution Model**: OS threads
**Communication**: TCP sockets with JSON serialization
**State**: `LocalStateAccess` (shared via `Arc<Mutex>`)
**Isolation**: Thread-level (shared memory space)

**Use Cases**:
- ✅ Traditional multi-threaded architectures
- ✅ Cross-language compatibility (TCP protocol)
- ✅ Learning Rust threading and synchronization
- ✅ Preparing for distributed deployment
- ⚠️ Limited fault isolation (shared process)

**Performance**: 1.10-2.32s (TCP overhead, JSON serialization)

→ [**Explore thread-socket Implementation**](thread-socket/README.md)

---

### [`process-rpc/`](process-rpc/README.md) - Multi-Process gRPC 🔗

**Execution Model**: Separate OS processes
**Communication**: gRPC (Tonic) over HTTP/2 with Protocol Buffers
**State**: `GrpcStateAccess` (Remote gRPC State Server)
**Isolation**: Process-level (true isolation)

**Use Cases**:
- ✅ True fault isolation (worker crashes don't affect coordinator)
- ✅ Closest to real distributed systems
- ✅ Testing failure injection and recovery
- ✅ Learning gRPC and Protocol Buffers
- ⚠️ Highest overhead (process spawning, Protobuf serialization)

**Performance**: 1.65-6.90s (slowest due to process startup and serialization)

**Advanced Features**:
- Fault injection (forced worker failures)
- Straggler detection (slow worker handling)
- Automatic process cleanup
- Type-safe contract via `.proto` files

→ [**Explore process-rpc Implementation**](process-rpc/README.md)

---

## Quick Start

### Run All Implementations (Stress Test)

```powershell
# Windows - runs 15 tests (5 per implementation)
.\scripts\stress_test.ps1
```

**Expected output**:
```
Testing task-channels implementation...
[task-channels] Test 1/5: PASS
[task-channels] Test 2/5: PASS
...
All 15 tests PASSED! ✓
```

### Run Individual Implementations

```bash
# In-process async (fastest)
cd task-channels
cargo run

# Multi-threaded TCP
cd thread-socket
cargo run

# Multi-process RPC (most realistic)
cd process-rpc
cargo run
```

### Expected Output (All Implementations)

```
Top 20 words:
the: 58970
of: 30237
and: 23121
...

Total words counted: 1107186
Execution time: 1.23s
```

---

## Implementation Comparison

| Feature | task-channels | thread-socket | process-rpc |
|---------|--------------|---------------|-------------|
| **Execution** | Tokio tasks | OS threads | OS processes |
| **Communication** | mpsc channels | TCP sockets | gRPC (HTTP/2) |
| **State Location** | In-memory | In-memory (shared) | Remote server |
| **Serialization** | None | JSON | Protobuf + JSON |
| **Fault Isolation** | None | Thread-level | Process-level |
| **Crash Recovery** | ❌ | ❌ | ✅ |
| **Performance** | ⭐⭐⭐ Fast | ⭐⭐ Moderate | ⭐ Slow |
| **Realism** | Low | Medium | High |
| **Complexity** | Low | Medium | High |

### When to Use Each

**task-channels** → Learning async Rust, single-machine performance
**thread-socket** → Traditional multi-threading, preparing for distribution
**process-rpc** → Real distributed systems, fault tolerance testing

---

## Technical Highlights

### 1. Trait-Based Abstraction

The core abstractions allow **compile-time verification** that implementations satisfy contracts:

```rust
// This works with ANY implementation!
pub async fn run_coordinator<R, W>(
    problem: Arc<dyn MapProblem<String, usize>>,
    runtime: Arc<R>,
    work_channel: Arc<W>,
) -> Result<(), String>
where
    R: WorkerRuntime,
    W: WorkChannel<Assignment, CompletionData>,
{
    // Coordinator logic here - never changes!
}
```

### 2. Zero Business Logic Changes

`WordSearchProblem` is defined **once** in `core/` and used by all implementations:

```rust
impl MapProblem<String, usize> for WordSearchProblem {
    fn map(&self, document: &str) -> Vec<(String, usize)> {
        // Word counting logic - same for all implementations
    }

    fn reduce(&self, values: &[usize]) -> usize {
        // Sum logic - same for all implementations
    }
}
```

### 3. Type-Safe Boundaries

Rust's trait system enforces safety at compile time:

```rust
trait WorkChannel<A, C>: Send + Sync {
    async fn send_work(&self, assignment: A, completion: C)
        -> Result<(), String>;
}
```

- `Send + Sync` bounds prevent data races
- Generic types `A` and `C` allow any serializable data
- `async` requirement enforces non-blocking I/O

### 4. Async-First Architecture (Design Decision)

**The `StateAccess` trait is async** to favor network-based implementations (gRPC, Redis, etc.):

```rust
#[async_trait]
pub trait StateAccess: Clone + Send + Sync + 'static {
    async fn get(&self, key: &str) -> Vec<i32>;
    async fn update(&self, key: String, value: i32);
    // ...
}
```

**Why async?**
- ✅ **gRPC** works naturally with native async/await (no blocking!)
- ✅ **Future-proof** for Redis, Cassandra, network storage
- ⚠️ **Local implementations** pay a small cost (wrapping sync HashMap in async)

**Alternative considered**: Sync trait would keep HashMap simple but force gRPC to use `block_in_place` + `block_on` (expensive and awkward).

**Decision**: Favor distributed systems patterns over in-memory simplicity.

---

## Testing

### Automated Stress Testing

The stress test script validates all three implementations:

```powershell
.\scripts\stress_test.ps1
```

**What it tests**:
- ✅ Correctness (word counts match expected)
- ✅ Stability (15 consecutive runs without crashes)
- ✅ Consistency (all implementations produce same results)

### Manual Testing

```bash
# Test with custom input files
cd task-channels
cargo run -- --input-dir ./custom_data --output-file results.txt
```

---

## Performance Characteristics

**Measured on**: Windows, 5 runs per implementation

| Implementation | Min | Max | Avg |
|----------------|-----|-----|-----|
| task-channels | 0.75s | 1.51s | 1.13s |
| thread-socket | 1.10s | 2.32s | 1.71s |
| process-rpc | 1.65s | 6.90s | 3.40s |

**Observations**:
- `task-channels` is consistently fastest (no serialization)
- `thread-socket` has moderate overhead (JSON serialization)
- `process-rpc` varies most (process startup time)

---

## Educational Value

This project teaches:

1. **Distributed Systems** (from MIT 6.824)
   - MapReduce algorithm
   - Coordinator/worker pattern
   - Fault tolerance concepts

2. **Rust Patterns**
   - Trait-based abstraction
   - Async/await with Tokio
   - Thread synchronization (`Arc`, `Mutex`)
   - Process management
   - Error handling with `Result<T, E>`

3. **Architecture Design**
   - Separation of concerns
   - Dependency injection via traits
   - Pluggable infrastructure
   - Type-safe boundaries

---

## Next Steps

### For Learners

1. **Start with** [`core/README.md`](core/README.md) to understand the abstractions
2. **Read** [`task-channels/README.md`](task-channels/README.md) for the simplest implementation
3. **Compare** the three implementations to see what changes and what stays fixed
4. **Experiment** by modifying parameters (worker count, chunk size, etc.)

### For Contributors

1. **Add a fourth implementation** (e.g., WebSockets, QUIC)
2. **Optimize performance** (better chunking, parallel reduce, etc.)
3. **Add benchmarks** (criterion for micro-benchmarks)
4. **Port other MIT labs** (Raft, KV Service, etc.)

---

## References

- **Original Lab**: [MIT 6.824 Lab 1](https://pdos.csail.mit.edu/6.824/labs/lab-mr.html)
- **MIT Course**: [6.824 Home Page](https://pdos.csail.mit.edu/6.824/)
- **MapReduce Paper**: [Google's MapReduce (2004)](https://research.google/pubs/pub62/)

---

**[← Back to Main README](../README.md)** | **[Explore Core Abstractions →](core/README.md)**
