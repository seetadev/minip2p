use std::collections::{HashMap, VecDeque};
use std::net::{SocketAddr, UdpSocket};

use minip2p_core::PeerId;
use minip2p_transport::{
    ConnectionEndpoint, ConnectionId, ConnectionState, StreamId, TransportError, TransportEvent,
};

const SEND_BUF_SIZE: usize = 1350;

#[derive(Clone, Debug)]
struct PendingStreamWrite {
    bytes: Vec<u8>,
    offset: usize,
    fin: bool,
}

impl PendingStreamWrite {
    fn data(bytes: Vec<u8>) -> Self {
        Self {
            bytes,
            offset: 0,
            fin: false,
        }
    }

    fn fin() -> Self {
        Self {
            bytes: Vec::new(),
            offset: 0,
            fin: true,
        }
    }
}

#[derive(Clone, Debug, Default)]
struct StreamRuntimeState {
    pending_writes: VecDeque<PendingStreamWrite>,
    local_write_closed: bool,
    remote_write_closed: bool,
    incoming_notified: bool,
    closed_notified: bool,
}

impl StreamRuntimeState {
    fn on_local_write_closed(&mut self) {
        self.local_write_closed = true;
    }

    fn on_remote_write_closed(&mut self) {
        self.remote_write_closed = true;
    }

    fn is_fully_closed(&self) -> bool {
        self.local_write_closed && self.remote_write_closed
    }
}

pub struct QuicConnection {
    id: ConnectionId,
    conn: quiche::Connection,
    peer_addr: SocketAddr,
    endpoint: ConnectionEndpoint,
    state: ConnectionState,
    stream_states: HashMap<u64, StreamRuntimeState>,
    next_local_bidi_stream_id: u64,
}

impl QuicConnection {
    pub fn new(
        id: ConnectionId,
        conn: quiche::Connection,
        peer_addr: SocketAddr,
        endpoint: ConnectionEndpoint,
    ) -> Self {
        let next_local_bidi_stream_id = if conn.is_server() { 1 } else { 0 };

        Self {
            id,
            conn,
            peer_addr,
            endpoint,
            state: ConnectionState::Connecting,
            stream_states: HashMap::new(),
            next_local_bidi_stream_id,
        }
    }

    pub fn source_cid_bytes(&self) -> Vec<u8> {
        self.conn.source_id().as_ref().to_vec()
    }

    pub fn endpoint(&self) -> &ConnectionEndpoint {
        &self.endpoint
    }

    pub fn set_peer_id(&mut self, peer_id: PeerId) {
        self.endpoint.set_peer_id(peer_id);
    }

    pub fn is_closed(&self) -> bool {
        self.conn.is_closed()
    }

    pub fn recv_packet(
        &mut self,
        buf: &mut [u8],
        from: SocketAddr,
        local: SocketAddr,
        socket: &UdpSocket,
        events: &mut Vec<TransportEvent>,
    ) -> Result<(), TransportError> {
        let recv_info = quiche::RecvInfo { from, to: local };

        match self.conn.recv(buf, recv_info) {
            Ok(_) => {}
            Err(quiche::Error::Done) => {}
            Err(e) => {
                events.push(TransportEvent::Error {
                    id: self.id,
                    message: format!("recv error: {e}"),
                });
                return Ok(());
            }
        }

        if self.state == ConnectionState::Connecting && self.conn.is_established() {
            self.state = ConnectionState::Connected;
            self.drain_send_queue(events)?;
            self.flush(socket)?;

            events.push(TransportEvent::Connected {
                id: self.id,
                endpoint: self.endpoint.clone(),
            });
        } else {
            self.drain_send_queue(events)?;
            self.flush(socket)?;
        }

        Ok(())
    }

    pub fn open_stream(&mut self) -> Result<StreamId, TransportError> {
        if self.state != ConnectionState::Connected {
            return Err(TransportError::InvalidState {
                id: self.id,
                state: self.state,
                expected: ConnectionState::Connected,
            });
        }

        let raw_stream_id = self.next_local_bidi_stream_id;
        self.next_local_bidi_stream_id = self.next_local_bidi_stream_id.wrapping_add(4);

        if self.stream_states.contains_key(&raw_stream_id) {
            return Err(TransportError::StreamExists {
                id: self.id,
                stream_id: StreamId::new(raw_stream_id),
            });
        }

        self.stream_states
            .insert(raw_stream_id, StreamRuntimeState::default());
        Ok(StreamId::new(raw_stream_id))
    }

