//! Vendor-neutral frame types, parser integration and agent wire protocol for 6grok.

use fivegrok_parser::{decode_agent_frame, DecodedPacket, DiagFrame};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Baseband/diagnostic producer associated with a normalized frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Vendor {
    Qualcomm,
    Mediatek,
    Samsung,
}

impl Vendor {
    /// Vendor IDs used by the surviving fivegrok-parser wire contract.
    pub const fn parser_id(self) -> u8 {
        match self {
            Self::Qualcomm => 0,
            Self::Mediatek => 1,
            Self::Samsung => 2,
        }
    }

    pub const fn from_parser_id(id: u8) -> Option<Self> {
        match id {
            0 => Some(Self::Qualcomm),
            1 => Some(Self::Mediatek),
            2 => Some(Self::Samsung),
            _ => None,
        }
    }
}

/// Normalized acquisition frame handed from a 6grok transport to the parser.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureFrame {
    pub sequence: u64,
    /// Unix epoch milliseconds.
    pub timestamp_wall: i64,
    /// Monotonic milliseconds from the acquisition process start.
    pub timestamp_mono: u64,
    pub vendor: Vendor,
    pub log_code: u16,
    /// Parser-compatible payload: `log_code_le || raw_vendor_packet`.
    pub payload: Vec<u8>,
}

impl CaptureFrame {
    pub fn decode(&self) -> DecodedPacket {
        let frame = DiagFrame {
            sequence: self.sequence,
            timestamp_wall: self.timestamp_wall,
            timestamp_mono: self.timestamp_mono,
            log_code: self.log_code,
            payload: self.payload.clone(),
            vendor: self.vendor.parser_id(),
        };
        decode_agent_frame(&frame)
    }

    pub fn decode_json(&self) -> serde_json::Result<serde_json::Value> {
        serde_json::to_value(self.decode())
    }
}

/// Encode a frame using the MessagePack tuple shape documented for the original
/// 5grok agent: `["Frame", [seq, ts_wall, ts_mono, log_code, payload, vendor]]`.
///
/// TCP users add an outer 32-bit length prefix; the MessagePack payload itself
/// remains transport-neutral and can also be carried over WebSocket or files.
pub fn encode_wire_frame(frame: &CaptureFrame) -> Result<Vec<u8>, WireError> {
    let message = (
        "Frame",
        (
            frame.sequence,
            frame.timestamp_wall,
            frame.timestamp_mono,
            frame.log_code,
            &frame.payload,
            frame.vendor.parser_id(),
        ),
    );
    rmp_serde::to_vec(&message).map_err(WireError::Encode)
}

pub fn decode_wire_frame(data: &[u8]) -> Result<CaptureFrame, WireError> {
    type FrameTuple = (u64, i64, u64, u16, Vec<u8>, u8);
    let (kind, frame): (String, FrameTuple) =
        rmp_serde::from_slice(data).map_err(WireError::Decode)?;
    if kind != "Frame" {
        return Err(WireError::UnknownMessage(kind));
    }
    let vendor = Vendor::from_parser_id(frame.5).ok_or(WireError::UnknownVendor(frame.5))?;
    Ok(CaptureFrame {
        sequence: frame.0,
        timestamp_wall: frame.1,
        timestamp_mono: frame.2,
        log_code: frame.3,
        payload: frame.4,
        vendor,
    })
}

#[derive(Debug)]
pub enum WireError {
    Encode(rmp_serde::encode::Error),
    Decode(rmp_serde::decode::Error),
    UnknownMessage(String),
    UnknownVendor(u8),
}

impl fmt::Display for WireError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Encode(err) => write!(f, "MessagePack encode failed: {err}"),
            Self::Decode(err) => write!(f, "MessagePack decode failed: {err}"),
            Self::UnknownMessage(kind) => write!(f, "unknown agent message type {kind:?}"),
            Self::UnknownVendor(id) => write!(f, "unknown vendor ID {id}"),
        }
    }
}

impl std::error::Error for WireError {}

/// Construct the payload representation expected by `fivegrok-parser`.
pub fn parser_payload(log_code: u16, raw_packet: &[u8]) -> Vec<u8> {
    let mut payload = Vec::with_capacity(raw_packet.len() + 2);
    payload.extend_from_slice(&log_code.to_le_bytes());
    payload.extend_from_slice(raw_packet);
    payload
}

/// Extract the log code from a Qualcomm DIAG `LOG_F` packet.
///
/// Layout used by the parser is:
/// `cmd(1), more(1), outer_len(2), entry_len(2), log_code(2), timestamp(8), ...`.
pub fn qualcomm_log_code(packet: &[u8]) -> Option<u16> {
    const DIAG_LOG_F: u8 = 0x10;
    if packet.len() < 8 || packet[0] != DIAG_LOG_F {
        return None;
    }
    Some(u16::from_le_bytes([packet[6], packet[7]]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_qualcomm_log_code() {
        let packet = [0x10, 0, 0, 0, 0, 0, 0x0b, 0x1d];
        assert_eq!(qualcomm_log_code(&packet), Some(0x1d0b));
    }

    #[test]
    fn rejects_non_log_packets() {
        let packet = [0x73, 0, 0, 0, 0, 0, 0x0b, 0x1d];
        assert_eq!(qualcomm_log_code(&packet), None);
    }

    #[test]
    fn messagepack_wire_roundtrip() {
        let frame = CaptureFrame {
            sequence: 42,
            timestamp_wall: 1000,
            timestamp_mono: 55,
            vendor: Vendor::Samsung,
            log_code: 0x2101,
            payload: vec![1, 2, 3, 4],
        };
        let encoded = encode_wire_frame(&frame).unwrap();
        let decoded = decode_wire_frame(&encoded).unwrap();
        assert_eq!(decoded.sequence, frame.sequence);
        assert_eq!(decoded.timestamp_wall, frame.timestamp_wall);
        assert_eq!(decoded.timestamp_mono, frame.timestamp_mono);
        assert_eq!(decoded.vendor, frame.vendor);
        assert_eq!(decoded.log_code, frame.log_code);
        assert_eq!(decoded.payload, frame.payload);
    }
}
