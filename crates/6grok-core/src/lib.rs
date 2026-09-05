//! Vendor-neutral frame types and parser integration for 6grok.

use fivegrok_parser::{decode_agent_frame, DecodedPacket, DiagFrame};
use serde::{Deserialize, Serialize};

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
}
