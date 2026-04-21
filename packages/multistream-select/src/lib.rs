#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use alloc::collections::BTreeSet;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

pub const MULTISTREAM_PROTOCOL_ID: &str = "/multistream/1.0.0";
const NOT_AVAILABLE: &str = "na";
const MAX_LINE_LEN: usize = 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MultistreamOutput {
    OutboundData(Vec<u8>),
    Negotiated { protocol: String },
    NotAvailable,
    ProtocolError { reason: &'static str },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Role {
    Dialer,
    Listener,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum State {
    WaitingForRemoteHeader,
    WaitingForProtocolRequest,
    WaitingForProtocolResponse,
    Done,
}

pub struct MultistreamSelect {
    role: Role,
    state: State,
    desired_protocol: Option<String>,
    supported_protocols: BTreeSet<String>,
    recv_buf: Vec<u8>,
    started: bool,
}

impl MultistreamSelect {
    pub fn dialer(desired_protocol: impl Into<String>) -> Self {
        Self {
            role: Role::Dialer,
            state: State::WaitingForRemoteHeader,
            desired_protocol: Some(desired_protocol.into()),
            supported_protocols: BTreeSet::new(),
            recv_buf: Vec::new(),
            started: false,
        }
    }

    pub fn listener(protocols: impl IntoIterator<Item = String>) -> Self {
        Self {
            role: Role::Listener,
            state: State::WaitingForRemoteHeader,
            desired_protocol: None,
            supported_protocols: protocols.into_iter().collect(),
            recv_buf: Vec::new(),
            started: false,
        }
    }

    pub fn start(&mut self) -> Vec<MultistreamOutput> {
        if self.started {
            return Vec::new();
        }

        self.started = true;
        vec![MultistreamOutput::OutboundData(encode_line(
            MULTISTREAM_PROTOCOL_ID,
        ))]
    }

    pub fn receive(&mut self, bytes: &[u8]) -> Vec<MultistreamOutput> {
        if self.state == State::Done {
            return Vec::new();
        }

        self.recv_buf.extend_from_slice(bytes);
        let mut outputs = Vec::new();

        loop {
            if self.recv_buf.len() > MAX_LINE_LEN {
                outputs.extend(self.protocol_error("line exceeds max length"));
                break;
            }

            let Some(newline_idx) = self.recv_buf.iter().position(|b| *b == b'\n') else {
                break;
            };

            let mut line = self.recv_buf.drain(..=newline_idx).collect::<Vec<_>>();
            if line.last() == Some(&b'\n') {
                line.pop();
            }

            let parsed = match core::str::from_utf8(&line) {
                Ok(line) if !line.is_empty() => line,
                Ok(_) => {
                    outputs.extend(self.protocol_error("empty protocol line"));
                    break;
                }
                Err(_) => {
                    outputs.extend(self.protocol_error("non-utf8 protocol line"));
                    break;
                }
            };

            outputs.extend(self.on_line(parsed));
            if self.state == State::Done {
                break;
            }
        }

        outputs
    }

    fn on_line(&mut self, line: &str) -> Vec<MultistreamOutput> {
        match self.state {
            State::WaitingForRemoteHeader => {
                if line != MULTISTREAM_PROTOCOL_ID {
                    return self.protocol_error("unexpected multistream header");
                }

                match self.role {
                    Role::Dialer => {
                        let desired = self
                            .desired_protocol
                            .as_ref()
                            .expect("dialer must have desired protocol");
                        self.state = State::WaitingForProtocolResponse;
                        vec![MultistreamOutput::OutboundData(encode_line(desired))]
                    }
                    Role::Listener => {
                        self.state = State::WaitingForProtocolRequest;
                        Vec::new()
                    }
                }
            }
            State::WaitingForProtocolRequest => {
                if self.supported_protocols.contains(line) {
                    self.state = State::Done;
                    vec![
                        MultistreamOutput::OutboundData(encode_line(line)),
                        MultistreamOutput::Negotiated {
                            protocol: line.to_string(),
                        },
                    ]
                } else {
                    self.state = State::Done;
                    vec![
                        MultistreamOutput::OutboundData(encode_line(NOT_AVAILABLE)),
                        MultistreamOutput::NotAvailable,
                    ]
                }
            }
            State::WaitingForProtocolResponse => {
                let desired = self
                    .desired_protocol
                    .as_ref()
                    .expect("dialer must have desired protocol");

                if line == desired {
                    self.state = State::Done;
                    vec![MultistreamOutput::Negotiated {
                        protocol: desired.clone(),
                    }]
                } else if line == NOT_AVAILABLE {
                    self.state = State::Done;
                    vec![MultistreamOutput::NotAvailable]
                } else {
                    self.protocol_error("unexpected protocol response")
                }
            }
            State::Done => Vec::new(),
        }
    }

    fn protocol_error(&mut self, reason: &'static str) -> Vec<MultistreamOutput> {
        self.state = State::Done;
        vec![MultistreamOutput::ProtocolError { reason }]
    }
}

fn encode_line(line: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(line.len() + 1);
    out.extend_from_slice(line.as_bytes());
    out.push(b'\n');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const PING: &str = "/ipfs/ping/1.0.0";

    #[test]
    fn dialer_and_listener_negotiate_ping() {
        let mut dialer = MultistreamSelect::dialer(PING);
        let mut listener = MultistreamSelect::listener([PING.to_string()]);

        let listener_start = listener.start();
        let dialer_start = dialer.start();

        assert_eq!(
            listener_start,
            vec![MultistreamOutput::OutboundData(encode_line(
                MULTISTREAM_PROTOCOL_ID
            ))]
        );
        assert_eq!(
            dialer_start,
            vec![MultistreamOutput::OutboundData(encode_line(
                MULTISTREAM_PROTOCOL_ID
            ))]
        );

        let dialer_after_header = dialer.receive(&encode_line(MULTISTREAM_PROTOCOL_ID));
        assert_eq!(
            dialer_after_header,
            vec![MultistreamOutput::OutboundData(encode_line(PING))]
        );

        let listener_after_header = listener.receive(&encode_line(MULTISTREAM_PROTOCOL_ID));
        assert!(listener_after_header.is_empty());

        let listener_after_request = listener.receive(&encode_line(PING));
        assert_eq!(
            listener_after_request,
            vec![
                MultistreamOutput::OutboundData(encode_line(PING)),
                MultistreamOutput::Negotiated {
                    protocol: PING.to_string()
                }
            ]
        );

        let dialer_after_response = dialer.receive(&encode_line(PING));
        assert_eq!(
            dialer_after_response,
            vec![MultistreamOutput::Negotiated {
                protocol: PING.to_string()
            }]
        );
    }

    #[test]
    fn listener_rejects_unsupported_protocol() {
        let mut listener = MultistreamSelect::listener(["/ipfs/identify/1.0.0".to_string()]);
        let _ = listener.start();
        let _ = listener.receive(&encode_line(MULTISTREAM_PROTOCOL_ID));

        let out = listener.receive(&encode_line(PING));
        assert_eq!(
            out,
            vec![
                MultistreamOutput::OutboundData(encode_line("na")),
                MultistreamOutput::NotAvailable
            ]
        );
    }

    #[test]
    fn parses_chunked_input_lines() {
        let mut dialer = MultistreamSelect::dialer(PING);
        let _ = dialer.start();

        let mut out = dialer.receive(b"/multistream/1.");
        assert!(out.is_empty());

        out = dialer.receive(b"0.0\n");
        assert_eq!(
            out,
            vec![MultistreamOutput::OutboundData(encode_line(PING))]
        );
    }

    #[test]
    fn errors_on_unexpected_header() {
        let mut dialer = MultistreamSelect::dialer(PING);
        let _ = dialer.start();

        let out = dialer.receive(&encode_line("/wrong/1.0.0"));
        assert_eq!(
            out,
            vec![MultistreamOutput::ProtocolError {
                reason: "unexpected multistream header"
            }]
        );
    }
}
