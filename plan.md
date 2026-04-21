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
- `packages/multistream-select` (`minip2p-multistream-select`)
- `packages/ping` (`minip2p-ping`)
- `packages/transport` (`minip2p-transport`)
- `transports/quic` (`minip2p-quic`)

Current validated capabilities:

- Local two-peer QUIC connectivity in integration tests.
- Bidirectional stream data exchange with half-close propagation.
- Multistream-select with spec-compliant varint-length-prefixed framing.
- Ping protocol: outbound RTT measurement, inbound echo, fragmentation buffering, configurable timeouts.
- End-to-end protocol stack: QUIC transport + multistream-select + ping in integration tests.
- Transport contract with documented lifecycle guarantees and 12 conformance tests.
- Varint helpers shared via `minip2p-identity` and re-exported through `minip2p-core`.
- Rustdoc on all public APIs. Internal comments on all private functions and types.

## Architecture Boundaries

### Core (`no_std + alloc`)

- `packages/identity`: peer identity primitives.
- `packages/core`: transport-agnostic address and shared types.
- `packages/transport`: transport contract, shared connection/event/error types.
- `packages/tls`: libp2p TLS certificate generation and peer verification. Transport-agnostic.

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

### Milestone 0: Docs and boundary alignment -- DONE

- [x] Keep root docs aligned with actual workspace crates and capabilities.
- [x] Document no_std boundaries and adapter responsibilities.
- [x] Clarify transport-agnostic vs transport-specific validation responsibilities.
- [x] Rustdoc on all public types/methods across all crates.
- [x] Internal comments on all private functions, types, and fields.
- [x] READMEs for every crate.

**Exit criteria**
- New contributors can run tests and explain crate boundaries in under 10 minutes.

### Milestone 1: Transport contract hardening -- DONE

- [x] Document connection lifecycle, stream lifecycle, and event ordering on Transport trait.
- [x] Add 12 conformance tests for transport contract behavior.
- [x] Fix ping memory leak (`entry().or_default()`), stale state, dead error variant.

**Exit criteria**
- Shared transport behavior is explicit and test-backed.

### Milestone 2: libp2p TLS peer authentication

New crate: `packages/tls` (`minip2p-tls`) -- `no_std + alloc` compatible.

- Add `minip2p-tls` crate for cert generation and verification per the libp2p TLS spec.
- Generate self-signed X.509 certs with embedded libp2p public key (OID `1.3.6.1.4.1.53594.1.1`).
- Verify peer certs after TLS handshake and derive PeerId automatically.
- Wire into QUIC transport: accept `Ed25519Keypair`, auto-verify on connect.
- Validate against libp2p TLS spec test vectors (Ed25519, ECDSA, secp256k1).
- Transport-agnostic: reusable by future TCP/WS/WebRTC adapters.
- Dependencies: `x509-cert` (RustCrypto, `no_std`) for cert building, `der` for DER parsing.

**Exit criteria**
- Two QUIC peers connect and automatically know each other's PeerId without manual verification.
- Spec test vectors pass.

### Milestone 3: Identify protocol

- Add `packages/identify` for `/ipfs/id/1.0.0`.
- Peers exchange supported protocols, observed addresses, agent version after connecting.
- Required foundation for relay and hole punch (peers need to know each other's addresses).

**Exit criteria**
- After connecting, both peers learn each other's supported protocols and observed address.

### Milestone 4: Swarm and connection management

- Introduce a swarm layer that orchestrates connections, protocol negotiation, and address tracking.
- Handle "connect to peer X" by picking transport, negotiating protocols, managing lifecycle.
- Required before relay (relay needs to manage multiple connections per peer).

**Exit criteria**
- A swarm connects two peers, negotiates protocols, and runs ping without manual wiring.

### Milestone 5: Relay and NAT traversal

- Add relay protocol: peers connect through a public relay server.
- Add STUN client for public address discovery.
- Add DCUtR / hole punch: attempt direct UDP connection, fall back to relay.
- Relay server binary.

**Exit criteria**
- Two peers behind NAT can ping each other via relay, with hole punch attempted for direct path.

### Milestone 6: Additional transports and operational polish

- TCP + TLS transport adapter (reuses `minip2p-tls`).
- WebSocket transport adapter.
- FFI boundaries for WASM/TypeScript.
- Runnable examples and troubleshooting docs.

**Exit criteria**
- One end-to-end example runs from docs without hidden steps.

## Quality Gates

- Unit tests for core parsing/types and error behavior.
- Integration tests for two-peer connectivity per transport.
- `cargo check --no-default-features` for `packages/identity`, `packages/core`, `packages/transport`, and `packages/tls`.
- Stable, documented error messages for common misconfiguration paths.
- Dependency and footprint discipline for core crates.

## Decision Log

- Keep **Sans-I/O** as architecture baseline.
- Keep **`no_std + alloc`** support for shared/core crates.
- Keep **QUIC-first** while designing for **multi-transport** expansion.
- Keep `transports/quic` as a **single crate** with internal layering for logic and runtime ergonomics.
- Optimize for **DX** without compromising explicit architecture boundaries.
- Varint helpers live in `minip2p-identity` (public) and are re-exported through `minip2p-core` to avoid circular deps.
- `multistream-select` depends on `minip2p-core` for shared varint code rather than duplicating it.
- Dropped `segment` index fields from `MultiaddrError`/`PeerAddrError` -- the protocol name and value in the error message are sufficient context.
- Use `x509-cert` (RustCrypto) for cert generation/verification -- `no_std + alloc` compatible, replaces `rcgen` (std-only).
- `minip2p-tls` is transport-agnostic: generates and verifies libp2p TLS certs, reusable across QUIC/TCP/WS.
- Follow the libp2p TLS spec for QUIC peer authentication -- QUIC's built-in TLS handles encryption, Noise XX would be redundant.
- Relay-first approach for NAT traversal, with hole punch (DCUtR) as optimization on top.
