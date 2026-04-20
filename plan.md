## MiniP2P Plan (DX + Sans-I/O + no_std)

This plan reflects the current repository state and the next execution steps.

## Non-Negotiables

- **Sans-I/O first**: protocol and transport logic are state machines, not runtime-bound loops.
- **`no_std + alloc` core**: core packages remain portable across embedded and constrained targets.
- **DX-first API design**: local setup should be easy, defaults should be practical, and errors should be actionable.
- **Transport extensibility**: architecture must support QUIC now, then TCP/WS/WebRTC without core rewrites.
- **FFI-friendly boundaries**: public APIs should be designed so future WASM/TypeScript integration is straightforward.

## Current Baseline

Implemented crates:

- `packages/identity` (`minip2p-identity`)
- `packages/core` (`minip2p-core`)
- `packages/transport` (`minip2p-transport`)
- `transports/quic` (`minip2p-quic`)

Current validated capabilities:

- Local two-peer QUIC connectivity in integration tests.
- Bidirectional byte exchange.
- Multiple connections per peer with policy-based selection.
- Identity upgrade event flow and peer index updates.

## Architecture Boundaries

### Core (`no_std + alloc`)

- `packages/identity`: peer identity primitives.
- `packages/core`: transport-agnostic address and shared types.
- `packages/transport`: transport contract, shared connection/event/error types.

### Runtime adapters (`std`)

- `transports/quic`: QUIC implementation and DX-oriented runtime integration.
- Future `transports/tcp`, `transports/ws`, `transports/webrtc` follow the same separation.

### Ownership rules

- Runtime concerns (UDP/TCP sockets, DNS resolution, timers) belong in adapter crates.
- Transport-specific address validation belongs in adapters, not in `packages/core`.
- Shared crates define generic contracts and common semantics only.

## DX Principles

- **Fast first success**: new users should get two peers connected in under 5 minutes.
- **Progressive complexity**: simple default path first, advanced tuning second.
- **Consistent semantics**: same event and error shape across transport adapters.
- **Actionable failures**: errors should include context and suggested next fix.
- **Docs as product**: quickstart and troubleshooting are part of the API quality bar.

## Transport Extensibility Policy

- `PeerAddr` remains transport-agnostic and only guarantees terminal `/p2p/<peer-id>` semantics.
- Each transport crate validates its own accepted address shapes.
- Transport crates may expose ergonomic helper APIs while conforming to shared transport contracts.

## Roadmap (Outcome-Driven)

### Milestone 0: Docs and boundary alignment

- Keep root docs aligned with actual workspace crates and capabilities.
- Document no_std boundaries and adapter responsibilities.
- Clarify transport-agnostic vs transport-specific validation responsibilities.

**Exit criteria**
- New contributors can run tests and explain crate boundaries in under 10 minutes.

### Milestone 1: Transport contract hardening

- Refine transport guarantees (message semantics, event ordering, close behavior).
- Ensure shared error taxonomy is adapter-agnostic and actionable.
- Add conformance tests for transport contract behavior.

**Exit criteria**
- Shared transport behavior is explicit and test-backed.

### Milestone 2: QUIC DX polish

- Improve QUIC docs and ergonomics for local development.
- Tighten diagnostics around dial/listen/send/close failures.
- Keep integration tests stable and deterministic.

**Exit criteria**
- New users can run QUIC examples/tests with minimal setup and clear feedback.

### Milestone 3: Additional transport adapters

- Add `transports/tcp` and `transports/ws` prototypes aligned to shared contract.
- Validate adapter parity for key lifecycle and send/receive behaviors.
- Plan WebRTC adapter shape and required browser/runtime bridges.

**Exit criteria**
- At least one non-QUIC transport reaches local two-peer connectivity parity.

### Milestone 4: Core protocols

- Add Noise XX handshake state machine.
- Add ping and identify protocols with deterministic events.
- Start gossipsub baseline after ping/identify stabilization.

**Exit criteria**
- Ping and identify are visible through stable host-facing events.

### Milestone 5: Swarm, FFI, and operational polish

- Introduce swarm orchestration and clearer action/event plumbing.
- Design FFI-safe boundaries for JS/WASM embedding.
- Expand troubleshooting docs and runnable examples.

**Exit criteria**
- One end-to-end example runs from docs without hidden steps.

## Quality Gates

- Unit tests for core parsing/types and error behavior.
- Integration tests for two-peer connectivity per transport.
- `cargo check --no-default-features` for `packages/identity`, `packages/core`, and `packages/transport`.
- Stable, documented error messages for common misconfiguration paths.
- Dependency and footprint discipline for core crates.

## Decision Log

- Keep **Sans-I/O** as architecture baseline.
- Keep **`no_std + alloc`** support for shared/core crates.
- Keep **QUIC-first** while designing for **multi-transport** expansion.
- Keep `transports/quic` as a **single crate** with internal layering for logic and runtime ergonomics.
- Optimize for **DX** without compromising explicit architecture boundaries.
