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

### Phase 0 — Raft Core

* Pure in-memory implementation
* Deterministic execution
* No IO, no randomness
* Focus: safety & liveness

Exit criteria:

* Single-leader guarantee
* Log-matching property
* Monotonic commit index
* Correct recovery from partitions

---

### Phase 1 — Simulation & Proof

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

### Phase 3 — Cloud-Native (AWS)

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

🚧 Work in progress — currently in **Raft Core & Simulation** phase.

---

## License

MIT / Apache-2.0 (TBD)
