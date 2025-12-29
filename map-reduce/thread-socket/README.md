# thread-socket - Multi-Threaded TCP Implementation

**[← Back to MapReduce README](../README.md)**

This implementation uses **OS threads** with **TCP sockets** for communication. Workers run as separate threads, communicating via TCP with JSON serialization, providing a **realistic middle ground** between async tasks and separate processes.

---

## Overview

**Execution Model**: OS threads (one per worker)
**Communication**: TCP sockets with JSON serialization
**State Storage**: `LocalStateAccess` (shared HashMap via `Arc<Mutex>`)
**Isolation**: Thread-level (shared memory, separate stacks)
**Performance**: ⭐⭐ Moderate (1.10-2.32s)

---

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                   Main Thread (Coordinator)                  │
│                        Executor                              │
└────────┬───────────────────────────────┬────────────────────┘
         │                               │
    ┌────▼─────┐                    ┌────▼─────┐
    │ TCP      │                    │ TCP      │
    │ Listener │                    │ Listener │
    │ :8001    │                    │ :8002    │
    └────┬─────┘                    └────┬─────┘
         │                               │
    ┌────▼──────┐                   ┌────▼──────┐
    │ OS Thread │                   │ OS Thread │
    │  Mapper 1 │                   │ Reducer 1 │
    │           │                   │           │
    │ [connect] │                   │ [connect] │
    └────┬──────┘                   └────┬──────┘
         │                               │
    ┌────▼──────┐                   ┌────▼──────┐
    │ OS Thread │                   │ OS Thread │
    │  Mapper N │                   │ Reducer N │
    └────┬──────┘                   └────┬──────┘
         │                               │
         └───────────┬───────────────────┘
                     │
            ┌────────▼─────────┐
            │ LocalStateAccess │
            │  Arc<Mutex<_>>   │
            └──────────────────┘
