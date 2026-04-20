use std::collections::{HashMap, VecDeque};
use std::net::{SocketAddr, UdpSocket};

use minip2p_core::PeerId;
use minip2p_transport::{
    ConnectionEndpoint, ConnectionId, ConnectionState, TransportError, TransportEvent,
};

const SEND_BUF_SIZE: usize = 1350;
const FRAME_LEN_BYTES: usize = 4;
const MAX_FRAME_SIZE: usize = 1024 * 1024;

pub struct QuicConnection {
    id: ConnectionId,
    conn: quiche::Connection,
    peer_addr: SocketAddr,
    endpoint: ConnectionEndpoint,
    state: ConnectionState,
    pending_stream_frames: VecDeque<(Vec<u8>, usize)>,
    stream_recv_buffers: HashMap<u64, Vec<u8>>,
}

impl QuicConnection {
    pub fn new(
        id: ConnectionId,
        conn: quiche::Connection,
        peer_addr: SocketAddr,
        endpoint: ConnectionEndpoint,
    ) -> Self {
        Self {
            id,
            conn,
            peer_addr,
            endpoint,
            state: ConnectionState::Connecting,
            pending_stream_frames: VecDeque::new(),
            stream_recv_buffers: HashMap::new(),
        }
    }

    pub fn source_cid_bytes(&self) -> Vec<u8> {
        self.conn.source_id().as_ref().to_vec()
    }

    pub fn endpoint(&self) -> &ConnectionEndpoint {
        &self.endpoint
    }

    pub fn state(&self) -> ConnectionState {
        self.state
    }

    pub fn is_connected(&self) -> bool {
        self.state == ConnectionState::Connected
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
            self.drain_send_queue()?;
            self.flush(socket)?;

            events.push(TransportEvent::Connected {
                id: self.id,
                endpoint: self.endpoint.clone(),
            });
        } else {
            self.drain_send_queue()?;
            self.flush(socket)?;
        }

        Ok(())
    }

    pub fn send_data(&mut self, data: &[u8], socket: &UdpSocket) -> Result<(), TransportError> {
        if data.len() > MAX_FRAME_SIZE {
            return Err(TransportError::SendFailed {
                id: self.id,
                reason: format!(
                    "message size {} exceeds max frame size {MAX_FRAME_SIZE}",
                    data.len()
                ),
            });
        }

        let frame_len = u32::try_from(data.len()).map_err(|_| TransportError::SendFailed {
            id: self.id,
            reason: format!("message size {} exceeds u32 framing limit", data.len()),
        })?;

        let mut frame = Vec::with_capacity(FRAME_LEN_BYTES + data.len());
        frame.extend_from_slice(&frame_len.to_be_bytes());
        frame.extend_from_slice(data);

        self.pending_stream_frames.push_back((frame, 0));
        self.drain_send_queue()?;

        self.flush(socket)?;
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
        self.drain_send_queue()?;
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

        for stream_id in self.conn.readable() {
            loop {
                match self.conn.stream_recv(stream_id, &mut buf) {
                    Ok((read, fin)) => {
                        if read > 0 {
                            self.process_stream_chunk(stream_id, &buf[..read], events, socket)?;
                        }

                        if fin {
                            self.stream_recv_buffers.remove(&stream_id);
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

        self.flush(socket)?;
        Ok(())
    }

    fn flush(&mut self, socket: &UdpSocket) -> Result<(), TransportError> {
        let mut out = [0u8; SEND_BUF_SIZE];
        loop {
            let (written, send_info) = match self.conn.send(&mut out) {
                Ok(v) => v,
                Err(quiche::Error::Done) => break,
                Err(e) => {
                    return Err(TransportError::SendFailed {
                        id: self.id,
                        reason: format!("quiche send error: {e}"),
                    });
                }
            };

            socket.send_to(&out[..written], send_info.to).map_err(|e| {
                TransportError::SendFailed {
                    id: self.id,
                    reason: format!("udp send error: {e}"),
                }
            })?;
        }
        Ok(())
    }

    fn drain_send_queue(&mut self) -> Result<(), TransportError> {
        let stream_id = if self.conn.is_server() { 1 } else { 0 };

        while let Some((frame, mut sent)) = self.pending_stream_frames.pop_front() {
            while sent < frame.len() {
                match self.conn.stream_send(stream_id, &frame[sent..], false) {
                    Ok(written) => {
                        if written == 0 {
                            self.pending_stream_frames.push_front((frame, sent));
                            return Ok(());
                        }
                        sent += written;
                    }
                    Err(quiche::Error::Done) => {
                        self.pending_stream_frames.push_front((frame, sent));
                        return Ok(());
                    }
                    Err(e) => {
                        return Err(TransportError::SendFailed {
                            id: self.id,
                            reason: format!("stream_send error: {e}"),
                        });
                    }
                }
            }
        }

        Ok(())
    }

    fn process_stream_chunk(
        &mut self,
        stream_id: u64,
        chunk: &[u8],
        events: &mut Vec<TransportEvent>,
        socket: &UdpSocket,
    ) -> Result<(), TransportError> {
        let mut close_due_to_frame_size = false;

        {
            let buffer = self.stream_recv_buffers.entry(stream_id).or_default();
            buffer.extend_from_slice(chunk);

            loop {
                if buffer.len() < FRAME_LEN_BYTES {
                    break;
                }

                let frame_len =
                    u32::from_be_bytes([buffer[0], buffer[1], buffer[2], buffer[3]]) as usize;

                if frame_len > MAX_FRAME_SIZE {
                    events.push(TransportEvent::Error {
                        id: self.id,
                        message: format!(
                            "received frame of {frame_len} bytes exceeding max {MAX_FRAME_SIZE}"
                        ),
                    });

                    buffer.clear();
                    close_due_to_frame_size = true;
                    break;
                }

                let total_len = FRAME_LEN_BYTES + frame_len;
                if buffer.len() < total_len {
                    break;
                }

                events.push(TransportEvent::Received {
                    id: self.id,
                    data: buffer[FRAME_LEN_BYTES..total_len].to_vec(),
                });
                buffer.drain(..total_len);
            }
        }

        if close_due_to_frame_size {
            self.close(socket)?;
        }

        Ok(())
    }

    pub fn matches_peer(&self, addr: SocketAddr) -> bool {
        self.peer_addr == addr
    }
}
