//! Transport contract conformance tests.
//!
//! These tests verify the guarantees documented on the `Transport` trait.
//! They run against the QUIC adapter but should pass for any conforming
//! transport implementation.

use std::io::Write;

use minip2p_core::{Multiaddr, PeerAddr, Protocol};
use minip2p_identity::PeerId;
use minip2p_quic::{QuicNodeConfig, QuicTransport};
use minip2p_transport::{ConnectionId, Transport, TransportError, TransportEvent};
use std::str::FromStr;
use tempfile::TempDir;

// ---------------------------------------------------------------------------
// Helpers (shared with two_peer.rs; duplicated to keep test files independent)
// ---------------------------------------------------------------------------

fn generate_cert_pair() -> (TempDir, String, String) {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut params = rcgen::CertificateParams::new(Vec::new()).expect("params");
    params
        .distinguished_name
        .push(rcgen::DnType::CommonName, "minip2p-test");
    let key_pair = rcgen::KeyPair::generate().expect("keypair");
    let cert = params.self_signed(&key_pair).expect("cert");

    let cert_path = dir.path().join("cert.pem");
    let key_path = dir.path().join("key.pem");
    std::fs::File::create(&cert_path)
        .expect("create cert")
        .write_all(cert.pem().as_bytes())
        .expect("write cert");
    std::fs::File::create(&key_path)
        .expect("create key")
        .write_all(key_pair.serialize_pem().as_bytes())
        .expect("write key");

    (
        dir,
        cert_path.to_str().unwrap().to_string(),
        key_path.to_str().unwrap().to_string(),
    )
}

fn make_listen_multiaddr(port: u16) -> Multiaddr {
    Multiaddr::from_protocols(vec![
        Protocol::Ip4([127, 0, 0, 1]),
        Protocol::Udp(port),
        Protocol::QuicV1,
    ])
}

fn test_peer_id() -> PeerId {
    PeerId::from_str("QmYyQSo1c1Ym7orWxLYvCrM2EmxFTANf8wXmmE7DWjhx5N").expect("peer id")
}

fn setup_pair() -> (QuicTransport, QuicTransport, PeerAddr, TempDir) {
    let (cert_dir, cert_path, key_path) = generate_cert_pair();
    let server_cfg = QuicNodeConfig::dev_listener_with_tls(&cert_path, &key_path);
    let client_cfg = QuicNodeConfig::dev_dialer();

    let mut server = QuicTransport::new(server_cfg, "127.0.0.1:0").expect("server");
    let client = QuicTransport::new(client_cfg, "127.0.0.1:0").expect("client");

    let port = server.local_addr().expect("addr").port();
    let listen_ma = make_listen_multiaddr(port);
    server.listen(&listen_ma).expect("listen");

    let peer_addr = PeerAddr::new(listen_ma, test_peer_id()).expect("peer addr");
    (server, client, peer_addr, cert_dir)
}

fn drive_pair_once(
    server: &mut QuicTransport,
    client: &mut QuicTransport,
) -> (Vec<TransportEvent>, Vec<TransportEvent>) {
    std::thread::sleep(std::time::Duration::from_millis(5));
    let s = server.poll().expect("server poll");
    let c = client.poll().expect("client poll");
    (s, c)
}

/// Drives the pair until both sides report Connected, collecting all events.
fn connect_pair(
    server: &mut QuicTransport,
    client: &mut QuicTransport,
    peer_addr: &PeerAddr,
) -> (ConnectionId, ConnectionId, Vec<TransportEvent>, Vec<TransportEvent>) {
    let client_conn = ConnectionId::new(1);
    client.dial(client_conn, peer_addr).expect("dial");

    let mut all_server_events = Vec::new();
    let mut all_client_events = Vec::new();
    let mut server_conn = None;
    let mut server_connected = false;
    let mut client_connected = false;

    for _ in 0..100 {
        let (se, ce) = drive_pair_once(server, client);
        for e in &se {
            match e {
                TransportEvent::IncomingConnection { id, .. } => {
                    if server_conn.is_none() {
                        server_conn = Some(*id);
                    }
                }
                TransportEvent::Connected { id, .. } => {
                    if server_conn.is_none() {
                        server_conn = Some(*id);
                    }
                    if server_conn == Some(*id) {
                        server_connected = true;
                    }
                }
                _ => {}
            }
        }
        for e in &ce {
            if let TransportEvent::Connected { id, .. } = e {
                if *id == client_conn {
                    client_connected = true;
                }
            }
        }
        all_server_events.extend(se);
        all_client_events.extend(ce);
        if client_connected && server_connected {
            break;
        }
    }

    assert!(client_connected, "client must connect");
    assert!(server_connected, "server must connect");
    let server_conn = server_conn.expect("server must accept");
    (server_conn, client_conn, all_server_events, all_client_events)
}

