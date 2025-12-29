# process-rpc - Multi-Process gRPC Implementation

**[← Back to MapReduce README](../README.md)**

This implementation uses **separate OS processes** with **gRPC (Tonic)** for communication. Workers run as independent processes, communicating via Protocol Buffers over HTTP/2, providing the **strongest isolation** and simulating a true distributed system environment.

---

## Overview

**Execution Model**: Separate OS processes (one per worker)
**Communication**: gRPC (Tonic/Prost) over HTTP/2
**State Storage**: `GrpcStateAccess` (Remote State Server)
**Isolation**: Process-level (separate memory spaces)
**Performance**: ⭐ Slowest (1.65-6.90s) - High overhead due to process startup and serialization

---

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                   Main Process (Coordinator)                 │
│                        Executor                              │
│                 + gRPC State Server                          │
└────────┬───────────────────────────────┬────────────────────┘
         │                               │
    ┌────▼─────┐                    ┌────▼─────┐
    │ gRPC     │                    │ gRPC     │
    │ Channel  │                    │ Channel  │
    │ :50051   │                    │ :50051   │
    └────┬─────┘                    └────┬─────┘
         │                               │
    ┌────▼──────┐                   ┌────▼──────┐
    │ Process   │                   │ Process   │
    │ Mapper 1  │                   │ Reducer 1 │
    │           │                   │           │
    │ [spawn]   │                   │ [spawn]   │
    └────┬──────┘                   └────┬──────┘
         │                               │
    ┌────▼──────┐                   ┌────▼──────┐
    │ Process   │                   │ Process   │
    │ Mapper N  │                   │ Reducer N │
    └────┬──────┘                   └────┬──────┘
         │                               │
         └───────────┬───────────────────┘
                     │
            ┌────────▼─────────┐
            │  gRPC State Svc  │
            │ (Remote Access)  │
            └──────────────────┘
```

---

## Key Components

### 1. Process Runtime (`process_runtime.rs`)
- Spawns new worker processes using `std::process::Command`.
- Passes configuration via command-line arguments (`--worker`, `--type`, `--task`).
- Manages the lifecycle of child processes.

### 2. gRPC Communication (`rpc.rs`, `.generated/`)
- Uses **Tonic** for the gRPC server and client.
- **Protocol Buffers** (`proto/mapreduce.proto`) define the service interface:
  - `GetTask`: Workers request work from the coordinator.
  - `ReportCompletion`: Workers notify when done.
  - `GetState`/`SetState`: Workers access shared state remotely.
- Generated code is stored in `.generated/` to keep the source tree clean.

### 3. State Server (`grpc_state_server.rs`)
- Runs a gRPC service within the coordinator process.
- Wraps the `LocalStateAccess` (HashMap) and exposes it over the network.
- Allows workers to read/write state as if it were local, but via network calls.

### 4. Worker Implementation (`mapper.rs`, `reducer.rs`)
- **Entry Point**: The `main` function detects the `--worker` flag and branches to worker logic.
- **Connection**: Workers connect back to the coordinator's gRPC server on startup.
- **Retry Logic**: Includes robust retry mechanisms for connection establishment.

---

## Performance Characteristics

This implementation is significantly slower than `task-channels` and `thread-socket` due to:
1.  **Process Startup Cost**: Spawning a new OS process for every worker is expensive on Windows.
2.  **Serialization Overhead**: All data must be serialized to Protobuf and deserialized.
3.  **Network Stack**: Communication goes through the full TCP/IP stack (loopback).

However, it provides the best simulation of a real distributed system where components fail independently and share no memory.

---

## Usage

Run via the main Cargo workspace or the stress test script:

```powershell
# Run directly
cargo run --release --bin map-reduce-process-rpc

# Run stress test
.\map-reduce\scripts\stress_test.ps1
```

## Configuration

- **Ports**: Configured in `src/config.rs` (default: 50051).
- **Protobuf**: Requires `protoc` compiler (automatically handled by build script if in PATH).
