use std::collections::BTreeSet;
use std::io::Write;
use std::str::FromStr;

use minip2p_core::{Multiaddr, PeerAddr, Protocol};
use minip2p_identity::PeerId;
use minip2p_quic::{QuicNodeConfig, QuicTransport};
use minip2p_transport::{ConnectionId, PeerSendPolicy, Transport, TransportError, TransportEvent};
use tempfile::TempDir;

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
        .expect("create cert file")
        .write_all(cert.pem().as_bytes())
        .expect("write cert");

    std::fs::File::create(&key_path)
        .expect("create key file")
        .write_all(key_pair.serialize_pem().as_bytes())
        .expect("write key");

    let cert_str = cert_path.to_str().expect("cert path").to_string();
    let key_str = key_path.to_str().expect("key path").to_string();

    (dir, cert_str, key_str)
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

    let listener_node_config = QuicNodeConfig::dev_listener_with_tls(&cert_path, &key_path);

    let dialer_node_config = QuicNodeConfig::dev_dialer();

    let mut server = QuicTransport::new(listener_node_config, "127.0.0.1:0").expect("server bind");
    let client = QuicTransport::new(dialer_node_config, "127.0.0.1:0").expect("client bind");

    let server_addr = server.local_addr().expect("server local addr");
    let listen_ma = make_listen_multiaddr(server_addr.port());

    server.listen(&listen_ma).expect("server listen");

    let peer_addr = PeerAddr::new(listen_ma, test_peer_id()).expect("peer addr");

    (server, client, peer_addr, cert_dir)
}

fn drive_pair_once(
    server: &mut QuicTransport,
    client: &mut QuicTransport,
) -> (Vec<TransportEvent>, Vec<TransportEvent>) {
    std::thread::sleep(std::time::Duration::from_millis(5));
    let server_events = server.poll().expect("server poll");
    let client_events = client.poll().expect("client poll");
    (server_events, client_events)
}

fn wait_for_client_connection(
    server: &mut QuicTransport,
    client: &mut QuicTransport,
    expected: ConnectionId,
    peer_addr: &PeerAddr,
    max_iters: usize,
) -> bool {
    for _ in 0..max_iters {
        let (_, client_events) = drive_pair_once(server, client);
        for event in client_events {
            if let TransportEvent::Connected { id, endpoint } = event {
                if id == expected {
                    assert_eq!(endpoint.peer_id(), Some(peer_addr.peer_id()));
                    return true;
                }
            }
        }
    }

    false
}

#[test]
fn two_peers_connect_and_exchange_data() {
    let (mut server, mut client, peer_addr, _cert_dir) = setup_pair();

    let client_conn_id = ConnectionId::new(1);
    client
        .dial(client_conn_id, &peer_addr)
        .expect("client dial");

    let mut server_got_connection = false;
    let mut client_got_connected = false;
    let mut server_received = false;
    let mut client_received = false;

    let mut server_conn_id = ConnectionId::new(0);

    for _ in 0..200 {
        std::thread::sleep(std::time::Duration::from_millis(5));

        let server_events = server.poll().expect("server poll");
        for event in &server_events {
            match event {
                TransportEvent::IncomingConnection { id, endpoint } => {
                    server_got_connection = true;
                    server_conn_id = *id;
                    assert!(endpoint.peer_id().is_none());
                }
                TransportEvent::Connected { id, .. } => {
                    if !server_got_connection {
                        server_got_connection = true;
                        server_conn_id = *id;
                    }
                }
                TransportEvent::Received { data, .. } => {
                    assert_eq!(data, b"hello from client");
                    server_received = true;
                }
                _ => {}
            }
        }

        let client_events = client.poll().expect("client poll");
        for event in &client_events {
            match event {
                TransportEvent::Connected { endpoint, .. } => {
                    assert_eq!(endpoint.peer_id(), Some(peer_addr.peer_id()));
                    client_got_connected = true;
                }
                TransportEvent::Received { data, .. } => {
                    assert_eq!(data, b"hello from server");
                    client_received = true;
                }
                _ => {}
            }
        }

        if client_got_connected && server_got_connection {
            break;
        }
    }

    assert!(
        server_got_connection,
        "server should see incoming connection"
    );
    assert!(client_got_connected, "client should be connected");

    client
        .send(client_conn_id, b"hello from client".to_vec())
        .expect("client send");

    for _ in 0..200 {
        std::thread::sleep(std::time::Duration::from_millis(5));

        let server_events = server.poll().expect("server poll");
        for event in &server_events {
            if let TransportEvent::Received { data, .. } = event {
                assert_eq!(data, b"hello from client");
                server_received = true;
            }
        }

        if server_received {
            break;
        }
    }

    assert!(server_received, "server should receive client data");

    server
        .send(server_conn_id, b"hello from server".to_vec())
        .expect("server send");

    for _ in 0..200 {
        std::thread::sleep(std::time::Duration::from_millis(5));

        let client_events = client.poll().expect("client poll");
        for event in &client_events {
            if let TransportEvent::Received { data, .. } = event {
                assert_eq!(data, b"hello from server");
                client_received = true;
            }
        }

        if client_received {
            break;
        }
    }

    assert!(client_received, "client should receive server data");
}

