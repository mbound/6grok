//! Clean-room Qualcomm DIAG transport/control primitives.
//!
//! This module implements public DIAG wire behavior independently. No GPL
//! implementation source is incorporated here; see `docs/PROTOCOL_REFERENCES.md`.

use crc::{Crc, CRC_16_IBM_SDLC};
use std::collections::BTreeMap;
use std::fmt;

const DIAG_CRC: Crc<u16> = Crc::<u16>::new(&CRC_16_IBM_SDLC);
pub const DIAG_LOG_F: u8 = 0x10;
pub const DIAG_LOG_CONFIG_F: u8 = 0x73;
pub const LOG_CONFIG_DISABLE_OP: u32 = 0;
pub const LOG_CONFIG_RETRIEVE_ID_RANGES_OP: u32 = 1;
pub const LOG_CONFIG_RETRIEVE_VALID_MASK_OP: u32 = 2;
pub const LOG_CONFIG_SET_MASK_OP: u32 = 3;
pub const LOG_CONFIG_GET_LOGMASK_OP: u32 = 4;

#[derive(Debug, Default)]
pub struct HdlcDecoder {
    frame: Vec<u8>,
    escaped: bool,
}

impl HdlcDecoder {
    pub fn push(&mut self, byte: u8) -> Option<Result<Vec<u8>, FrameError>> {
        match byte {
            0x7e => {
                self.escaped = false;
                if self.frame.is_empty() {
                    return None;
                }
                let raw = std::mem::take(&mut self.frame);
                Some(validate_diag_frame(raw))
            }
            0x7d => {
                self.escaped = true;
                None
            }
            value => {
                self.frame.push(if self.escaped { value ^ 0x20 } else { value });
                self.escaped = false;
                None
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FrameError {
    TooShort,
    BadCrc { expected: u16, actual: u16 },
}

impl fmt::Display for FrameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooShort => write!(f, "frame shorter than DIAG payload + CRC"),
            Self::BadCrc { expected, actual } => write!(
                f,
                "CRC mismatch (wire=0x{expected:04x}, calculated=0x{actual:04x})"
            ),
        }
    }
}

pub fn validate_diag_frame(mut raw: Vec<u8>) -> Result<Vec<u8>, FrameError> {
    if raw.len() < 3 {
        return Err(FrameError::TooShort);
    }
    let crc_pos = raw.len() - 2;
    let expected = u16::from_le_bytes([raw[crc_pos], raw[crc_pos + 1]]);
    let actual = DIAG_CRC.checksum(&raw[..crc_pos]);
    if expected != actual {
        return Err(FrameError::BadCrc { expected, actual });
    }
    raw.truncate(crc_pos);
    Ok(raw)
}

/// Encode a raw DIAG packet into the asynchronous HDLC representation used by
/// serial/USB diagnostic endpoints.
pub fn encode_hdlc(packet: &[u8]) -> Vec<u8> {
    let crc = DIAG_CRC.checksum(packet).to_le_bytes();
    let mut out = Vec::with_capacity(packet.len() + 8);
    for byte in packet.iter().copied().chain(crc) {
        match byte {
            0x7d | 0x7e => {
                out.push(0x7d);
                out.push(byte ^ 0x20);
            }
            other => out.push(other),
        }
    }
    out.push(0x7e);
    out
}

/// Build a DIAG_LOG_CONFIG_F request with no operation data.
pub fn log_config_request(operation: u32) -> Vec<u8> {
    let mut packet = Vec::with_capacity(8);
    packet.push(DIAG_LOG_CONFIG_F);
    packet.extend_from_slice(&[0, 0, 0]);
    packet.extend_from_slice(&operation.to_le_bytes());
    packet
}

pub fn retrieve_id_ranges_request() -> Vec<u8> {
    log_config_request(LOG_CONFIG_RETRIEVE_ID_RANGES_OP)
}

pub fn disable_logging_request() -> Vec<u8> {
    log_config_request(LOG_CONFIG_DISABLE_OP)
}

/// A 16-bit Qualcomm log code is `equipment_id:4 | item_id:12`.
pub const fn split_log_code(code: u16) -> (u8, u16) {
    (((code >> 12) & 0x0f) as u8, code & 0x0fff)
}

pub fn group_log_codes(codes: &[u16]) -> BTreeMap<u8, Vec<u16>> {
    let mut grouped: BTreeMap<u8, Vec<u16>> = BTreeMap::new();
    for &code in codes {
        let (equip, item) = split_log_code(code);
        grouped.entry(equip).or_default().push(item);
    }
    for items in grouped.values_mut() {
        items.sort_unstable();
        items.dedup();
    }
    grouped
}

/// Construct a SET_MASK request for one equipment ID.
///
/// `last_item` is inclusive, therefore the bitmask contains
/// `floor(last_item / 8) + 1` bytes.
pub fn set_mask_request(equip_id: u8, last_item: u32, items: &[u16]) -> Result<Vec<u8>, ControlError> {
    if equip_id > 0x0f {
        return Err(ControlError::InvalidEquipmentId(equip_id));
    }
    let mask_len = (last_item as usize / 8) + 1;
    let mut mask = vec![0_u8; mask_len];
    for &item in items {
        if u32::from(item) > last_item {
            return Err(ControlError::ItemOutOfRange {
                equip_id,
                item,
                last_item,
            });
        }
        let index = usize::from(item) / 8;
        let bit = item % 8;
        mask[index] |= 1 << bit;
    }

    let mut packet = Vec::with_capacity(16 + mask.len());
    packet.push(DIAG_LOG_CONFIG_F);
    packet.extend_from_slice(&[0, 0, 0]);
    packet.extend_from_slice(&LOG_CONFIG_SET_MASK_OP.to_le_bytes());
    packet.extend_from_slice(&u32::from(equip_id).to_le_bytes());
    packet.extend_from_slice(&last_item.to_le_bytes());
    packet.extend_from_slice(&mask);
    Ok(packet)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LogConfigHeader {
    pub operation: u32,
    pub status: u32,
}

pub fn parse_log_config_header(packet: &[u8]) -> Result<(LogConfigHeader, &[u8]), ControlError> {
    if packet.len() < 12 || packet[0] != DIAG_LOG_CONFIG_F {
        return Err(ControlError::NotLogConfigResponse);
    }
    let operation = read_u32(packet, 4)?;
    let status = read_u32(packet, 8)?;
    Ok((LogConfigHeader { operation, status }, &packet[12..]))
}

pub fn parse_id_ranges_response(packet: &[u8]) -> Result<[u32; 16], ControlError> {
    let (header, data) = parse_log_config_header(packet)?;
    if header.operation != LOG_CONFIG_RETRIEVE_ID_RANGES_OP {
        return Err(ControlError::UnexpectedOperation(header.operation));
    }
    if header.status != 0 {
        return Err(ControlError::RemoteStatus(header.status));
    }
    if data.len() < 16 * 4 {
        return Err(ControlError::TruncatedResponse);
    }
    let mut ranges = [0_u32; 16];
    for (idx, slot) in ranges.iter_mut().enumerate() {
        *slot = read_u32(data, idx * 4)?;
    }
    Ok(ranges)
}

fn read_u32(data: &[u8], offset: usize) -> Result<u32, ControlError> {
    let bytes: [u8; 4] = data
        .get(offset..offset + 4)
        .ok_or(ControlError::TruncatedResponse)?
        .try_into()
        .map_err(|_| ControlError::TruncatedResponse)?;
    Ok(u32::from_le_bytes(bytes))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ControlError {
    InvalidEquipmentId(u8),
    ItemOutOfRange { equip_id: u8, item: u16, last_item: u32 },
    NotLogConfigResponse,
    UnexpectedOperation(u32),
    RemoteStatus(u32),
    TruncatedResponse,
}

impl fmt::Display for ControlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidEquipmentId(id) => write!(f, "invalid equipment ID {id}"),
            Self::ItemOutOfRange { equip_id, item, last_item } => write!(
                f,
                "log item 0x{item:03x} exceeds equipment {equip_id} last_item 0x{last_item:03x}"
            ),
            Self::NotLogConfigResponse => write!(f, "packet is not a DIAG_LOG_CONFIG_F response"),
            Self::UnexpectedOperation(op) => write!(f, "unexpected log-config operation {op}"),
            Self::RemoteStatus(status) => write!(f, "modem returned log-config status {status}"),
            Self::TruncatedResponse => write!(f, "truncated log-config response"),
        }
    }
}

