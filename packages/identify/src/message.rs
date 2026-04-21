//! Protobuf encoding and decoding for the Identify message.
//!
//! Implements the following protobuf schema (proto2):
//!
//! ```text
//! message Identify {
//!   optional bytes  publicKey       = 1;
//!   repeated bytes  listenAddrs     = 2;
//!   repeated string protocols       = 3;
//!   optional bytes  observedAddr    = 4;
//!   optional string protocolVersion = 5;
//!   optional string agentVersion    = 6;
//! }
//! ```
//!
//! All fields use wire type LEN (2). The encoder writes fields in field-number
//! order. The decoder accepts fields in any order and ignores unknown fields,
//! following protobuf convention.

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;

use minip2p_core::{read_uvarint, write_uvarint, VarintError};
use thiserror::Error;

// Protobuf tag bytes: (field_number << 3) | wire_type
// All fields are wire type LEN (2).
const TAG_PUBLIC_KEY: u8 = (1 << 3) | 2;       // 0x0A
const TAG_LISTEN_ADDRS: u8 = (2 << 3) | 2;     // 0x12
const TAG_PROTOCOLS: u8 = (3 << 3) | 2;        // 0x1A
const TAG_OBSERVED_ADDR: u8 = (4 << 3) | 2;    // 0x22
const TAG_PROTOCOL_VERSION: u8 = (5 << 3) | 2; // 0x2A
const TAG_AGENT_VERSION: u8 = (6 << 3) | 2;    // 0x32

/// The decoded identify message exchanged between peers.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct IdentifyMessage {
    /// The peer's public key (protobuf-encoded `PublicKey` message).
    pub public_key: Option<Vec<u8>>,
    /// Addresses the peer is listening on (multiaddr binary encoding).
    pub listen_addrs: Vec<Vec<u8>>,
    /// Protocol IDs the peer supports.
    pub protocols: Vec<String>,
    /// The address the peer observes us connecting from (multiaddr binary).
    pub observed_addr: Option<Vec<u8>>,
    /// Protocol version string (e.g. `"ipfs/0.1.0"`).
    pub protocol_version: Option<String>,
    /// Agent version string (e.g. `"go-libp2p/0.36.0"`).
    pub agent_version: Option<String>,
}

/// Errors that can occur during identify message decoding.
#[derive(Clone, Debug, Eq, PartialEq, Error)]
pub enum IdentifyMessageError {
    /// A varint could not be decoded.
    #[error("varint error: {0}")]
    Varint(#[from] VarintError),
    /// A length-delimited field extends beyond the message boundary.
    #[error("field at offset {offset} has length {length} but only {remaining} bytes remain")]
    FieldOverflow {
        offset: usize,
        length: usize,
        remaining: usize,
    },
    /// A string field contains invalid UTF-8.
    #[error("invalid UTF-8 in field with tag {tag:#04x}")]
    InvalidUtf8 { tag: u8 },
}

impl IdentifyMessage {
    /// Encodes the message to protobuf binary format.
    ///
    /// Fields are written in field-number order.
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();

        // Field 1: publicKey (optional bytes)
        if let Some(ref key) = self.public_key {
            encode_bytes_field(&mut out, TAG_PUBLIC_KEY, key);
        }

        // Field 2: listenAddrs (repeated bytes)
        for addr in &self.listen_addrs {
            encode_bytes_field(&mut out, TAG_LISTEN_ADDRS, addr);
        }

        // Field 3: protocols (repeated string)
        for proto in &self.protocols {
            encode_bytes_field(&mut out, TAG_PROTOCOLS, proto.as_bytes());
        }

        // Field 4: observedAddr (optional bytes)
        if let Some(ref addr) = self.observed_addr {
            encode_bytes_field(&mut out, TAG_OBSERVED_ADDR, addr);
        }

        // Field 5: protocolVersion (optional string)
        if let Some(ref ver) = self.protocol_version {
            encode_bytes_field(&mut out, TAG_PROTOCOL_VERSION, ver.as_bytes());
        }

        // Field 6: agentVersion (optional string)
        if let Some(ref ver) = self.agent_version {
            encode_bytes_field(&mut out, TAG_AGENT_VERSION, ver.as_bytes());
        }

        out
    }

