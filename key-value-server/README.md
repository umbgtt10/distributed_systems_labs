# Key-Value Server: Pluggable Storage Architecture Demonstration

A Rust implementation inspired by [MIT 6.824 Lab 3: Raft Key-Value Service](https://pdos.csail.mit.edu/6.824/labs/lab-kvraft.html), demonstrating **storage abstraction** through three working implementations that share identical server logic.

**[← Back to Main README](../README.md)**

---

## What This Project Demonstrates

### MIT's Original Lab
- **One implementation** with Raft consensus
- **Learning goal**: Build a fault-tolerant replicated state machine
- **Fixed decisions**: Raft for replication, in-memory storage

### This Rust Adaptation
- **Three storage implementations** with pluggable backends
- **Learning goals**: Distributed KV protocols + abstraction design + Rust patterns
- **Pluggable decisions**: Storage backend, persistence strategy, data structures

**Key Innovation**: The same gRPC server and client logic run unchanged across all three storage backends, proving the abstractions work.

---

## Architecture Overview

This workspace uses a **trait-based storage abstraction**:

```
┌─────────────────────────────────────────────────────────────┐
│               Server Logic (Fixed)                          │
│          KeyValueServer + GrpcClient + Protocol             │
│            (same across all implementations)                │
└─────────────────────────────────────────────────────────────┘
                              ▲
                              │
┌─────────────────────────────────────────────────────────────┐
│              Core Abstractions (Trait Boundaries)           │
│        Storage trait: get() | put() | print_all()           │
└─────────────────────────────────────────────────────────────┘
                              ▲
                              │
          ┌───────────────────┼─────────────────────┐
          │                   │                     │
    ┌─────▼─────────┐   ┌─────▼──────────┐   ┌─────▼──────┐
    │server-in-memory│   │server-flat-file│   │server-sled-db│
    │   (HashMap)    │   │  (File I/O)    │   │ (Embedded DB)│
    └────────────────┘   └────────────────┘   └────────────┘
```

---

## Project Structure

### [`core/`](core/README.md) - Core Abstractions & Protocol 🧩

**Role**: Defines the `Storage` trait and provides the gRPC server/client implementation

**Key Components**:
- `Storage` trait - Storage backend contract (get/put with version control)
- `KeyValueServer` - Generic gRPC service wrapping any `Storage` implementation
- `GrpcClient` - Sophisticated client with retry logic and recovery detection
- `PacketLossWrapper` - Fault injection middleware for testing
- `ServerRunner` - Orchestration for running tests with multiple clients

**Why it matters**: The trait boundary ensures that server logic never depends on storage implementation details. Swapping backends requires zero changes to the protocol layer.

→ [**Explore Core Abstractions**](core/README.md)

---

### [`server-in-memory/`](server-in-memory/README.md) - In-Memory HashMap 🚀

**Storage Backend**: `HashMap<String, (String, u64)>` with `Mutex`
**Persistence**: None (data lost on restart)
**Concurrency**: Single global lock

**Use Cases**:
- ✅ Development and testing
- ✅ Performance baseline
- ✅ Learning the protocol
- ❌ Not suitable for production (no persistence)

**Performance**: Fastest (no I/O overhead)

→ [**Explore In-Memory Implementation**](server-in-memory/README.md)

---

### [`server-flat-file/`](server-flat-file/README.md) - File-Based Persistence 📁

**Storage Backend**: JSON-serialized text file
**Persistence**: Full (survives restarts)
**Concurrency**: File-level locking

**Use Cases**:
- ✅ Simple persistence requirements
- ✅ Human-readable storage format
- ✅ Easy debugging (inspect file directly)
- ❌ Not suitable for high-throughput workloads

**Performance**: Moderate (file I/O on every operation)

→ [**Explore Flat-File Implementation**](server-flat-file/README.md)

---

### [`server-sled-db/`](server-sled-db/README.md) - Embedded Database 🗄️

**Storage Backend**: [Sled](https://github.com/spacejam/sled) embedded database
**Persistence**: Full with crash recovery
**Concurrency**: Lock-free data structures

**Use Cases**:
- ✅ Production-grade persistence
- ✅ High-throughput concurrent workloads
- ✅ ACID guarantees
- ✅ Efficient range queries

**Performance**: Fast (optimized B-tree, memory-mapped I/O)

→ [**Explore Sled DB Implementation**](server-sled-db/README.md)

---

## Key Features

### 1. Optimistic Concurrency Control
Every value has a version number. Updates specify expected version:
- `PUT key="x" value="y" version=0` → Create (fails if key exists)
- `PUT key="x" value="y" version=5` → Update (fails if current version ≠ 5)

**Why**: Prevents lost updates without holding locks across network calls.

### 2. Fault Tolerance Testing
- **Client-side packet loss**: Simulates dropped requests
- **Server-side packet loss**: Drops responses *after* successful writes
- **Recovery detection**: Client recognizes when a "failed" write actually succeeded

### 3. Stress Testing Framework
All implementations include stress test scripts that:
- Spawn multiple concurrent clients
- Generate random operations on overlapping keys
- Inject packet loss to test recovery logic
- Verify final consistency

---

## Quick Start

### Run All Tests
```powershell
cd key-value-server\scripts
.\stress_test.ps1
```

This runs all three implementations in sequence and reports results.

### Run Individual Implementation
```powershell
# In-memory
cd server-in-memory\scripts
.\run_test.ps1

# Flat-file
cd server-flat-file\scripts
.\run_test.ps1

# Sled DB
cd server-sled-db\scripts
.\run_test.ps1
```

---

## Configuration

All implementations share `config.json` in the key-value-server root:

```json
{
  "test_duration_seconds": 30,
  "server_packet_loss_rate": 2.0,
  "max_retries_server_packet_loss": 10,
  "clients": [
    {
      "name": "client_1",
      "keys": ["key1", "key2", "key3"],
      "client_packet_loss_rate": 1.0,
      "success_sleep_ms": 10,
      "error_sleep_ms": 50
    }
  ]
}
```

**Parameters**:
- `test_duration_seconds`: How long to run the stress test
- `server_packet_loss_rate`: Percentage of server responses to drop (after write succeeds)
- `max_retries_server_packet_loss`: Network retry limit for transient failures
- `clients`: Array of client configurations with overlapping key sets

---

## Performance Comparison

| Implementation | Throughput | Latency | Persistence | Concurrency |
|---------------|-----------|---------|-------------|-------------|
| In-Memory     | Highest   | Lowest  | None        | Mutex lock  |
| Flat-File     | Moderate  | High    | Full        | File lock   |
| Sled DB       | High      | Low     | Full + ACID | Lock-free   |

---

## Learning Outcomes

### Storage Abstraction Design
- **Trait boundaries**: Where to draw the line between protocol and storage
- **Error mapping**: Converting storage errors to protocol responses
- **Async traits**: Using `async_trait` for async methods in traits

### Distributed Systems Concepts
- **Optimistic concurrency**: Version-based conflict detection
- **Idempotency**: Handling duplicate requests safely
- **Fault injection**: Testing recovery paths systematically
- **Retry strategies**: Distinguishing transient vs permanent failures

### Rust Patterns
- **Generic servers**: `KeyValueServer<S: Storage>` works with any backend
- **Trait objects**: `Arc<dyn Storage>` for runtime polymorphism
- **Spawn blocking**: Mixing sync storage APIs with async server code
- **Error propagation**: `Result` types and `?` operator throughout

---

## Next Steps

1. **Explore Core**: Understand the `Storage` trait and server implementation → [`core/README.md`](core/README.md)
2. **Run Tests**: Execute stress tests to see fault tolerance in action → `scripts/stress_test.ps1`
3. **Compare Implementations**: Examine how each backend satisfies the `Storage` contract
4. **Extend**: Add new backends (Redis, SQLite, etc.) by implementing `Storage`

---

## References

- [MIT 6.824: Distributed Systems](https://pdos.csail.mit.edu/6.824/)
- [Tonic gRPC Framework](https://github.com/hyperium/tonic)
- [Sled Embedded Database](https://github.com/spacejam/sled)
- [Optimistic Concurrency Control](https://en.wikipedia.org/wiki/Optimistic_concurrency_control)
