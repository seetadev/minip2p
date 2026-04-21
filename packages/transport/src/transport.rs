use alloc::vec::Vec;

use minip2p_core::{Multiaddr, PeerAddr};

use crate::{ConnectionId, StreamId, TransportError, TransportEvent};

/// The core transport abstraction.
///
/// Concrete adapters (QUIC, WebSocket, etc.) implement this trait. The host
/// drives the transport by calling [`poll`](Transport::poll) and reacting to
/// [`TransportEvent`](crate::TransportEvent)s.
pub trait Transport {
    /// Initiate an outbound connection.
    fn dial(&mut self, id: ConnectionId, addr: &PeerAddr) -> Result<(), TransportError>;

    /// Start listening for inbound connections on the given address.
    fn listen(&mut self, addr: &Multiaddr) -> Result<(), TransportError>;

    /// Open a new bidirectional stream on an existing connection.
    fn open_stream(&mut self, id: ConnectionId) -> Result<StreamId, TransportError>;

    /// Write data to a stream.
    fn send_stream(
        &mut self,
        id: ConnectionId,
        stream_id: StreamId,
        data: Vec<u8>,
    ) -> Result<(), TransportError>;

    /// Half-close the write side of a stream (send FIN).
    fn close_stream_write(
        &mut self,
        id: ConnectionId,
        stream_id: StreamId,
    ) -> Result<(), TransportError>;

    /// Abruptly reset a stream in both directions.
    fn reset_stream(&mut self, id: ConnectionId, stream_id: StreamId)
    -> Result<(), TransportError>;

    /// Gracefully close a connection.
    fn close(&mut self, id: ConnectionId) -> Result<(), TransportError>;

    /// Drive the transport forward and return any pending events.
    fn poll(&mut self) -> Result<Vec<TransportEvent>, TransportError>;
}
