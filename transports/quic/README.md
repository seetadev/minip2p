# minip2p-quic

Synchronous QUIC transport adapter for minip2p, powered by [quiche](https://github.com/cloudflare/quiche).

No async runtime required. The host calls `poll()` in their own loop.

## Features

- Implements the `minip2p_transport::Transport` trait.
- Synchronous, non-blocking UDP socket — no tokio, no futures.
- Self-signed TLS certificate support via PEM files on disk.
- Server-side incoming connection detection.
- Bidirectional stream send/recv.
- `verify_peer(false)` for local development and testing.

## Usage

### Basic server + client

```rust
use minip2p_core::{Multiaddr, PeerAddr, Protocol};
use minip2p_identity::PeerId;
use minip2p_quic::{QuicConfig, QuicTransport};
use minip2p_transport::{ConnectionId, Transport, TransportEvent};

// --- Server ---
let server_config = QuicConfig::new()
    .with_cert_paths("/path/to/cert.pem", "/path/to/key.pem")
    .verify_peer(false);

let mut server = QuicTransport::new(server_config, "127.0.0.1:0")
    .expect("server bind");

let server_addr = server.local_addr().expect("local addr");
let listen_addr = Multiaddr::from_protocols(vec![
    Protocol::Ip4([127, 0, 0, 1]),
    Protocol::Udp(server_addr.port()),
    Protocol::QuicV1,
]);

server.listen(&listen_addr).expect("listen");

// --- Client ---
let client_config = QuicConfig::new().verify_peer(false);
let mut client = QuicTransport::new(client_config, "127.0.0.1:0")
    .expect("client bind");

let dummy_peer_id = PeerId::from_bytes(&[0x00, 0x04, 0x01, 0x02, 0x03, 0x04])
    .expect("peer id");

let peer_addr = PeerAddr::new(listen_addr.clone(), dummy_peer_id)
    .expect("peer addr");

let conn_id = ConnectionId::new(1);
client.dial(conn_id, &peer_addr).expect("dial");
```

### Driving the event loop

Call `poll()` in a loop. No async runtime needed — just a regular `loop`:

```rust
use minip2p_transport::{ConnectionId, Transport, TransportEvent};
use std::time::Duration;

fn drive(transport: &mut impl Transport) {
    loop {
        let events = transport.poll().expect("poll failure");

        for event in events {
            match event {
                TransportEvent::Connected { id, .. } => {
                    println!("connected: {id}");
                }
                TransportEvent::Received { id, data } => {
                    println!("got {} bytes on {id}", data.len());
                }
                TransportEvent::IncomingConnection { id, .. } => {
                    println!("incoming: {id}");
                }
                TransportEvent::Closed { id } => {
                    println!("closed: {id}");
                    return;
                }
                _ => {}
            }
        }

        std::thread::sleep(Duration::from_millis(1));
    }
}
```

### Sending data

```rust
use minip2p_transport::{ConnectionId, Transport};

fn send_message(
    transport: &mut impl Transport,
    conn_id: ConnectionId,
    msg: &[u8],
) {
    transport
        .send(conn_id, msg.to_vec())
        .expect("send");
}
```

### Closing a connection

```rust
use minip2p_transport::{ConnectionId, Transport};

fn close_connection(
    transport: &mut impl Transport,
    conn_id: ConnectionId,
) {
    transport.close(conn_id).expect("close");
    // TransportEvent::Closed will be emitted on the next poll()
    // when the connection is fully closed.
}
```

## Configuration

`QuicConfig` is a builder:

| Method | Description |
|--------|-------------|
| `QuicConfig::new()` | Default config, no TLS, `verify_peer(false)`. |
| `.with_cert_paths(cert, key)` | PEM file paths for server TLS. Required for `listen()`. |
| `.verify_peer(bool)` | Enable/disable peer certificate verification. Default: `false`. |

```rust
use minip2p_quic::QuicConfig;

let config = QuicConfig::new()
    .with_cert_paths("cert.pem", "key.pem")
    .verify_peer(true);
```

## TLS certificates

quiche requires TLS certificates loaded from PEM files on disk. For local development, generate self-signed certs at runtime:

```rust
use std::io::Write;

let mut params = rcgen::CertificateParams::new(Vec::new())?;
params.distinguished_name.push(rcgen::DnType::CommonName, "minip2p");
let key_pair = rcgen::KeyPair::generate()?;
let cert = params.self_signed(&key_pair)?;

std::fs::write("cert.pem", cert.pem())?;
std::fs::write("key.pem", key_pair.serialize_pem())?;
```

## Scope

This crate is a concrete transport adapter. It depends on `std` (quiche links BoringSSL and requires a UDP socket). For the transport interface without runtime dependencies, see `minip2p-transport`.