    /// Decodes a message from protobuf binary format.
    ///
    /// Accepts fields in any order and ignores unknown fields.
    pub fn decode(input: &[u8]) -> Result<Self, IdentifyMessageError> {
        let mut msg = IdentifyMessage::default();
        let mut idx = 0;

        while idx < input.len() {
            // Read field tag (a varint, but usually 1 byte for field numbers < 16).
            let (tag_value, tag_used) = read_uvarint(&input[idx..])?;
            idx += tag_used;

            let tag = tag_value as u8;
            let wire_type = tag & 0x07;

            match wire_type {
                // Wire type 2 (LEN): length-delimited field.
                2 => {
                    let (length, len_used) = read_uvarint(&input[idx..])?;
                    idx += len_used;

                    let length = length as usize;
                    let remaining = input.len().saturating_sub(idx);
                    if length > remaining {
                        return Err(IdentifyMessageError::FieldOverflow {
                            offset: idx,
                            length,
                            remaining,
                        });
                    }

                    let value = &input[idx..idx + length];
                    idx += length;

                    match tag {
                        TAG_PUBLIC_KEY => {
                            msg.public_key = Some(value.to_vec());
                        }
                        TAG_LISTEN_ADDRS => {
                            msg.listen_addrs.push(value.to_vec());
                        }
                        TAG_PROTOCOLS => {
                            let s = core::str::from_utf8(value)
                                .map_err(|_| IdentifyMessageError::InvalidUtf8 { tag })?;
                            msg.protocols.push(String::from(s));
                        }
                        TAG_OBSERVED_ADDR => {
                            msg.observed_addr = Some(value.to_vec());
                        }
                        TAG_PROTOCOL_VERSION => {
                            let s = core::str::from_utf8(value)
                                .map_err(|_| IdentifyMessageError::InvalidUtf8 { tag })?;
                            msg.protocol_version = Some(String::from(s));
                        }
                        TAG_AGENT_VERSION => {
                            let s = core::str::from_utf8(value)
                                .map_err(|_| IdentifyMessageError::InvalidUtf8 { tag })?;
                            msg.agent_version = Some(String::from(s));
                        }
                        _ => {
                            // Unknown field — skip it (already consumed the bytes above).
                        }
                    }
                }
                // Wire type 0 (VARINT): skip the varint value.
                0 => {
                    let (_, used) = read_uvarint(&input[idx..])?;
                    idx += used;
                }
                // Wire type 5 (I32): skip 4 bytes.
                5 => {
                    if idx + 4 > input.len() {
                        return Err(IdentifyMessageError::FieldOverflow {
                            offset: idx,
                            length: 4,
                            remaining: input.len().saturating_sub(idx),
                        });
                    }
                    idx += 4;
                }
                // Wire type 1 (I64): skip 8 bytes.
                1 => {
                    if idx + 8 > input.len() {
                        return Err(IdentifyMessageError::FieldOverflow {
                            offset: idx,
                            length: 8,
                            remaining: input.len().saturating_sub(idx),
                        });
                    }
                    idx += 8;
                }
                // Unknown wire type — cannot skip safely.
                _ => {
                    break;
                }
            }
        }

        Ok(msg)
    }
}

