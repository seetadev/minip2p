use alloc::string::String;
use alloc::vec::Vec;

use minip2p_core::{Multiaddr, PeerId};

use crate::{ConnectionEndpoint, ConnectionId, StreamId};

/// Events emitted by a transport back to the host.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TransportEvent {
    Connected {
        id: ConnectionId,
        endpoint: ConnectionEndpoint,
    },
    StreamOpened {
        id: ConnectionId,
        stream_id: StreamId,
    },
    IncomingStream {
        id: ConnectionId,
        stream_id: StreamId,
    },
    StreamData {
        id: ConnectionId,
        stream_id: StreamId,
        data: Vec<u8>,
    },
    StreamRemoteWriteClosed {
        id: ConnectionId,
        stream_id: StreamId,
    },
    StreamClosed {
        id: ConnectionId,
        stream_id: StreamId,
    },
    Closed {
        id: ConnectionId,
    },
    Error {
        id: ConnectionId,
        message: String,
    },
    IncomingConnection {
        id: ConnectionId,
        endpoint: ConnectionEndpoint,
    },
    PeerIdentityVerified {
        id: ConnectionId,
        endpoint: ConnectionEndpoint,
        previous_peer_id: Option<PeerId>,
    },
    Listening {
        addr: Multiaddr,
    },
}