```

---

## Key Components

### 1. `SocketWorkSender` - TCP-Based Work Distribution

Implements TCP-based work distribution using length-prefixed JSON messages. Each worker connects to a dynamically-assigned TCP port to receive work assignments.

**Protocol**:
```
Coordinator → Worker (TCP): [4-byte length][JSON assignment]
```

**Characteristics**:
- **JSON serialization**: Human-readable, debuggable
- **Length-prefixed**: `u32` length + JSON payload
- **Async I/O**: Uses Tokio for non-blocking operations
- **One connection per worker**: Coordinator listens, workers connect

**Distributed System Trade-off**:
This introduces **serialization overhead** and **network stack latency**, mimicking a real distributed system. However, since it runs on `localhost`, it avoids the unpredictability of a real network (packet loss, variable latency).

---

### 2. `SocketWorkerSynchronization` - TCP Completion Listener

Workers signal completion by connecting to the coordinator's completion listener and sending a length-prefixed JSON message containing worker ID and success/failure status.

**Protocol**:
```
Worker → Coordinator (TCP): [4-byte length][JSON: {"Success": worker_id} or {"Failure": worker_id}]
```

**Characteristics**:
- **Async accept**: Non-blocking using Tokio
- **JSON protocol**: Structured `CompletionMessage` enum
- **Reliable**: TCP guarantees delivery

---

### 3. `ThreadRuntime` - OS Thread Spawning

Spawns OS threads, each with its own Tokio runtime for async TCP operations.

**Characteristics**:
- **OS-level isolation**: Separate stack per thread
- **Heavier than tasks**: ~2MB stack per thread vs ~2KB per task
- **Panics contained**: Thread panic doesn't affect others (but state may be poisoned)
- **Each thread has Tokio runtime**: For async TCP I/O

---

### 4. `MapperFactory` / `ReducerFactory` - Thread Worker Creation

**Characteristics**:
- **Socket addresses**: Each worker gets unique TCP port
- **Shared state**: `Arc<Mutex<HashMap>>` cloned to each thread
- **Fault injection**: Configurable failure/straggler probabilities

---

## Advantages

### ✅ Realistic Threading Model

- **OS-level scheduling**: True parallelism on multi-core
- **Real synchronization**: Actual `Mutex`/`Arc` patterns
- **Similar to production**: Many systems use threaded architectures

### ✅ TCP Protocol Experience

- **Network programming**: Real socket programming
- **Serialization**: JSON serialization/deserialization
- **Debugging tools**: Can use Wireshark, tcpdump, netstat
- **Language agnostic**: Workers could be in any language

### ✅ Moderate Fault Isolation

- **Thread panics**: Don't crash entire process (if handled)
- **Separate stacks**: Stack overflow in one thread doesn't affect others
- **Better than async tasks**: More isolation than single-process

### ✅ Stepping Stone

- **Prepares for distribution**: Same TCP protocol could work across machines
- **Learning path**: From async → threads → processes

---

## Disadvantages

### ❌ Serialization Overhead

- **JSON encoding/decoding**: ~10-50μs per message
- **Copy overhead**: Data copied multiple times (app → kernel → network → kernel → app)
- **Slower than channels**: 10-100x slower than in-memory channels

### ❌ Limited Fault Isolation

- **Shared memory**: All threads in same process
- **Poisoned locks**: Panic while holding `Mutex` poisons it for all threads
- **Process crash**: Any thread with `abort()` kills everyone

### ❌ Resource Usage

- **Stack memory**: ~2MB per thread (default on Windows)
- **OS limits**: Typically max 1000-2000 threads per process
- **Context switching**: More expensive than async task switching

### ❌ Complexity

- **More code**: TCP protocol handling, serialization
- **More failure modes**: Socket errors, connection drops, timeouts
- **Harder debugging**: Multi-threaded bugs (race conditions, deadlocks)

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
- `num_mappers` / `num_reducers` - Number of OS threads
- `mapper_timeout_ms` / `reducer_timeout_ms` - Socket timeout + straggler detection
- Fault injection rates per mapper/reducer

**Port Allocation**:
- Work channels: Dynamic ports (OS assigns)
- Completion listener: Dynamic port (OS assigns)
- Addresses passed to workers during creation

---

## Running the Implementation

### Basic Run

```bash
cd thread-socket
cargo run
```

### Monitoring Connections

```bash
# In another terminal (while running)
netstat -an | grep ESTABLISHED | grep 127.0.0.1
```

You'll see connections like:
```
TCP    127.0.0.1:8001    127.0.0.1:52341    ESTABLISHED  # Mapper 1
TCP    127.0.0.1:8002    127.0.0.1:52342    ESTABLISHED  # Mapper 2
...
```

---

## Testing Fault Tolerance

### Worker Failures

```json
{
  "mapper_failure_probability": 20,
  "reducer_failure_probability": 20,
  "mapper_timeout_ms": 3000,
  "reducer_timeout_ms": 3000
}
```

**Behavior**:
- Worker thread panics (simulated crash)
- Completion listener times out
- Executor spawns replacement thread
- Work is reassigned

### Worker Stragglers

```json
{
  "mapper_straggler_probability": 15,
  "reducer_straggler_probability": 15,
  "mapper_straggler_delay_ms": 5000,
  "reducer_straggler_delay_ms": 5000,
  "mapper_timeout_ms": 3000,
  "reducer_timeout_ms": 3000
}
```

**Behavior**:
- Worker sleeps for 5 seconds
- Executor times out after 3 seconds
- Backup worker spawned (redundant execution)
- First to complete wins

---

## Performance Characteristics

**Performance Factors**:
- ❌ JSON serialization overhead (~10-50μs per message)
- ❌ TCP loopback latency (~10-100μs)
- ❌ Thread context switching
- ✅ True parallelism across cores
- ✅ No network stack (loopback optimized)

### Comparison to task-channels

- **~1.6x slower** on average
- **More consistent times** (less variance)
- **Scales similarly** with more workers

---

## Code Organization

```
thread-socket/
├── src/
│   ├── main.rs                        # Entry point
│   ├── mapper.rs                      # Mapper implementation
│   ├── reducer.rs                     # Reducer implementation
│   ├── socket_work_channel.rs         # TCP-based work distribution
│   ├── socket_completion_signaling.rs # TCP-based completion
│   └── thread_runtime.rs              # OS thread spawning
├── config.json                        # Configuration
├── data/                              # Text files (not in repo)
└── Cargo.toml
```

---

## Protocol Details

### Work Assignment Protocol

```
┌─────────────────────────────────────┐
│ Message Format (Work Assignment)    │
├─────────────────────────────────────┤
│ Length (u32, big-endian) │ 4 bytes  │
│ JSON payload             │ N bytes  │
└─────────────────────────────────────┘
```

**Example**:
```
[0x00, 0x00, 0x00, 0x2A]  // Length = 42 bytes
{"chunk_id":1,"data":["line1","line2"],"targets":["word"]}
```

### Completion Protocol

```
┌─────────────────────────────────────┐
│ Message Format (Completion)         │
├─────────────────────────────────────┤
│ Length (u32, big-endian) │ 4 bytes  │
│ JSON payload             │ N bytes  │
└─────────────────────────────────────┘
```

**Example (Success)**:
```
[0x00, 0x00, 0x00, 0x10]  // Length = 16 bytes
{"Success":3}  // Worker 3 succeeded
```

**Example (Failure)**:
```
[0x00, 0x00, 0x00, 0x10]  // Length = 16 bytes
{"Failure":5}  // Worker 5 failed
```

---

## Debugging Tips

### View Active Connections

```bash
netstat -an | findstr 127.0.0.1
```

### Enable Logging

Add to `main.rs`:
```rust
env_logger::init();
log::debug!("Worker {} processing assignment", id);
```

### Simulate Network Issues

```rust
// In socket_work_channel.rs
thread::sleep(Duration::from_millis(100)); // Add latency
```

### Catch Socket Errors

```rust
match stream.write_all(&bytes) {
    Ok(_) => {},
    Err(e) => eprintln!("Socket error: {}", e),
}
```

---

## Key Takeaways

### When to Use

✅ **Perfect for**:
- Learning network programming
- Traditional multi-threaded architectures
- Preparing for distributed deployment
- Testing with realistic latency

❌ **Not suitable for**:
- Maximum performance requirements
- True process isolation
- Very high worker counts (>1000)
- Cross-machine deployment (needs multi-process)

### Design Lessons

1. **Serialization has cost**: JSON adds measurable overhead
2. **TCP is reliable**: Even on loopback, protocol design matters
3. **Threads are heavy**: Limited by OS resources
4. **Fault tolerance needs work**: Poisoned locks are a real issue

### Compared to Other Implementations

| Aspect | task-channels | **thread-socket** | process-rpc |
|--------|--------------|-------------------|-------------|
| Speed | Fastest | Moderate | Variable |
| Isolation | None | Thread-level | Process-level |
| Realism | Low | **Medium** | High |
| Complexity | Low | **Medium** | High |

---

## Next Steps

- **[Compare with task-channels](../task-channels/README.md)** - See async task implementation
- **[Compare with process-rpc](../process-rpc/README.md)** - See multi-process RPC implementation
- **[Explore core traits](../core/README.md)** - Understand the abstraction layer
- **[Back to Main README](../README.md)**
