use alloc::vec::Vec;

use minip2p_core::{Multiaddr, PeerAddr};

use crate::{ConnectionId, StreamId, TransportError, TransportEvent};

pub trait Transport {
    fn dial(&mut self, id: ConnectionId, addr: &PeerAddr) -> Result<(), TransportError>;

    fn listen(&mut self, addr: &Multiaddr) -> Result<(), TransportError>;

    fn open_stream(&mut self, id: ConnectionId) -> Result<StreamId, TransportError>;

    fn send_stream(
        &mut self,
        id: ConnectionId,
        stream_id: StreamId,
        data: Vec<u8>,
    ) -> Result<(), TransportError>;

    fn close_stream_write(
        &mut self,
        id: ConnectionId,
        stream_id: StreamId,
    ) -> Result<(), TransportError>;

    fn reset_stream(&mut self, id: ConnectionId, stream_id: StreamId)
    -> Result<(), TransportError>;

    fn close(&mut self, id: ConnectionId) -> Result<(), TransportError>;

    fn poll(&mut self) -> Result<Vec<TransportEvent>, TransportError>;
}
