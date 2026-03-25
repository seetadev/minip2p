## MiniP2P Plan (Better DX + Opinionated)

This plan keeps the same core technical direction and improves developer experience:

- **Sans-IO stays**: protocol logic remains pure state machines.
- **QUIC-first stays**: primary early transport remains QUIC (`/quic-v1`).
- **`no_std` support**: core crates are designed to work with `no_std` + `alloc`.
- **Lightweight by design**: keep memory, CPU, and dependency footprint small.
- **Better DX**: easier onboarding, clearer errors, stronger defaults.
- **Opinionated defaults**: good behavior out of the box, with optional overrides.

The architecture remains educational and explicit, while setup and operations become simpler.

## What Success Looks Like

By default, a new user should be able to:

1. Install and run a local two-peer demo in under 5 minutes.
2. Start with safe defaults and no mandatory advanced settings.
3. Change behavior via config file, env vars, or runtime overrides.
4. Understand failures from actionable error messages.

## Target Users

- **Learner**: wants to understand libp2p internals without too much boilerplate.
- **App developer**: wants a stable API and good defaults for local networking.
- **Integrator**: needs predictable configuration for CI, staging, and production.

## Product Principles

### 1) Beginner-First Defaults

- Provide default transport/protocol settings that work for localhost immediately.
- Make advanced knobs optional.
- Keep first examples copy/paste friendly.

### 2) Progressive Complexity

- Quick start path for basic use.
- Advanced sections for protocol tuning and transport details.
- Public API should expose power without requiring it.

### 3) Opinionated Defaults

- Choose practical defaults for local-first usage.
- Reduce setup choices for first-time users.
- Allow overrides without requiring them for normal use.

### 4) `no_std` + Lightweight First

- Keep protocol/state-machine crates `no_std` compatible (using `alloc` where needed).
- Separate runtime adapters (`std`/network/FFI) from core protocol logic.
- Prefer small, focused dependencies and avoid unnecessary abstraction overhead.

### 5) Configuration Consistency

- One config schema shared by Rust and JS/WASM entry points.
- Same key names across all surfaces.
- Explicit precedence and validation rules.

### 6) Clear Operational Feedback

- Structured logs and consistent event names.
- Error messages include cause + suggested fix when possible.
- Diagnostic mode for handshake and protocol negotiation traces.

## Proposed User Experience

### Quick Start Path

- `cargo test` validates local build health.
- `examples/` include a basic connect + ping + pub/sub walkthrough.
- "Getting started" docs focus on outcomes, not internals.

### Optional Future CLI (Nice-to-have)

- `minip2p init` creates starter config.
- `minip2p run --config minip2p.toml` starts a node.
- `minip2p inspect` prints effective merged config.

## Configuration Strategy

### Config Sources and Precedence

From lowest to highest priority:

1. Built-in defaults
2. Config file (`minip2p.toml`)
3. Environment variables (`MINIP2P_*`)
4. Runtime overrides (API/CLI flags)

### Validation Rules

- Validate all user-provided values at startup.
- Fail fast on invalid types or out-of-range values.
- Include exact config path in errors (for example, `protocols.ping.interval_ms`).

## Roadmap (User-Outcome Driven)

### Milestone 0: Foundation Cleanup

- Align docs with current workspace reality (`packages/identity`, `packages/core`).
- Add a simple "state of the project" section in README.
- Define stable config schema draft.
- Define and document `no_std` boundaries for each planned crate.

**Exit criteria**
- New contributors can run tests and understand project status in under 10 minutes.

### Milestone 1: Config Core + Defaults

- Implement typed config structs with defaults.
- Add file/env/runtime merge logic.
- Add config validation and human-readable errors.
- Keep config/core types usable from `no_std` contexts where practical.

**Exit criteria**
- A node can start from defaults only.
- A user can override at least 5 key settings via env vars.

### Milestone 2: Local Connectivity (QUIC)

- Bring up two local peers over QUIC with default config.
- Add clear connection lifecycle events.
- Document first working local network flow.

**Exit criteria**
- Two peers connect and exchange bytes reliably in local integration tests.

### Milestone 3: Core Protocols (Ping + Identify)

- Enable ping and identify by default.
- Add protocol-specific config knobs.
- Improve event payloads for host apps.

**Exit criteria**
- Ping latency and timeout events are visible and deterministic.
- Identify data is available to host runtime.

### Milestone 4: GossipSub (Configurable)

- Implement subscribe/publish baseline.
- Expose mesh and heartbeat tuning via config.
- Add anti-duplication behavior and tests.

**Exit criteria**
- Multi-peer local topic propagation works with configurable mesh parameters.

### Milestone 5: DX and Operational Polish

- Add guided examples and troubleshooting docs.
- Provide structured logging and debug mode.
- Improve error catalog and common remediation hints.

**Exit criteria**
- At least one end-to-end example runs from docs with no hidden steps.

## Proposed Workspace Evolution

Current crates:

- `packages/identity` (implemented)
- `packages/core` (scaffold)

Planned additions:

- `packages/config` for typed settings, merge, and validation.
- `packages/transport` for connection state and transport abstractions.
- `packages/protocols` for multistream/ping/identify/gossipsub.
- `packages/swarm` for orchestration and action/event boundaries.
- `packages/ffi` for WASM-safe interfaces.

## Documentation Plan

- `README.md`: quick start + current status + minimal architecture map.
- `docs/config.md`: full config reference with defaults and examples.
- `docs/troubleshooting.md`: common errors and fixes.
- `examples/`: numbered examples from easiest to advanced.

## Quality Gates

- Unit tests for config parsing/merge/validation.
- Integration tests for two-peer local connectivity.
- Snapshot tests for key event and error payloads.
- CI check that docs examples remain runnable.
- CI checks for `no_std` builds on core crates.
- Track size and dependency budgets to preserve lightweight defaults.

## Decision Log (Initial)

- Keep **sans-IO** as non-negotiable architecture.
- Keep **QUIC-first** for early milestones.
- Require **`no_std` support** for core state-machine crates.
- Keep the stack **lightweight** in code size and dependencies.
- Improve **DX** through clearer workflows, docs, and errors.
- Be **opinionated by default**, configurable when needed.