/// Writes a length-delimited protobuf field: tag + varint length + bytes.
fn encode_bytes_field(out: &mut Vec<u8>, tag: u8, data: &[u8]) {
    out.push(tag);
    write_uvarint(data.len() as u64, out);
    out.extend_from_slice(data);
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_empty_message() {
        let msg = IdentifyMessage::default();
        let encoded = msg.encode();
        assert!(encoded.is_empty());
        let decoded = IdentifyMessage::decode(&encoded).unwrap();
        assert_eq!(msg, decoded);
    }

    #[test]
    fn round_trip_full_message() {
        let msg = IdentifyMessage {
            public_key: Some(vec![0x08, 0x01, 0x12, 0x20, 0xAA]),
            listen_addrs: vec![vec![0x04, 127, 0, 0, 1], vec![0x04, 10, 0, 0, 1]],
            protocols: vec![
                String::from("/ipfs/ping/1.0.0"),
                String::from("/ipfs/id/1.0.0"),
            ],
            observed_addr: Some(vec![0x04, 192, 168, 1, 100]),
            protocol_version: Some(String::from("ipfs/0.1.0")),
            agent_version: Some(String::from("minip2p/0.1.0")),
        };

        let encoded = msg.encode();
        let decoded = IdentifyMessage::decode(&encoded).unwrap();
        assert_eq!(msg, decoded);
    }

    #[test]
    fn round_trip_optional_fields_absent() {
        let msg = IdentifyMessage {
            public_key: None,
            listen_addrs: vec![],
            protocols: vec![String::from("/ipfs/ping/1.0.0")],
            observed_addr: None,
            protocol_version: None,
            agent_version: Some(String::from("test/0.1.0")),
        };

        let encoded = msg.encode();
        let decoded = IdentifyMessage::decode(&encoded).unwrap();
        assert_eq!(msg, decoded);
    }

    #[test]
    fn decode_ignores_unknown_fields() {
        // Build a message with a known field, then an unknown field (tag 0x3A = field 7, LEN),
        // then another known field.
        let mut data = Vec::new();

        // Field 6 (agentVersion): "test"
        data.push(TAG_AGENT_VERSION);
        data.push(4); // length
        data.extend_from_slice(b"test");

        // Unknown field 7, LEN: "unknown"
        data.push((7 << 3) | 2);
        data.push(7);
        data.extend_from_slice(b"unknown");

        // Field 5 (protocolVersion): "v1"
        data.push(TAG_PROTOCOL_VERSION);
        data.push(2);
        data.extend_from_slice(b"v1");

        let decoded = IdentifyMessage::decode(&data).unwrap();
        assert_eq!(decoded.agent_version.as_deref(), Some("test"));
        assert_eq!(decoded.protocol_version.as_deref(), Some("v1"));
    }

    #[test]
    fn decode_rejects_truncated_field() {
        let mut data = Vec::new();
        data.push(TAG_PUBLIC_KEY);
        data.push(10); // claims 10 bytes
        data.extend_from_slice(&[0u8; 5]); // only 5 bytes

        let err = IdentifyMessage::decode(&data).unwrap_err();
        assert!(matches!(err, IdentifyMessageError::FieldOverflow { .. }));
    }

    #[test]
    fn decode_rejects_invalid_utf8_in_string_field() {
        let mut data = Vec::new();
        data.push(TAG_AGENT_VERSION);
        data.push(3);
        data.extend_from_slice(&[0xFF, 0xFE, 0xFD]); // invalid UTF-8

        let err = IdentifyMessage::decode(&data).unwrap_err();
        assert!(matches!(err, IdentifyMessageError::InvalidUtf8 { .. }));
    }

    #[test]
    fn decode_skips_varint_unknown_fields() {
        let mut data = Vec::new();

        // Unknown field 10, VARINT: value 42
        data.push((10 << 3) | 0); // tag: field 10, wire type 0
        data.push(42); // varint value

        // Known field: agentVersion
        data.push(TAG_AGENT_VERSION);
        data.push(2);
        data.extend_from_slice(b"ok");

        let decoded = IdentifyMessage::decode(&data).unwrap();
        assert_eq!(decoded.agent_version.as_deref(), Some("ok"));
    }

    #[test]
    fn encode_field_order_matches_spec() {
        let msg = IdentifyMessage {
            public_key: Some(vec![0x01]),
            listen_addrs: vec![vec![0x02]],
            protocols: vec![String::from("p")],
            observed_addr: Some(vec![0x03]),
            protocol_version: Some(String::from("v")),
            agent_version: Some(String::from("a")),
        };

        let encoded = msg.encode();

        // Verify fields appear in order: 1, 2, 3, 4, 5, 6
        let tags: Vec<u8> = extract_field_tags(&encoded);
        assert_eq!(
            tags,
            vec![
                TAG_PUBLIC_KEY,
                TAG_LISTEN_ADDRS,
                TAG_PROTOCOLS,
                TAG_OBSERVED_ADDR,
                TAG_PROTOCOL_VERSION,
                TAG_AGENT_VERSION,
            ]
        );
    }

    /// Helper: extracts the tag bytes from a protobuf-encoded message.
    fn extract_field_tags(data: &[u8]) -> Vec<u8> {
        let mut tags = Vec::new();
        let mut idx = 0;
        while idx < data.len() {
            let tag = data[idx];
            tags.push(tag);
            idx += 1;

            let wire_type = tag & 0x07;
            match wire_type {
                2 => {
                    let (len, used) = read_uvarint(&data[idx..]).unwrap();
                    idx += used + len as usize;
                }
                0 => {
                    let (_, used) = read_uvarint(&data[idx..]).unwrap();
                    idx += used;
                }
                _ => break,
            }
        }
        tags
    }
}