    pub fn send_stream(
        &mut self,
        stream_id: StreamId,
        data: Vec<u8>,
        socket: &UdpSocket,
        events: &mut Vec<TransportEvent>,
    ) -> Result<(), TransportError> {
        if data.is_empty() {
            return Ok(());
        }

        let state = self.get_or_create_local_stream_state(stream_id)?;
        if state.local_write_closed {
            return Err(TransportError::StreamSendFailed {
                id: self.id,
                stream_id,
                reason: "local stream write side is already closed".into(),
            });
        }

        state
            .pending_writes
            .push_back(PendingStreamWrite::data(data));

        self.drain_send_queue(events)?;
        self.flush(socket)?;
        Ok(())
    }

    pub fn close_stream_write(
        &mut self,
        stream_id: StreamId,
        socket: &UdpSocket,
        events: &mut Vec<TransportEvent>,
    ) -> Result<(), TransportError> {
        let state = self.get_or_create_local_stream_state(stream_id)?;

        if state.local_write_closed {
            return Err(TransportError::StreamCloseWriteFailed {
                id: self.id,
                stream_id,
                reason: "local stream write side is already closed".into(),
            });
        }

        state.on_local_write_closed();
        state.pending_writes.push_back(PendingStreamWrite::fin());

        self.drain_send_queue(events)?;
        self.flush(socket)?;
        Ok(())
    }

    pub fn reset_stream(
        &mut self,
        stream_id: StreamId,
        events: &mut Vec<TransportEvent>,
    ) -> Result<(), TransportError> {
        let state = self.stream_states.get_mut(&stream_id.as_u64()).ok_or(
            TransportError::StreamNotFound {
                id: self.id,
                stream_id,
            },
        )?;

        self.conn
            .stream_shutdown(stream_id.as_u64(), quiche::Shutdown::Write, 0x00)
            .map_err(|e| TransportError::StreamResetFailed {
                id: self.id,
                stream_id,
                reason: format!("failed to shutdown stream write side: {e}"),
            })?;

        self.conn
            .stream_shutdown(stream_id.as_u64(), quiche::Shutdown::Read, 0x00)
            .map_err(|e| TransportError::StreamResetFailed {
                id: self.id,
                stream_id,
                reason: format!("failed to shutdown stream read side: {e}"),
            })?;

        state.local_write_closed = true;
        state.remote_write_closed = true;

        if !state.closed_notified {
            state.closed_notified = true;
            events.push(TransportEvent::StreamClosed {
                id: self.id,
                stream_id,
            });
        }

        Ok(())
    }

    pub fn close(&mut self, socket: &UdpSocket) -> Result<(), TransportError> {
        self.conn
            .close(true, 0x00, b"bye")
            .map_err(|e| TransportError::CloseFailed {
                id: self.id,
                reason: format!("close error: {e}"),
            })?;

        self.state = ConnectionState::Closing;
        let mut drain_events = Vec::new();
        self.drain_send_queue(&mut drain_events)?;
        self.flush(socket)?;
        Ok(())
    }

    pub fn poll_streams(
        &mut self,
        events: &mut Vec<TransportEvent>,
        socket: &UdpSocket,
    ) -> Result<(), TransportError> {
        if !self.conn.is_established() {
            return Ok(());
        }

        let mut buf = [0u8; 65535];

        for raw_stream_id in self.conn.readable() {
            let stream_id = StreamId::new(raw_stream_id);
            self.ensure_stream_discovered(stream_id, events);

            loop {
                match self.conn.stream_recv(raw_stream_id, &mut buf) {
                    Ok((read, fin)) => {
                        if read > 0 {
                            events.push(TransportEvent::StreamData {
                                id: self.id,
                                stream_id,
                                data: buf[..read].to_vec(),
                            });
                        }

                        if fin {
                            if let Some(state) = self.stream_states.get_mut(&raw_stream_id) {
                                state.on_remote_write_closed();
                            }

                            events.push(TransportEvent::StreamRemoteWriteClosed {
                                id: self.id,
                                stream_id,
                            });

                            self.note_stream_closed_if_finished(stream_id, events);
                            break;
                        }
                    }
                    Err(quiche::Error::Done) => break,
                    Err(e) => {
                        events.push(TransportEvent::Error {
                            id: self.id,
                            message: format!("stream_recv error on {stream_id}: {e}"),
                        });
                        break;
                    }
                }
            }
        }

        self.drain_send_queue(events)?;
        self.flush(socket)?;
        self.gc_closed_streams();
        Ok(())
    }

