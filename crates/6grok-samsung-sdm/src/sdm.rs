// SPDX-FileCopyrightText: 2026 mbound
// SPDX-License-Identifier: GPL-2.0-or-later
//
// Samsung SDM framing/control adapted and translated from fgsect/scat:
//   repository: https://github.com/fgsect/scat
//   commit: 361ff551a4fbb30789c46750c00586682a7a9b26
//   paths:
//     src/scat/parsers/samsung/sdmcmd.py
//     src/scat/parsers/samsung/samsungparser.py
// Modified/translated to Rust for 6grok on 2026-09-05.

use clap::ValueEnum;
use std::fmt;

pub const SDM_START: u8 = 0x7f;
pub const SDM_END: u8 = 0x7e;
pub const SDM_HEADER_LEN: usize = 14;
pub const SDM_DIRECTION_DM: u8 = 0xa0;
pub const DEFAULT_START_MAGIC: u32 = 0x4141_4141;
pub const MAX_SDM_PACKET: usize = 8 * 1024 * 1024;

pub const GROUP_CONTROL: u8 = 0x00;
pub const GROUP_COMMON: u8 = 0x01;
pub const GROUP_LTE: u8 = 0x02;
pub const GROUP_EDGE: u8 = 0x03;
pub const GROUP_HSPA: u8 = 0x04;
pub const GROUP_TRACE: u8 = 0x05;
pub const GROUP_IP: u8 = 0x07;

pub const CONTROL_START: u8 = 0x00;
pub const CONTROL_STOP: u8 = 0x02;
pub const CHANGE_UPDATE_PERIOD_REQUEST: u8 = 0x06;
pub const COMMON_ITEM_SELECT_REQUEST: u8 = 0x10;
pub const LTE_ITEM_SELECT_REQUEST: u8 = 0x20;
pub const EDGE_ITEM_SELECT_REQUEST: u8 = 0x30;
pub const HSPA_ITEM_SELECT_REQUEST: u8 = 0x40;
pub const CDMA_ITEM_SELECT_REQUEST: u8 = 0x44;

// Common-data item IDs from SCAT sdmcmd.py.
pub const COMMON_BASIC_INFO: u8 = 0x00;
pub const COMMON_CELL_INFO: u8 = 0x01;
pub const COMMON_DATA_INFO: u8 = 0x02;
pub const COMMON_SIGNALING_INFO: u8 = 0x03;
pub const COMMON_MULTI_SIGNALING_INFO: u8 = 0x06;
pub const COMMON_NR_RRC_SIGNALING_INFO: u8 = 0x08;
pub const COMMON_NR_NAS_SIGNALING_INFO: u8 = 0x09;

// LTE-data item IDs from SCAT sdmcmd.py.
pub const LTE_PHY_STATUS: u8 = 0x00;
pub const LTE_PHY_CELL_SEARCH_MEAS: u8 = 0x01;
pub const LTE_PHY_NCELL_INFO: u8 = 0x02;
pub const LTE_L1_RF: u8 = 0x10;
pub const LTE_RRC_SERVING_CELL: u8 = 0x50;
pub const LTE_RRC_STATUS: u8 = 0x51;
pub const LTE_RRC_OTA_PACKET: u8 = 0x52;
pub const LTE_NAS_EMM_MESSAGE: u8 = 0x5a;
pub const LTE_NAS_ESM_MESSAGE: u8 = 0x5f;

/// Native Shannon SDM frames occupy a separate 6grok namespace rather than
/// masquerading as the surviving parser's synthetic 0x20xx..0x23xx MIPC IDs.
pub const SDM_SYNTHETIC_BASE: u16 = 0x2400;