#[test]
fn same_peer_supports_multiple_connections() {
    let (mut server, mut client, peer_addr, _cert_dir) = setup_pair();

    let conn_a = ConnectionId::new(10);
    let conn_b = ConnectionId::new(11);

    client.dial(conn_a, &peer_addr).expect("dial conn_a");
    client.dial(conn_b, &peer_addr).expect("dial conn_b");

    let mut client_connected = BTreeSet::new();
    let mut server_connection_ids = BTreeSet::new();

    for _ in 0..400 {
        std::thread::sleep(std::time::Duration::from_millis(5));

        let server_events = server.poll().expect("server poll");
        for event in server_events {
            match event {
                TransportEvent::IncomingConnection { id, endpoint } => {
                    assert!(endpoint.peer_id().is_none());
                    server_connection_ids.insert(id);
                }
                TransportEvent::Connected { id, .. } => {
                    server_connection_ids.insert(id);
                }
                _ => {}
            }
        }

        let client_events = client.poll().expect("client poll");
        for event in client_events {
            if let TransportEvent::Connected { id, endpoint } = event {
                assert_eq!(endpoint.peer_id(), Some(peer_addr.peer_id()));
                client_connected.insert(id);
            }
        }

        if client_connected.len() == 2 && server_connection_ids.len() >= 2 {
            break;
        }
    }

    assert_eq!(client_connected.len(), 2, "client should connect twice");
    assert!(
        server_connection_ids.len() >= 2,
        "server should track two incoming connections"
    );

    let known = client.connection_ids_for_peer(peer_addr.peer_id());
    assert_eq!(known, vec![conn_a, conn_b]);
    assert_eq!(
        client.primary_connection_for_peer(peer_addr.peer_id()),
        Some(conn_a)
    );

    let used_primary = client
        .send_to_peer(
            peer_addr.peer_id(),
            b"msg-a".to_vec(),
            PeerSendPolicy::Primary,
        )
        .expect("send primary");
    let used_newest = client
        .send_to_peer(
            peer_addr.peer_id(),
            b"msg-b".to_vec(),
            PeerSendPolicy::NewestConnected,
        )
        .expect("send newest");

    assert_eq!(used_primary, conn_a);
    assert_eq!(used_newest, conn_b);

    let mut got_a = false;
    let mut got_b = false;

    for _ in 0..400 {
        std::thread::sleep(std::time::Duration::from_millis(5));

        let server_events = server.poll().expect("server poll");
        for event in server_events {
            if let TransportEvent::Received { data, .. } = event {
                if data == b"msg-a" {
                    got_a = true;
                }
                if data == b"msg-b" {
                    got_b = true;
                }
            }
        }

        let _ = client.poll().expect("client poll");

        if got_a && got_b {
            break;
        }
    }

    assert!(got_a, "server should receive message on conn_a");
    assert!(got_b, "server should receive message on conn_b");
}

#[test]
fn listen_without_tls_returns_config_error_and_no_listening_event() {
    let config = QuicNodeConfig::dev_dialer();
    let mut transport = QuicTransport::new(config, "127.0.0.1:0").expect("bind");

    let local_port = transport.local_addr().expect("local addr").port();
    let listen_ma = make_listen_multiaddr(local_port);

    let err = transport
        .listen(&listen_ma)
        .expect_err("listen should fail");
    assert!(matches!(err, TransportError::InvalidConfig { .. }));

    let events = transport.poll().expect("poll should still work");
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, TransportEvent::Listening { .. })),
        "listening event must not be emitted on failed listen"
    );
}

#[test]
fn identity_upgrade_emits_verified_event_and_updates_peer_index() {
    let (mut server, mut client, peer_addr, _cert_dir) = setup_pair();

    let client_conn_id = ConnectionId::new(42);
    client
        .dial(client_conn_id, &peer_addr)
        .expect("client dial");

    let mut server_conn_id = None;
    let mut client_connected = false;

    for _ in 0..200 {
        std::thread::sleep(std::time::Duration::from_millis(5));

        for event in server.poll().expect("server poll") {
            match event {
                TransportEvent::IncomingConnection { id, endpoint } => {
                    assert!(endpoint.peer_id().is_none());
                    server_conn_id = Some(id);
                }
                TransportEvent::Connected { id, .. } => {
                    server_conn_id = Some(id);
                }
                _ => {}
            }
        }

        for event in client.poll().expect("client poll") {
            if let TransportEvent::Connected { .. } = event {
                client_connected = true;
            }
        }

        if server_conn_id.is_some() && client_connected {
            break;
        }
    }

    let server_conn_id = server_conn_id.expect("server connection id");
    assert!(client_connected, "client should be connected");

    let verified_peer_id = test_peer_id();
    server
        .verify_connection_peer_id(server_conn_id, verified_peer_id.clone())
        .expect("verify peer id");

    let events = server.poll().expect("server poll");
    let verified_event = events
        .iter()
        .find_map(|event| {
            if let TransportEvent::PeerIdentityVerified {
                id,
                endpoint,
                previous_peer_id,
            } = event
            {
                Some((*id, endpoint.clone(), previous_peer_id.clone()))
            } else {
                None
            }
        })
        .expect("peer identity verified event");

    assert_eq!(verified_event.0, server_conn_id);
    assert_eq!(verified_event.1.peer_id(), Some(&verified_peer_id));
    assert!(verified_event.2.is_none());

    let indexed = server.connection_ids_for_peer(&verified_peer_id);
    assert_eq!(indexed, vec![server_conn_id]);
}