impl std::error::Error for ControlError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hdlc_roundtrip_handles_escaping() {
        let packet = [0x73, 0x7d, 0x7e, 0x00, 0x55];
        let encoded = encode_hdlc(&packet);
        let mut decoder = HdlcDecoder::default();
        let mut decoded = None;
        for byte in encoded {
            if let Some(result) = decoder.push(byte) {
                decoded = Some(result.unwrap());
            }
        }
        assert_eq!(decoded.unwrap(), packet);
    }

    #[test]
    fn splits_and_groups_log_codes() {
        assert_eq!(split_log_code(0xb0c0), (0x0b, 0x00c0));
        let grouped = group_log_codes(&[0xb0c0, 0xb17f, 0xb0c0, 0x1a00]);
        assert_eq!(grouped.get(&0x0b).unwrap(), &[0x00c0, 0x017f]);
        assert_eq!(grouped.get(&0x01).unwrap(), &[0x0a00]);
    }

    #[test]
    fn builds_set_mask_with_inclusive_last_item() {
        let packet = set_mask_request(0x0b, 8, &[0, 8]).unwrap();
        assert_eq!(&packet[..16], &[0x73, 0, 0, 0, 3, 0, 0, 0, 0x0b, 0, 0, 0, 8, 0, 0, 0]);
        assert_eq!(&packet[16..], &[0x01, 0x01]);
    }

    #[test]
    fn parses_range_response() {
        let mut packet = vec![0x73, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0];
        for value in 0_u32..16 {
            packet.extend_from_slice(&(0x100 + value).to_le_bytes());
        }
        let ranges = parse_id_ranges_response(&packet).unwrap();
        assert_eq!(ranges[0], 0x100);
        assert_eq!(ranges[15], 0x10f);
    }
}