#[derive(Debug, Clone, Copy, ValueEnum, PartialEq, Eq)]
pub enum SdmProfile {
    /// RRC/NAS signaling plus basic serving-cell context.
    Signaling,
    /// Serving/neighbor PHY measurements plus basic serving-cell context.
    Radio,
    /// Union of signaling and radio selections.
    Full,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SdmHeader {
    pub length1: u16,
    pub zero: u8,
    pub length2: u16,
    pub stamp: u16,
    pub direction: u8,
    pub radio_id: u8,
    pub group: u8,
    pub command: u8,
    pub timestamp: u32,
}

impl SdmHeader {
    pub fn synthetic_log_code(self) -> u16 {
        SDM_SYNTHETIC_BASE | u16::from(self.group & 0x1f)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SdmFrame {
    pub header: SdmHeader,
    /// Complete wire packet, including 0x7f start byte and 0x7e terminator.
    pub packet: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SdmError {
    HeaderTooShort,
    InvalidLengthRelation { length1: u16, length2: u16 },
    PacketTooLarge(usize),
    BadTerminator(u8),
}

impl fmt::Display for SdmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::HeaderTooShort => write!(f, "Samsung SDM header is shorter than 14 bytes"),
            Self::InvalidLengthRelation { length1, length2 } => write!(
                f,
                "Samsung SDM inner/outer lengths disagree (length1={length1}, length2={length2})"
            ),
            Self::PacketTooLarge(size) => write!(f, "Samsung SDM packet length {size} exceeds limit"),
            Self::BadTerminator(value) => {
                write!(f, "Samsung SDM packet has bad terminator 0x{value:02x}")
            }
        }
    }
}

impl std::error::Error for SdmError {}

pub fn parse_header(header: &[u8]) -> Result<SdmHeader, SdmError> {
    if header.len() < SDM_HEADER_LEN {
        return Err(SdmError::HeaderTooShort);
    }
    let length1 = u16::from_le_bytes([header[0], header[1]]);
    let zero = header[2];
    let length2 = u16::from_le_bytes([header[3], header[4]]);
    let stamp = u16::from_le_bytes([header[5], header[6]]);
    let direction = header[7];
    let group_raw = header[8];
    let command = header[9];
    let timestamp = u32::from_le_bytes([header[10], header[11], header[12], header[13]]);
    Ok(SdmHeader {
        length1,
        zero,
        length2,
        stamp,
        direction,
        radio_id: group_raw >> 5,
        group: group_raw & 0x1f,
        command,
        timestamp,
    })
}

/// Generate the SDM packet shape used by SCAT's `generate_sdm_packet`.
pub fn generate_packet(
    direction: u8,
    group: u8,
    command: u8,
    payload: &[u8],
    timestamp: u32,
) -> Result<Vec<u8>, SdmError> {
    // SCAT: pkt_len = 2 + 3 + 4 + payload + 2; length1 = pkt_len + 3.
    let length2 = 11usize
        .checked_add(payload.len())
        .ok_or(SdmError::PacketTooLarge(usize::MAX))?;
    let length1 = length2 + 3;
    let total = length1 + 2;
    if total > MAX_SDM_PACKET || length1 > u16::MAX as usize {
        return Err(SdmError::PacketTooLarge(total));
    }

    let mut out = Vec::with_capacity(total);
    out.push(SDM_START);
    out.extend_from_slice(&(length1 as u16).to_le_bytes());
    out.push(0);
    out.extend_from_slice(&(length2 as u16).to_le_bytes());
    out.extend_from_slice(&0_u16.to_le_bytes()); // stamp
    out.push(direction);
    out.push(group);
    out.push(command);
    out.extend_from_slice(&timestamp.to_le_bytes());
    out.extend_from_slice(payload);
    out.push(SDM_END);
    Ok(out)
}

pub fn item_selection(items: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(1 + items.len() * 2);
    out.push(items.len().min(0xfe) as u8);
    for &item in items.iter().take(0xfe) {
        out.push(item);
        out.push(1);
    }
    out
}

pub fn select_all() -> Vec<u8> {
    vec![0xff]
}

pub fn deselect_all() -> Vec<u8> {
    vec![0x00]
}

pub fn stop_packet() -> Result<Vec<u8>, SdmError> {
    generate_packet(SDM_DIRECTION_DM, GROUP_CONTROL, CONTROL_STOP, &[], 0)
}

/// Build the initialization transaction adapted from SCAT `SamsungParser.init_diag`.
pub fn init_packets(
    start_magic: u32,
    profile: SdmProfile,
    all_items: bool,
) -> Result<Vec<Vec<u8>>, SdmError> {
    let mut packets = Vec::new();
    packets.push(generate_packet(
        SDM_DIRECTION_DM,
        GROUP_CONTROL,
        CONTROL_START,
        &start_magic.to_be_bytes(),
        0,
    )?);
    packets.push(generate_packet(
        SDM_DIRECTION_DM,
        GROUP_CONTROL,
        CHANGE_UPDATE_PERIOD_REQUEST,
        &[0x05],
        0,
    )?);

    for command in [
        COMMON_ITEM_SELECT_REQUEST,
        LTE_ITEM_SELECT_REQUEST,
        EDGE_ITEM_SELECT_REQUEST,
        HSPA_ITEM_SELECT_REQUEST,
        CDMA_ITEM_SELECT_REQUEST,
    ] {
        packets.push(generate_packet(
            SDM_DIRECTION_DM,
            GROUP_CONTROL,
            command,
            &deselect_all(),
            0,
        )?);
    }

    if all_items {
        for command in [
            COMMON_ITEM_SELECT_REQUEST,
            LTE_ITEM_SELECT_REQUEST,
            EDGE_ITEM_SELECT_REQUEST,
            HSPA_ITEM_SELECT_REQUEST,
            CDMA_ITEM_SELECT_REQUEST,
        ] {
            packets.push(generate_packet(
                SDM_DIRECTION_DM,
                GROUP_CONTROL,
                command,
                &select_all(),
                0,
            )?);
        }
        return Ok(packets);
    }

    let (common, lte) = profile_items(profile);
    packets.push(generate_packet(
        SDM_DIRECTION_DM,
        GROUP_CONTROL,
        COMMON_ITEM_SELECT_REQUEST,
        &item_selection(&common),
        0,
    )?);
    packets.push(generate_packet(
        SDM_DIRECTION_DM,
        GROUP_CONTROL,
        LTE_ITEM_SELECT_REQUEST,
        &item_selection(&lte),
        0,
    )?);
    Ok(packets)
}

fn profile_items(profile: SdmProfile) -> (Vec<u8>, Vec<u8>) {
    let common_context = [COMMON_BASIC_INFO, COMMON_CELL_INFO, COMMON_DATA_INFO];
    let common_signaling = [
        COMMON_SIGNALING_INFO,
        COMMON_MULTI_SIGNALING_INFO,
        COMMON_NR_RRC_SIGNALING_INFO,
        COMMON_NR_NAS_SIGNALING_INFO,
    ];
    let lte_context = [LTE_RRC_SERVING_CELL, LTE_RRC_STATUS];
    let lte_signaling = [LTE_RRC_OTA_PACKET, LTE_NAS_EMM_MESSAGE, LTE_NAS_ESM_MESSAGE];
    let lte_radio = [LTE_PHY_STATUS, LTE_PHY_CELL_SEARCH_MEAS, LTE_PHY_NCELL_INFO, LTE_L1_RF];

    match profile {
        SdmProfile::Signaling => (
            common_context
                .into_iter()
                .chain(common_signaling)
                .collect(),
            lte_context.into_iter().chain(lte_signaling).collect(),
        ),
        SdmProfile::Radio => (
            common_context.to_vec(),
            lte_context.into_iter().chain(lte_radio).collect(),
        ),
        SdmProfile::Full => (
            common_context
                .into_iter()
                .chain(common_signaling)
                .collect(),
            lte_context
                .into_iter()
                .chain(lte_signaling)
                .chain(lte_radio)
                .collect(),
        ),
    }
}

#[derive(Debug, Default)]
pub struct SdmDecoder {
    buffer: Vec<u8>,
}

impl SdmDecoder {
    pub fn push(&mut self, data: &[u8]) -> Vec<Result<SdmFrame, SdmError>> {
        self.buffer.extend_from_slice(data);
        let mut out = Vec::new();

        loop {
            let Some(start) = self.buffer.iter().position(|&b| b == SDM_START) else {
                // Keep no arbitrary garbage: a future 0x7f will establish framing.
                self.buffer.clear();
                break;
            };
            if start > 0 {
                self.buffer.drain(..start);
            }
            if self.buffer.len() < 1 + SDM_HEADER_LEN {
                break;
            }

            let header = match parse_header(&self.buffer[1..1 + SDM_HEADER_LEN]) {
                Ok(header) => header,
                Err(err) => {
                    out.push(Err(err));
                    self.buffer.drain(..1);
                    continue;
                }
            };

            if header.length1 != header.length2.saturating_add(3) {
                out.push(Err(SdmError::InvalidLengthRelation {
                    length1: header.length1,
                    length2: header.length2,
                }));
                self.buffer.drain(..1);
                continue;
            }

            let total = usize::from(header.length1) + 2;
            if total < 1 + SDM_HEADER_LEN + 1 || total > MAX_SDM_PACKET {
                out.push(Err(SdmError::PacketTooLarge(total)));
                self.buffer.drain(..1);
                continue;
            }
            if self.buffer.len() < total {
                break;
            }
            let terminator = self.buffer[total - 1];
            if terminator != SDM_END {
                out.push(Err(SdmError::BadTerminator(terminator)));
                // Match SCAT's resynchronization intent while being conservative:
                // discard only this start marker and search again.
                self.buffer.drain(..1);
                continue;
            }

            let packet: Vec<u8> = self.buffer.drain(..total).collect();
            out.push(Ok(SdmFrame { header, packet }));
        }

        out
    }
}

pub fn group_name(group: u8) -> &'static str {
    match group {
        GROUP_CONTROL => "control",
        GROUP_COMMON => "common",
        GROUP_LTE => "lte",
        GROUP_EDGE => "edge",
        GROUP_HSPA => "hspa",
        GROUP_TRACE => "trace",
        GROUP_IP => "ip",
        _ => "unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_packet_matches_scat_length_relationship() {
        let packet = generate_packet(0xa0, GROUP_COMMON, COMMON_BASIC_INFO, &[1, 2, 3], 0x12345678)
            .unwrap();
        assert_eq!(packet[0], SDM_START);
        assert_eq!(*packet.last().unwrap(), SDM_END);
        let header = parse_header(&packet[1..15]).unwrap();
        assert_eq!(header.length1, header.length2 + 3);
        assert_eq!(usize::from(header.length1) + 2, packet.len());
        assert_eq!(header.direction, 0xa0);
        assert_eq!(header.group, GROUP_COMMON);
        assert_eq!(header.command, COMMON_BASIC_INFO);
        assert_eq!(header.timestamp, 0x12345678);
    }

