# Raft in Rust — A Multi-Environment, Correctness-First Implementation

## Overview

This project implements the **Raft consensus algorithm** once and realizes it across multiple execution environments — from a fully deterministic in-memory simulator, to `no_std + Embassy`, and finally to a production-grade AWS/Kubernetes deployment.

The initial inspiration and methodological foundation for this work comes from the **MIT distributed systems labs (6.824 / 6.5840)**, particularly the Raft exercise. The project builds on those ideas but deliberately extends them toward stronger abstraction boundaries, multi-environment realizations, and production-grade operability.

The core principle is simple and non-negotiable:

> **Correctness is proven in simulation. Infrastructure is added only after the logic is frozen.**

This repository is intentionally structured to separate **algorithmic correctness** from **operational concerns**.

---

## Design Philosophy

### 1. One Core, Many Realizations

* Raft logic is implemented **exactly once** in a technology-agnostic core.
* All environment-specific concerns (runtime, networking, storage, observability) are layered *around* the core.
* The Raft core does not know:

  * which async runtime it runs on
  * how messages are transported
  * how data is persisted
  * where or how it is deployed

### 2. Correctness Before Infrastructure

* The algorithm is validated using a **deterministic, adversarial test harness**.
* Network partitions, message drops, reordering, and crashes are simulated.
* Only after correctness is established do we introduce:

  * persistence
  * real networking
  * cloud infrastructure

### 3. Infrastructure Is a Plugin

* Tokio, gRPC, Kubernetes, AWS, Grafana, Jaeger, etc. are **realizations**, not dependencies.
* The Raft core remains close to "bare metal" and compatible with `no_std`.

---

## Project Structure

```
raft-core/            # no_std Raft algorithm (frozen logic)
raft-sim/             # std-based deterministic simulator & test harness
raft-runtime-embassy/ # no_std + Embassy realization (embedded target)
raft-runtime-std/     # std + Tokio + networking (cloud runtime)
raft-deploy-aws/      # Docker, Kubernetes, Helm, observability
```

### `raft-core`

* `#![no_std]` (optionally `alloc`)
* Implements:

  * Leader election
  * Log replication
  * Commit rules
  * State transitions
* Exposes abstract traits for:

  * Transport
  * Clock
  * Storage

This crate is **never allowed** to depend on:

* async runtimes
* networking stacks
* serialization frameworks
* operating system facilities

---

## Implementation Phases

### Phase 0 — Raft Core ✅

* Pure in-memory implementation
* Deterministic execution
* No IO, no randomness
* Focus: safety & liveness

Exit criteria (all met):

* ✅ Single-leader guarantee
* ✅ Log-matching property
* ✅ Monotonic commit index
* ✅ Correct recovery from partitions

**Status**: Complete. Validated in Embassy-sim with UDP transport.

### Phase 1 — Log Compaction & Crash Recovery ✅

* Log compaction via snapshots
* Snapshot creation and transfer
* Crash recovery with snapshots
* Bounded memory usage for long-running clusters

Exit criteria (all met):

* ✅ Automatic log compaction at threshold
* ✅ Snapshot transfer to lagging followers
* ✅ Correct recovery after crash with snapshot
* ✅ Memory bounded even with high write load

**Status**: Complete. 100 tests passing. Validated in simulation and Embassy-sim.

---

## Raft Enhancements

### Pre-Vote Protocol ✅ **ENABLED**

This implementation includes the **Pre-Vote Protocol** as described in Section 9.6 of Diego Ongaro's Raft thesis. Pre-vote is a critical optimization that prevents disruptions from partitioned or restarting nodes.

#### Why Pre-Vote?

In standard Raft, when a node's election timer fires, it immediately:
1. Increments its term
2. Starts an election
3. Requests votes from peers

**Problem**: A node that's been partitioned (can't reach majority) will repeatedly time out and increment its term. When the partition heals, this node contacts the cluster with a very high term, causing the current leader to step down unnecessarily.

**Solution**: Pre-vote adds a preliminary phase:
1. Node first asks "would you vote for me?" (pre-vote request)
2. If it receives majority approval, *then* it increments term and starts a real election
3. If pre-vote fails, term stays unchanged (no disruption)

#### Benefits:
- ✅ **Prevents term inflation** from partitioned nodes
- ✅ **Reduces disruptions** during network issues
- ✅ **Maintains liveness** - legitimate elections still proceed
- ✅ **No safety impact** - all Raft guarantees preserved

#### Implementation Details:
- Pre-vote uses current term (no increment)
- Pre-vote doesn't modify `voted_for` or persistent state
- Same log up-to-date checks as regular votes
- Majority of pre-votes required to proceed to real election
- Transparent to rest of system (no API changes)

**Status**: Fully implemented and tested. 6 dedicated pre-vote tests. Works across all environments (sim, embassy-sim).

---

### Phase 1 — Simulation & Proof ✅

