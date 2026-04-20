use alloc::string::String;
use alloc::vec::Vec;

use minip2p_core::Multiaddr;

use crate::{ConnectionEndpoint, ConnectionId};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TransportEvent {
    Connected {
        id: ConnectionId,
        endpoint: ConnectionEndpoint,
    },
    Received {
        id: ConnectionId,
        data: Vec<u8>,
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
    Listening {
        addr: Multiaddr,
    },
}