    #[test]
    fn decoder_handles_fragmentation_and_resynchronization() {
        let one = generate_packet(0xa0, GROUP_COMMON, COMMON_CELL_INFO, &[0xaa], 0).unwrap();
        let two = generate_packet(0xa0, GROUP_LTE, LTE_RRC_OTA_PACKET, &[0xbb, 0xcc], 1).unwrap();
        let mut stream = vec![0x00, 0x11, 0x22];
        stream.extend_from_slice(&one);
        stream.extend_from_slice(&two);

        let mut decoder = SdmDecoder::default();
        let split = 9;
        assert!(decoder.push(&stream[..split]).is_empty());
        let frames: Vec<_> = decoder
            .push(&stream[split..])
            .into_iter()
            .map(Result::unwrap)
            .collect();
        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0].header.group, GROUP_COMMON);
        assert_eq!(frames[1].header.group, GROUP_LTE);
        assert_eq!(frames[1].header.command, LTE_RRC_OTA_PACKET);
    }

    #[test]
    fn native_namespace_does_not_overlap_mipc_synthetic_ranges() {
        let header = SdmHeader {
            length1: 14,
            zero: 0,
            length2: 11,
            stamp: 0,
            direction: 0xa0,
            radio_id: 0,
            group: GROUP_LTE,
            command: LTE_RRC_OTA_PACKET,
            timestamp: 0,
        };
        assert_eq!(header.synthetic_log_code(), 0x2402);
        assert!(!(0x2000..=0x23ff).contains(&header.synthetic_log_code()));
    }

    #[test]
    fn signaling_profile_selects_lte_and_nr_signaling() {
        let (common, lte) = profile_items(SdmProfile::Signaling);
        assert!(common.contains(&COMMON_NR_RRC_SIGNALING_INFO));
        assert!(common.contains(&COMMON_NR_NAS_SIGNALING_INFO));
        assert!(lte.contains(&LTE_RRC_OTA_PACKET));
        assert!(lte.contains(&LTE_NAS_EMM_MESSAGE));
    }
}
