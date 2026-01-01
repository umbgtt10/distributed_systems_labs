# Distributed Systems in Rust

A collection of distributed systems implementations in Rust, inspired by MIT's legendary [6.824: Distributed Systems](https://pdos.csail.mit.edu/6.824/schedule.html) course.

## Motivation

MIT's 6.824 course has trained thousands of engineers in distributed systems fundamentals through hands-on labs. The original labs are written in Go, leveraging its simplicity and built-in concurrency primitives.

**This repository reimagines these labs in Rust** to explore:

- ✅ **Memory Safety**: Rust's ownership system eliminates entire classes of bugs common in distributed systems (data races, use-after-free, etc.)
- ✅ **Performance**: Zero-cost abstractions and fine-grained control over memory layout enable production-grade performance
- ✅ **Type Safety**: Strong static typing catches errors at compile time that would be runtime failures in Go
- ✅ **Architectural Abstraction**: Rust's trait system enables pluggable architectures that Go's interfaces can't express as safely

## Why Rust for Distributed Systems?

| Concern | Go (Original) | Rust (This Repo) |
|---------|---------------|------------------|
| **Memory Safety** | GC + runtime panics | Compile-time guarantees |
| **Concurrency** | Goroutines (easy but risky) | Send/Sync bounds (safe by default) |
| **Performance** | Good (GC overhead) | Excellent (zero-cost abstractions) |
| **Type System** | Interfaces (runtime) | Traits (compile-time) |
| **Error Handling** | `if err != nil` | `Result<T, E>` (exhaustive) |

Rust's ownership model and type system make it **harder to write** but **easier to maintain** - particularly valuable as distributed systems grow in complexity.

## Projects

### [MapReduce](map-reduce/README.md) 🗺️

**Status:** ✅ Complete
**Original Lab:** [MIT 6.824 Lab 1](https://pdos.csail.mit.edu/6.824/labs/lab-mr.html)

A Rust adaptation and **architectural generalization** of MIT's MapReduce lab. Unlike the original single implementation, this demonstrates **pluggable infrastructure** through three working implementations:

- **task-channels**: In-process with Tokio async tasks
- **thread-socket**: Multi-threaded with TCP sockets
- **process-rpc**: Multi-process with RPC over TCP

All three share identical business logic, proving the abstraction design works.

→ [Explore MapReduce Implementation](map-reduce/README.md)

---

### [Key-Value Server](key-value-server/README.md) 🔑

**Status:** ✅ Complete
**Original Lab:** [MIT 6.824 Lab 3](https://pdos.csail.mit.edu/6.824/labs/lab-kvraft.html)

A Rust implementation demonstrating **pluggable storage backends** through three working implementations that share identical server/client logic:

- **server-in-memory**: HashMap with Mutex (reference implementation)
- **server-flat-file**: JSON file-based persistence
- **server-sled-db**: Production-grade embedded database

All three implement the same `Storage` trait with:
- **Optimistic concurrency control**: Version-based conflict detection
- **Fault injection**: Client and server-side packet loss simulation
- **Recovery detection**: Smart retry logic for failed writes
- **gRPC protocol**: Type-safe RPC with Tonic

→ [Explore Key-Value Server Implementation](key-value-server/README.md)

---

### Future Projects

- **Raft Consensus** (Lab 3)
- **Sharded Key/Value Service** (Lab 4)

## Learning Objectives

This repository serves multiple learning goals:

1. **Distributed Systems Fundamentals** (from MIT's original curriculum)
   - Coordination, fault tolerance, consistency, performance

2. **Rust Systems Programming** (unique to this adaptation)
   - Ownership, lifetimes, trait bounds, async/await, zero-cost abstractions

3. **Architectural Design** (enhanced beyond original)
   - Abstraction boundaries, pluggable infrastructure, trait-based polymorphism

## Getting Started

### Prerequisites

```bash
# Install Rust (https://rustup.rs/)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Verify installation
cargo --version
```

### Quick Start

```bash
# Clone the repository
git clone https://github.com/yourusername/distributed_systems.git
cd distributed_systems

# Run MapReduce stress tests (all implementations)
cd map-reduce
# Windows (PowerShell)
.\scripts\stress_test.ps1

# Run Key-Value Server stress tests (all implementations)
cd key-value-server\scripts
.\stress_test.ps1

# Individual implementations
cd map-reduce\task-channels && cargo run
cd key-value-server\server-in-memory && cargo run
```

## Project Structure

```
distributed_systems/
├── README.md                    # ← You are here
├── map-reduce/                  # MapReduce implementation
│   ├── README.md               # MapReduce overview
│   ├── core/                   # Core abstractions
│   │   └── README.md          # Trait design explanation
│   ├── task-channels/         # Implementation 1
│   │   └── README.md         # In-process async
│   ├── thread-socket/        # Implementation 2
│   │   └── README.md        # Multi-threaded TCP
│   ├── process-rpc/          # Implementation 3
│   │   └── README.md        # Multi-process RPC
│   └── word-search/          # MapReduce application
│       └── README.md        # Word search implementation
└── key-value-server/           # Key-Value Server implementation
    ├── README.md              # KV Server overview
    ├── core/                  # Core abstractions & protocol
    │   └── README.md         # Storage trait & gRPC
    ├── server-in-memory/     # Implementation 1
    │   └── README.md        # HashMap storage
    ├── server-flat-file/     # Implementation 2
    │   └── README.md        # JSON file storage
    └── server-sled-db/       # Implementation 3
        └── README.md        # Embedded database
```

## Educational Philosophy

This repository maintains **fidelity to MIT's pedagogical goals** while adding Rust-specific learning:

- ✅ **Respect**: Each lab preserves the core concepts from MIT's curriculum
- ✅ **Enhance**: Adds abstraction design patterns unique to Rust
- ✅ **Extend**: Demonstrates multiple implementation strategies
- ✅ **Educate**: Comprehensive documentation explains the "why" behind design choices

## Contributing

Contributions welcome! Particularly interested in:

- Additional implementation strategies (gRPC, WebSockets, etc.)
- Performance optimizations and benchmarks
- Documentation improvements
- Additional MIT 6.824 labs (Raft, KV Service, etc.)

## Acknowledgments

- **MIT PDOS Group** for creating the [6.824 course](https://pdos.csail.mit.edu/6.824/)
- **Robert Morris, Frans Kaashoek, and Nickolai Zeldovich** for their exceptional teaching
- **The Rust Community** for building an incredible language and ecosystem

## License

This is an educational project. Please refer to MIT 6.824's course policies when using this material for academic purposes.

---

**Built with 🦀 Rust** | Inspired by MIT 6.824 | Educational Resource