// ---------------------------------------------------------------------------
// Connection lifecycle
// ---------------------------------------------------------------------------

#[test]
fn connected_is_emitted_exactly_once_after_dial() {
    let (mut server, mut client, peer_addr, _dir) = setup_pair();
    let (_, _, _, client_events) = connect_pair(&mut server, &mut client, &peer_addr);

    let connected_count = client_events
        .iter()
        .filter(|e| matches!(e, TransportEvent::Connected { .. }))
        .count();
    assert_eq!(connected_count, 1, "Connected must be emitted exactly once");
}

#[test]
fn incoming_connection_precedes_connected_on_server() {
    let (mut server, mut client, peer_addr, _dir) = setup_pair();
    let (server_conn, _, server_events, _) = connect_pair(&mut server, &mut client, &peer_addr);

    let incoming_idx = server_events
        .iter()
        .position(|e| matches!(e, TransportEvent::IncomingConnection { id, .. } if *id == server_conn));
    let connected_idx = server_events
        .iter()
        .position(|e| matches!(e, TransportEvent::Connected { id, .. } if *id == server_conn));

    assert!(
        incoming_idx.is_some(),
        "server must emit IncomingConnection"
    );
    assert!(connected_idx.is_some(), "server must emit Connected");
    assert!(
        incoming_idx.unwrap() < connected_idx.unwrap(),
        "IncomingConnection must precede Connected"
    );
}

#[test]
fn no_stream_events_before_connected() {
    let (mut server, mut client, peer_addr, _dir) = setup_pair();
    let (_, _, _, client_events) = connect_pair(&mut server, &mut client, &peer_addr);

    let connected_idx = client_events
        .iter()
        .position(|e| matches!(e, TransportEvent::Connected { .. }))
        .expect("must have Connected");

    for event in &client_events[..connected_idx] {
        assert!(
            !matches!(
                event,
                TransportEvent::StreamOpened { .. }
                    | TransportEvent::IncomingStream { .. }
                    | TransportEvent::StreamData { .. }
                    | TransportEvent::StreamRemoteWriteClosed { .. }
                    | TransportEvent::StreamClosed { .. }
            ),
            "stream event {event:?} emitted before Connected"
        );
    }
}

#[test]
fn dial_with_duplicate_id_returns_connection_exists() {
    let (mut server, mut client, peer_addr, _dir) = setup_pair();
    let id = ConnectionId::new(42);
    client.dial(id, &peer_addr).expect("first dial");

    let err = client.dial(id, &peer_addr).expect_err("duplicate must fail");
    assert!(
        matches!(err, TransportError::ConnectionExists { .. }),
        "expected ConnectionExists, got {err:?}"
    );

    // Drive to avoid dangling state.
    for _ in 0..20 {
        drive_pair_once(&mut server, &mut client);
    }
}

#[test]
fn close_rejects_further_stream_operations() {
    let (mut server, mut client, peer_addr, _dir) = setup_pair();
    let (_, client_conn, _, _) = connect_pair(&mut server, &mut client, &peer_addr);

    // Open a stream first so we can test send after close.
    let stream_id = client.open_stream(client_conn).expect("open stream");
    client
        .send_stream(client_conn, stream_id, b"data".to_vec())
        .expect("send before close");

    client.close(client_conn).expect("close");

    // After close(), the connection is in Closing state. Opening new streams
    // or sending on existing ones should fail.
    let err = client.open_stream(client_conn);
    assert!(
        err.is_err(),
        "open_stream must fail after close"
    );
}

// ---------------------------------------------------------------------------
// Stream lifecycle
// ---------------------------------------------------------------------------

#[test]
fn open_stream_emits_stream_opened() {
    let (mut server, mut client, peer_addr, _dir) = setup_pair();
    let (_, client_conn, _, _) = connect_pair(&mut server, &mut client, &peer_addr);

    let stream_id = client.open_stream(client_conn).expect("open stream");

    // StreamOpened should appear in the next poll.
    let (_, ce) = drive_pair_once(&mut server, &mut client);
    // It may also be in the pending_events from open_stream itself, so check both.
    let found = ce
        .iter()
        .any(|e| matches!(e, TransportEvent::StreamOpened { id, stream_id: sid } if *id == client_conn && *sid == stream_id));

    // If not in poll, check the open_stream already queued it (it does).
    // Either way the contract is: StreamOpened is emitted.
    assert!(
        found
            || client
                .poll()
                .unwrap()
                .iter()
                .any(|e| matches!(e, TransportEvent::StreamOpened { .. })),
        "StreamOpened must be emitted"
    );
}