#[test]
fn dial_supports_dns4_target_for_quic_transport() {
    let (mut server, mut client, peer_addr, _cert_dir) = setup_pair();

    let port = match peer_addr.transport().protocols().get(1) {
        Some(Protocol::Udp(port)) => *port,
        _ => panic!("peer transport must contain udp port"),
    };

    let dns_transport = Multiaddr::from_protocols(vec![
        Protocol::Dns4("localhost".to_string()),
        Protocol::Udp(port),
        Protocol::QuicV1,
    ]);
    let dns_peer_addr =
        PeerAddr::new(dns_transport, peer_addr.peer_id().clone()).expect("dns peer addr");

    let conn_id = ConnectionId::new(77);
    client.dial(conn_id, &dns_peer_addr).expect("dial via dns4");

    let connected =
        wait_for_client_connection(&mut server, &mut client, conn_id, &dns_peer_addr, 200);
    assert!(connected, "client should connect via dns4 target");
}

#[test]
fn listen_rejects_address_mismatch_with_bound_socket() {
    let (_cert_dir, cert_path, key_path) = generate_cert_pair();
    let config = QuicNodeConfig::dev_listener_with_tls(&cert_path, &key_path);
    let mut transport = QuicTransport::new(config, "127.0.0.1:0").expect("bind");

    let local = transport.local_addr().expect("local addr");
    let mismatched_port = if local.port() == u16::MAX {
        local.port() - 1
    } else {
        local.port() + 1
    };

    let listen_ma = make_listen_multiaddr(mismatched_port);
    let err = transport
        .listen(&listen_ma)
        .expect_err("listen with mismatched address should fail");

    assert!(matches!(
        err,
        TransportError::InvalidAddress {
            context: "listen address",
            ..
        }
    ));

    let events = transport.poll().expect("poll should still work");
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, TransportEvent::Listening { .. })),
        "listening event must not be emitted on failed listen"
    );
}

#[test]
fn peer_send_policy_prefers_connection_age_over_connection_id_order() {
    let (mut server, mut client, peer_addr, _cert_dir) = setup_pair();

    let older_conn = ConnectionId::new(100);
    let newer_conn = ConnectionId::new(1);

    client
        .dial(older_conn, &peer_addr)
        .expect("dial older conn");
    let older_connected =
        wait_for_client_connection(&mut server, &mut client, older_conn, &peer_addr, 250);
    assert!(older_connected, "older connection should establish");

    client
        .dial(newer_conn, &peer_addr)
        .expect("dial newer conn");
    let newer_connected =
        wait_for_client_connection(&mut server, &mut client, newer_conn, &peer_addr, 250);
    assert!(newer_connected, "newer connection should establish");

    let used_primary = client
        .send_to_peer(
            peer_addr.peer_id(),
            b"primary-oldest".to_vec(),
            PeerSendPolicy::Primary,
        )
        .expect("send with primary policy");
    let used_oldest = client
        .send_to_peer(
            peer_addr.peer_id(),
            b"explicit-oldest".to_vec(),
            PeerSendPolicy::OldestConnected,
        )
        .expect("send with oldest policy");
    let used_newest = client
        .send_to_peer(
            peer_addr.peer_id(),
            b"newest".to_vec(),
            PeerSendPolicy::NewestConnected,
        )
        .expect("send with newest policy");

    assert_eq!(used_primary, older_conn);
    assert_eq!(used_oldest, older_conn);
    assert_eq!(used_newest, newer_conn);
}

#[test]
fn large_payload_is_delivered_as_single_received_event() {
    let (mut server, mut client, peer_addr, _cert_dir) = setup_pair();

    let conn_id = ConnectionId::new(501);
    client.dial(conn_id, &peer_addr).expect("dial");
    let connected = wait_for_client_connection(&mut server, &mut client, conn_id, &peer_addr, 250);
    assert!(connected, "client should connect");

    let payload = vec![42u8; 32 * 1024];
    client
        .send(conn_id, payload.clone())
        .expect("send large payload");

    for _ in 0..250 {
        let (server_events, _) = drive_pair_once(&mut server, &mut client);

        let received: Vec<Vec<u8>> = server_events
            .into_iter()
            .filter_map(|event| {
                if let TransportEvent::Received { data, .. } = event {
                    Some(data)
                } else {
                    None
                }
            })
            .collect();

        if !received.is_empty() {
            assert_eq!(received.len(), 1, "payload should arrive as one message");
            assert_eq!(received[0], payload, "payload bytes should be preserved");
            return;
        }
    }

    panic!("server did not receive large payload in time");
}
