# minip2p

A minimal, educational [libp2p](https://libp2p.io/) implementation in Rust.

The project is built around four non-negotiables:

- **Sans-I/O architecture** for protocol and transport state machines.
- **`no_std`-friendly core crates** (`alloc`-based where needed).
- **Top-notch DX** with clear defaults, actionable errors, and easy local bring-up.
- **FFI-friendly APIs** so Rust boundaries can later map cleanly to WASM/TypeScript hosts.

## Workspace status

Current crates:

- `packages/identity` (`minip2p-identity`): peer identity primitives and validation.
- `packages/core` (`minip2p-core`): transport-agnostic address/types (`Multiaddr`, `PeerAddr`, `PeerId` re-export).
- `packages/transport` (`minip2p-transport`): transport contract types, connection+stream lifecycle, events, and errors.
- `packages/multistream-select` (`minip2p-multistream-select`): Sans-I/O protocol negotiation state machine (`/multistream/1.0.0`).
- `packages/ping` (`minip2p-ping`): Sans-I/O ping protocol state machine (`/ipfs/ping/1.0.0`).
- `transports/quic` (`minip2p-quic`): `std` QUIC transport adapter built on `quiche`.

Current validated behavior:

- Two local peers connect over QUIC in integration tests.
- Peers open QUIC streams and exchange stream data bidirectionally.
- Stream half-close (close write) propagates as transport events.
- Identity upgrade events are emitted and peer indexing updates correctly.
- Multistream-select negotiation with spec-compliant varint-length-prefixed framing.
- Ping protocol round-trips with RTT measurement over negotiated streams.
- End-to-end tests: QUIC transport + multistream-select + ping protocol.

## Architecture boundaries

- `packages/identity`, `packages/core`, and `packages/transport` are designed to remain `no_std + alloc` friendly.
- Runtime networking concerns (sockets, DNS, timers) belong in transport adapter crates.
- Transport-specific address validation belongs in transport adapters, not `packages/core`.

## Quick start

Prerequisites:

- Rust stable toolchain
- Cargo

Build and run tests:

```bash
cargo test
```

## Documentation

Every crate has a README and rustdoc on all public APIs. Internal methods and types are commented for contributor onboarding.

Generate the full API docs with:

```bash
cargo doc --workspace --no-deps --open
```

## Roadmap focus

- [x] Local QUIC connectivity and integration coverage.
- [x] Multistream-select with spec-compliant varint framing.
- [x] Ping protocol with RTT measurement and timeout handling.
- [x] End-to-end protocol stack tests (QUIC + multistream + ping).
- [x] Rustdoc and internal comments across all crates.
- [ ] Harden transport contract guarantees for long-term multi-transport support.
- [ ] Improve QUIC ergonomics and operational diagnostics.
- [ ] Add core protocols (Noise XX, identify, gossipsub).
- [ ] Introduce additional transport adapters (TCP, WebSocket, WebRTC).

See `plan.md` for the detailed execution plan and milestones.
