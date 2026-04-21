use alloc::vec::Vec;

use minip2p_core::{Multiaddr, PeerAddr};

use crate::{ConnectionId, StreamId};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TransportAction {
    Dial {
        id: ConnectionId,
        addr: PeerAddr,
    },
    Listen {
        addr: Multiaddr,
    },
    OpenStream {
        id: ConnectionId,
    },
    SendStream {
        id: ConnectionId,
        stream_id: StreamId,
        data: Vec<u8>,
    },
    CloseStreamWrite {
        id: ConnectionId,
        stream_id: StreamId,
    },
    ResetStream {
        id: ConnectionId,
        stream_id: StreamId,
    },
    Close {
        id: ConnectionId,
    },
}
