# Embassy-Sim: Raft Consensus in `no_std` Embedded Environment

## Overview

**Embassy-Sim** is a proof-of-concept demonstration that a full Raft consensus cluster can run in a `no_std` embedded environment with **real UDP-based networking**. Built on the [Embassy](https://embassy.dev) async runtime for Cortex-M microcontrollers, this project simulates a 5-node Raft cluster running on QEMU with simulated Ethernet.

### Key Achievement

✅ **Running a distributed consensus protocol (Raft) entirely in `no_std`**
- No standard library (no heap allocations via `std`, only `alloc`)
- Real network stack (embassy-net with UDP)
- Multiple async tasks coordinated via Embassy executor
- Serialization with postcard (no protobuf/JSON dependencies)
- Simulated Ethernet driver for testing without hardware

## Architecture

### Core Components

```
embassy-sim/
├── src/
│   ├── main.rs                    # Entry point, spawns cluster
│   ├── embassy_node.rs            # Main Raft node loop
│   ├── transport/
│   │   ├── mod.rs                 # Feature bifurcation point
│   │   ├── async_transport.rs     # Transport trait
│   │   ├── embassy_transport.rs   # Raft-to-async bridge
│   │   ├── channel/               # In-memory transport
│   │   │   ├── transport.rs
│   │   │   └── setup.rs
│   │   └── udp/                   # UDP network transport
│   │       ├── transport.rs       # UDP async implementation
│   │       ├── driver.rs          # Mock Ethernet driver
│   │       ├── serde.rs           # Wire protocol
│   │       ├── config.rs          # Network configuration
│   │       └── setup.rs           # Cluster initialization
│   ├── embassy_storage.rs         # In-memory Raft log
│   ├── embassy_timer.rs           # Election/heartbeat timers
│   └── [other adapters]
└── Cargo.toml
```

### Transport Bifurcation Pattern

The project implements a **clean separation** between two transport modes:

1. **Channel Transport** (`--features channel-transport`): In-memory channels for fast simulation
2. **UDP Transport** (`--features udp-transport`): Full network stack with simulated Ethernet

The `transport/mod.rs` module conditionally compiles the active transport and re-exports a unified `setup` interface, making `main.rs` completely transport-agnostic:

```rust
// main.rs doesn't know or care which transport is active
transport::setup::initialize_cluster(spawner, cancel.clone()).await;
```

**Features are mutually exclusive** by design (enforced at compile-time).

## Running the Simulation

### Prerequisites

- Rust nightly toolchain
- `thumbv7em-none-eabihf` target installed:
  ```bash
  rustup target add thumbv7em-none-eabihf
  ```
- QEMU ARM system emulation:
  ```bash
  # Windows (via Chocolatey)
  choco install qemu

  # Linux
  sudo apt install qemu-system-arm
  ```

### Quick Start

**UDP Transport (Default):**
```powershell
# From embassy-sim directory
cargo run --release --features udp-transport

# Or use the script
.\scripts\run_udp.ps1
```

**Channel Transport:**
```powershell
cargo run --release --features channel-transport --no-default-features

# Or use the script
.\scripts\run_channel.ps1
```

### Expected Output

```
INFO Starting 5-node Raft cluster in Embassy
INFO Using UDP transport (simulated Ethernet)
INFO WireRaftMsg serialization layer: COMPLETE ✓
INFO Network stacks created, waiting for configuration...
INFO Node 1 network configured: Some(StaticConfigV4 { ... })
...
INFO All UDP nodes started!
INFO Node 3 starting election (term 1)
INFO Node 3 became Leader for term 1
INFO Node 3 sending heartbeats...
```

## Technical Details

### Memory Constraints

- **Heap Size**: 128 KB (configurable in `heap.rs`)
- **Max Log Entries**: 256 per node (heapless::Vec)
- **Channel Sizes**:
  - Channel transport: 32 messages
  - UDP transport: 16 messages per node
- **UDP Buffer**: 4 KB per socket

### Network Configuration

Each node gets a static IP in the `10.0.0.0/24` subnet:
- Node 1: `10.0.0.1:9001`
- Node 2: `10.0.0.2:9002`
- Node 3: `10.0.0.3:9003`
- Node 4: `10.0.0.4:9004`
- Node 5: `10.0.0.5:9005`

### Serialization

Raft messages are serialized using **postcard** (a compact binary format):
- `RaftMsg` → `WireRaftMsg` (custom mirror type with Serde derives)
- No dynamic allocations during serialization
- Efficient for embedded environments (~100-500 bytes per message)

### Concurrency Model

Embassy's async runtime manages:
- 5 Raft node tasks
- 5 Network stack runners (UDP only)
- 5 UDP listener tasks (UDP only)
- 5 UDP sender tasks (UDP only)

All coordinated via `embassy-executor` with bounded channels for backpressure.

## Development

### Building for Different Features

```bash
# Check compilation
cargo check --target thumbv7em-none-eabihf --features udp-transport
cargo check --target thumbv7em-none-eabihf --features channel-transport --no-default-features

# Build release binary
cargo build --release --target thumbv7em-none-eabihf

# Run tests (host environment)
cargo test --lib
```

### Code Quality

The codebase follows embedded best practices:
- ✅ Zero `unsafe` outside documented initialization
- ✅ No panics in critical paths
- ✅ Bounded memory usage
- ✅ Feature-gated optional dependencies
- ✅ Compile-time mutual exclusivity enforcement

### Limitations

This is a **proof-of-concept**, not production code:
- Fixed 5-node cluster (hardcoded)
- No persistent storage (log resets on restart)
- No log compaction/snapshotting
- Simplified error handling
- QEMU-only (not tested on real hardware)

## Project Goals

### ✅ Achieved

1. **Prove Raft can run in `no_std`**: Demonstrated with a working 5-node cluster
2. **Real UDP networking**: Full TCP/IP stack via embassy-net (not just in-memory channels)
3. **Clean architecture**: Transport abstraction allows easy feature switching
4. **Embassy integration**: Proper use of async tasks, timers, and channels
5. **Scalable structure**: Bifurcation pattern ready for additional transports (UART, CAN, etc.)

### Future Enhancements (Out of Scope)

- [ ] Run on real hardware (STM32, nRF52, etc.)
- [ ] Persistent storage (flash driver integration)
- [ ] Dynamic cluster membership
- [ ] Log compaction/snapshotting
- [ ] Network partition simulation

## Related Projects

- **raft-core**: The platform-agnostic Raft library this project uses
- **Embassy**: The async runtime powering the executor
- **embassy-net**: The embedded TCP/IP stack (based on smoltcp)

## License

Copyright 2025 Umberto Gotti
Licensed under the Apache License, Version 2.0

## Acknowledgments

Built with:
- [Embassy](https://github.com/embassy-rs/embassy) - Async runtime for embedded Rust
- [smoltcp](https://github.com/smoltcp-rs/smoltcp) - TCP/IP stack
- [postcard](https://github.com/jamesmunns/postcard) - Compact serialization format