#[test]
fn incoming_stream_precedes_stream_data() {
    let (mut server, mut client, peer_addr, _dir) = setup_pair();
    let (server_conn, client_conn, _, _) = connect_pair(&mut server, &mut client, &peer_addr);

    let stream_id = client.open_stream(client_conn).expect("open stream");
    client
        .send_stream(client_conn, stream_id, b"hello".to_vec())
        .expect("send");

    // Drive until server sees data.
    let mut server_events = Vec::new();
    for _ in 0..50 {
        let (se, _) = drive_pair_once(&mut server, &mut client);
        server_events.extend(se);
        if server_events
            .iter()
            .any(|e| matches!(e, TransportEvent::StreamData { .. }))
        {
            break;
        }
    }

    let incoming_idx = server_events
        .iter()
        .position(|e| matches!(e, TransportEvent::IncomingStream { id, .. } if *id == server_conn));
    let data_idx = server_events
        .iter()
        .position(|e| matches!(e, TransportEvent::StreamData { id, .. } if *id == server_conn));

    assert!(incoming_idx.is_some(), "must emit IncomingStream");
    assert!(data_idx.is_some(), "must emit StreamData");
    assert!(
        incoming_idx.unwrap() < data_idx.unwrap(),
        "IncomingStream must precede StreamData"
    );
}

#[test]
fn close_stream_write_produces_remote_write_closed() {
    let (mut server, mut client, peer_addr, _dir) = setup_pair();
    let (server_conn, client_conn, _, _) = connect_pair(&mut server, &mut client, &peer_addr);

    let stream_id = client.open_stream(client_conn).expect("open stream");
    client
        .send_stream(client_conn, stream_id, b"data".to_vec())
        .expect("send");
    client
        .close_stream_write(client_conn, stream_id)
        .expect("close write");

    let mut saw_remote_write_closed = false;
    for _ in 0..50 {
        let (se, _) = drive_pair_once(&mut server, &mut client);
        if se.iter().any(|e| {
            matches!(e, TransportEvent::StreamRemoteWriteClosed { id, .. } if *id == server_conn)
        }) {
            saw_remote_write_closed = true;
            break;
        }
    }
    assert!(
        saw_remote_write_closed,
        "server must see StreamRemoteWriteClosed"
    );
}

#[test]
fn reset_stream_emits_stream_closed() {
    let (mut server, mut client, peer_addr, _dir) = setup_pair();
    let (_, client_conn, _, _) = connect_pair(&mut server, &mut client, &peer_addr);

    let stream_id = client.open_stream(client_conn).expect("open stream");

    // Send data so quiche registers the stream internally.
    client
        .send_stream(client_conn, stream_id, b"hello".to_vec())
        .expect("send");

    // Drive so the stream is fully established on the wire.
    for _ in 0..10 {
        drive_pair_once(&mut server, &mut client);
    }

    client
        .reset_stream(client_conn, stream_id)
        .expect("reset");

    // StreamClosed should appear in pending events or next poll.
    let mut saw_closed = false;
    let events = client.poll().unwrap();
    if events.iter().any(|e| {
        matches!(e, TransportEvent::StreamClosed { id, stream_id: sid } if *id == client_conn && *sid == stream_id)
    }) {
        saw_closed = true;
    }

    if !saw_closed {
        for _ in 0..20 {
            let (_, ce) = drive_pair_once(&mut server, &mut client);
            if ce.iter().any(|e| {
                matches!(e, TransportEvent::StreamClosed { id, stream_id: sid } if *id == client_conn && *sid == stream_id)
            }) {
                saw_closed = true;
                break;
            }
        }
    }
    assert!(saw_closed, "reset_stream must emit StreamClosed");
}

// ---------------------------------------------------------------------------
// Error conditions
// ---------------------------------------------------------------------------

#[test]
fn open_stream_on_unknown_connection_returns_not_found() {
    let (_dir, cert, key) = generate_cert_pair();
    let cfg = QuicNodeConfig::dev_listener_with_tls(&cert, &key);
    let mut transport = QuicTransport::new(cfg, "127.0.0.1:0").expect("bind");

    let err = transport
        .open_stream(ConnectionId::new(999))
        .expect_err("must fail");
    assert!(matches!(err, TransportError::ConnectionNotFound { .. }));
}

#[test]
fn send_on_unknown_connection_returns_not_found() {
    let (_dir, cert, key) = generate_cert_pair();
    let cfg = QuicNodeConfig::dev_listener_with_tls(&cert, &key);
    let mut transport = QuicTransport::new(cfg, "127.0.0.1:0").expect("bind");

    let err = transport
        .send_stream(ConnectionId::new(999), 0.into(), b"data".to_vec())
        .expect_err("must fail");
    assert!(matches!(err, TransportError::ConnectionNotFound { .. }));
}

#[test]
fn poll_returns_empty_when_idle() {
    let (_dir, cert, key) = generate_cert_pair();
    let cfg = QuicNodeConfig::dev_listener_with_tls(&cert, &key);
    let mut transport = QuicTransport::new(cfg, "127.0.0.1:0").expect("bind");

    let events = transport.poll().expect("poll");
    assert!(events.is_empty(), "idle poll must return empty vec");
}