* Deterministic cluster simulator
* Simulated network with:

  * partitions
  * message drops
  * reordering
  * latency
* Crash / restart modeling

Purpose:

* Produce **evidence of correctness** via integration-level tests
* Tokio may be used **only as a dev dependency**

---

### Phase 2 — `no_std + Embassy`

* Embedded-compatible realization
* Small, static clusters (5–7 nodes)
* Embassy executor and timers
* In-memory or flash-backed storage
* Fixed-capacity buffers

Purpose:

* Validate abstraction boundaries
* Demonstrate near bare-metal execution
* Prove portability without logic changes

---

### Phase 3 — Raft Advanced Features (Planned)

* Dynamic membership changes
* Read-only query optimization
* Leadership transfer

Purpose:

* Complete Raft implementation for production readiness
* Safe reconfiguration without downtime
* Performance optimizations for read-heavy workloads

**Note**: Log compaction and Pre-Vote Protocol already complete (see above).

---

### Phase 4 — Cloud-Native (AWS)

* `std` + Tokio runtime
* Real networking (e.g. gRPC/TCP)
* Persistent storage (EBS)
* Docker images
* Kubernetes (StatefulSets)
* Helm charts

Observability:

* Metrics (Prometheus / Grafana)
* Tracing (OpenTelemetry / Jaeger)
* Structured logging

Serialization:

* `serde`, `sonic`, or similar — **strictly outside the core**

---

## Testing Strategy

* All Raft correctness tests run unchanged across phases
* Infrastructure additions must not alter test behavior
* Failures must be:

  * reproducible
  * deterministic
  * explainable

The test harness is treated as a **formal contract**.

---

## TODO: Advanced Raft Features & Documentation

### Algorithmic Features (To Be Implemented)

- 🔲 **Dynamic Membership**: Adding/removing nodes from the cluster (Joint Consensus or Single-Server Changes)
- 🔲 **Read-Only Queries**: Linearizable reads without log entries (leader leases)
- 🔲 **Leadership Transfer**: Graceful handoff for maintenance

### Architectural Decision Records (To Be Documented)

#### High Priority
- 🔲 **ADR-R11: Storage Durability Guarantees** - Fsync policy, WAL vs direct writes, durability/throughput tradeoffs
- 🔲 **ADR-R13: Transport Abstraction Design** - Why async-agnostic, message delivery guarantees, timeout handling
- 🔲 **ADR-R14: Error Propagation Strategy** - Panic vs Result, storage failure handling, network retry policies
- 🔲 **ADR-R16: Configuration Management** - Static vs dynamic config, election timeout tuning, snapshot threshold policy
- 🔲 **ADR-R19: Security Boundary Definition** - Trusted network assumption, TLS/auth in runtime layer, no BFT

#### Medium Priority
- 🔲 **ADR-R9: Observer Pattern for Instrumentation** - Trait-based observers, observable events, separation of concerns
- 🔲 **ADR-R10: Zero-Cost Abstractions for Telemetry** - Zero-overhead observability in `no_std`, Prometheus integration
- 🔲 **ADR-R12: Serialization Strategy** - Wire format choice, backward compatibility, schema evolution
- 🔲 **ADR-R17: Multi-Environment Realization Strategy** - Why Embassy sim exists, path to production, adapter responsibilities

#### Lower Priority
- 🔲 **ADR-R15: Deterministic Testing Philosophy** - Imperative vs property-based tests, chaos testing, soak tests
- 🔲 **ADR-R18: Memory Bounds & Resource Limits** - Max log size, message size limits, connection limits
- 🔲 **ADR-R20: Client Request Semantics** - Submit returns index not result, NotLeader retries, linearizability guarantees

---

## Non-Goals

This project intentionally does **not** aim to:

* implement Byzantine fault tolerance
* maximize throughput
* compete with production systems like etcd
* support large dynamic clusters in embedded targets

The goal is **clarity, correctness, and architectural rigor**.

---

## Motivation

This project exists to demonstrate:

* deep understanding of distributed consensus
* disciplined abstraction design
* correctness-first engineering
* portability across radically different environments
* production-ready observability practices

---

## Status

### Phase Progress

- ✅ **Phase 0 — Raft Core**: Complete (Leader election, log replication, commit rules)
- ✅ **Phase 1 — Simulation & Proof**: Complete (raft-sim with deterministic test harness)
- ✅ **Phase 2 — `no_std + Embassy`**: Complete (Embassy-sim with UDP transport)
  - Leader election with randomized timeouts
  - Log replication with quorum tracking
  - Client request handling with transparent forwarding
  - Commit-based acknowledgments
- 🔲 **Phase 3 — Raft Advanced Features**: Planned (log compaction, dynamic membership, etc.)
- 🔲 **Phase 4 — Cloud-Native (AWS)**: Planned

**Note**: Phase 2 was completed before Phase 1 to validate the abstraction boundaries work in the most constrained environment first.

---

## License

MIT / Apache-2.0 (TBD)