    fn flush(&mut self, socket: &UdpSocket) -> Result<(), TransportError> {
        let mut out = [0u8; SEND_BUF_SIZE];
        loop {
            let (written, send_info) = match self.conn.send(&mut out) {
                Ok(v) => v,
                Err(quiche::Error::Done) => break,
                Err(e) => {
                    return Err(TransportError::CloseFailed {
                        id: self.id,
                        reason: format!("quiche send error: {e}"),
                    });
                }
            };

            socket.send_to(&out[..written], send_info.to).map_err(|e| {
                TransportError::CloseFailed {
                    id: self.id,
                    reason: format!("udp send error: {e}"),
                }
            })?;
        }
        Ok(())
    }

    fn drain_send_queue(&mut self, events: &mut Vec<TransportEvent>) -> Result<(), TransportError> {
        let stream_ids: Vec<u64> = self.stream_states.keys().copied().collect();

        for raw_stream_id in stream_ids {
            loop {
                let pending = self
                    .stream_states
                    .get(&raw_stream_id)
                    .and_then(|state| state.pending_writes.front().cloned());

                let Some(pending) = pending else {
                    break;
                };

                let stream_id = StreamId::new(raw_stream_id);
                let payload = &pending.bytes[pending.offset..];
                let fin = pending.fin && payload.is_empty();

                let written = match self.conn.stream_send(raw_stream_id, payload, fin) {
                    Ok(written) => written,
                    Err(quiche::Error::Done) => break,
                    Err(e) => {
                        return Err(TransportError::StreamSendFailed {
                            id: self.id,
                            stream_id,
                            reason: format!("stream_send error: {e}"),
                        });
                    }
                };

                if let Some(state) = self.stream_states.get_mut(&raw_stream_id) {
                    if let Some(front) = state.pending_writes.front_mut() {
                        if front.fin && front.bytes.is_empty() {
                            state.pending_writes.pop_front();
                        } else {
                            if written == 0 {
                                break;
                            }

                            front.offset = front.offset.saturating_add(written);
                            if front.offset >= front.bytes.len() {
                                state.pending_writes.pop_front();
                            }
                        }
                    }
                }

                self.note_stream_closed_if_finished(stream_id, events);
            }
        }

        Ok(())
    }

    fn ensure_stream_discovered(&mut self, stream_id: StreamId, events: &mut Vec<TransportEvent>) {
        let is_remote = self.is_remote_initiated_stream(stream_id.as_u64());
        let state = self.stream_states.entry(stream_id.as_u64()).or_default();

        if is_remote && !state.incoming_notified {
            state.incoming_notified = true;
            events.push(TransportEvent::IncomingStream {
                id: self.id,
                stream_id,
            });
        }
    }

    fn note_stream_closed_if_finished(
        &mut self,
        stream_id: StreamId,
        events: &mut Vec<TransportEvent>,
    ) {
        if let Some(state) = self.stream_states.get_mut(&stream_id.as_u64()) {
            if state.is_fully_closed() && !state.closed_notified {
                state.closed_notified = true;
                events.push(TransportEvent::StreamClosed {
                    id: self.id,
                    stream_id,
                });
            }
        }
    }

    fn gc_closed_streams(&mut self) {
        let to_remove: Vec<u64> = self
            .stream_states
            .iter()
            .filter_map(|(stream_id, state)| {
                if state.closed_notified && state.pending_writes.is_empty() {
                    Some(*stream_id)
                } else {
                    None
                }
            })
            .collect();

        for stream_id in to_remove {
            self.stream_states.remove(&stream_id);
        }
    }

    fn get_or_create_local_stream_state(
        &mut self,
        stream_id: StreamId,
    ) -> Result<&mut StreamRuntimeState, TransportError> {
        if self.stream_states.contains_key(&stream_id.as_u64()) {
            return Ok(self
                .stream_states
                .get_mut(&stream_id.as_u64())
                .expect("stream state must exist"));
        }

        if !self.is_local_initiated_stream(stream_id.as_u64()) {
            return Err(TransportError::StreamNotFound {
                id: self.id,
                stream_id,
            });
        }

        Ok(self.stream_states.entry(stream_id.as_u64()).or_default())
    }

    fn is_local_initiated_stream(&self, stream_id: u64) -> bool {
        let local_initiator_bit = if self.conn.is_server() { 1 } else { 0 };
        (stream_id & 0x1) == local_initiator_bit
    }

    fn is_remote_initiated_stream(&self, stream_id: u64) -> bool {
        !self.is_local_initiated_stream(stream_id)
    }

    pub fn matches_peer(&self, addr: SocketAddr) -> bool {
        self.peer_addr == addr
    }
}
