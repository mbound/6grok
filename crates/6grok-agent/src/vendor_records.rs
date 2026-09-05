//! Vendor-record adapters for diagnostic captures obtained outside the
//! Qualcomm DIAG transport.
//!
//! These adapters deliberately stop at the record/envelope boundary. Device-
//! specific USB/kernel acquisition can feed them without being coupled to the
//! parser or API layers.

use anyhow::{bail, Context, Result};
use clap::ValueEnum;
use sixgrok_core::Vendor;
use std::io::{ErrorKind, Read};

const MTK_HEADER_LEN: usize = 9;
const SAMSUNG_HEADER_LEN: usize = 16;
const MAX_RECORD: usize = 8 * 1024 * 1024;

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum RecordFormat {
    /// 9-byte MediaTek record header followed by its payload.
    Mediatek,
    /// 16-byte Samsung MIPC record header followed by its payload.
    Samsung,
    /// Treat the entire input as one raw vendor PDU; requires --log.
    Raw,
}

#[derive(Debug)]
pub struct VendorRecord {
    pub vendor: Vendor,
    pub log_code: u16,
    /// Vendor packet passed after the parser's two-byte normalized log prefix.
    pub packet: Vec<u8>,
}

pub fn read_records<R: Read, F: FnMut(VendorRecord) -> Result<()>>(
    mut reader: R,
    format: RecordFormat,
    fixed_log_code: Option<u16>,
    mut sink: F,
) -> Result<()> {
    match format {
        RecordFormat::Mediatek => loop {
            let Some(header) = read_exact_or_eof::<_, MTK_HEADER_LEN>(&mut reader)? else {
                break;
            };
            let log_code = u16::from_le_bytes([header[0], header[1]]);
            let length = u16::from_le_bytes([header[6], header[7]]) as usize;
            if length > MAX_RECORD {
                bail!("MediaTek record payload {length} exceeds limit");
            }
            let mut packet = Vec::with_capacity(MTK_HEADER_LEN + length);
            packet.extend_from_slice(&header);
            let start = packet.len();
            packet.resize(start + length, 0);
            reader
                .read_exact(&mut packet[start..])
                .context("reading MediaTek record payload")?;
            sink(VendorRecord {
                vendor: Vendor::Mediatek,
                log_code,
                packet,
            })?;
        },
        RecordFormat::Samsung => loop {
            let Some(header) = read_exact_or_eof::<_, SAMSUNG_HEADER_LEN>(&mut reader)? else {
                break;
            };
            let command = u16::from_le_bytes([header[4], header[5]]);
            let length = u16::from_le_bytes([header[6], header[7]]) as usize;
            if length > MAX_RECORD {
                bail!("Samsung record payload {length} exceeds limit");
            }
            let log_code = fixed_log_code.or_else(|| {
                (0x2000..=0x23ff)
                    .contains(&command)
                    .then_some(command)
            });
            let log_code = log_code.ok_or_else(|| {
                anyhow::anyhow!(
                    "Samsung MIPC command 0x{command:04x} is not a 5grok synthetic log code; provide --log 0x20xx/0x21xx/0x22xx"
                )
            })?;
            let mut packet = Vec::with_capacity(SAMSUNG_HEADER_LEN + length);
            packet.extend_from_slice(&header);
            let start = packet.len();
            packet.resize(start + length, 0);
            reader
                .read_exact(&mut packet[start..])
                .context("reading Samsung MIPC record payload")?;
            sink(VendorRecord {
                vendor: Vendor::Samsung,
                log_code,
                packet,
            })?;
        },
        RecordFormat::Raw => {
            let log_code = fixed_log_code.context("raw vendor input requires --log")?;
            let vendor = if (0x2000..=0x23ff).contains(&log_code) {
                Vendor::Samsung
            } else if matches!(
                log_code,
                0x0c00..=0x0eff | 0x1c00..=0x1eff
            ) {
                Vendor::Mediatek
            } else {
                bail!("cannot infer raw-PDU vendor from log code 0x{log_code:04x}; use a MediaTek or Samsung synthetic/parser-supported code")
            };
            let mut packet = Vec::new();
            reader
                .take(MAX_RECORD as u64 + 1)
                .read_to_end(&mut packet)
                .context("reading raw vendor PDU")?;
            if packet.len() > MAX_RECORD {
                bail!("raw vendor PDU exceeds {MAX_RECORD} bytes");
            }
            if !packet.is_empty() {
                sink(VendorRecord {
                    vendor,
                    log_code,
                    packet,
                })?;
            }
        }
    }
    Ok(())
}

fn read_exact_or_eof<R: Read, const N: usize>(reader: &mut R) -> Result<Option<[u8; N]>> {
    let mut out = [0_u8; N];
    let mut offset = 0;
    while offset < N {
        match reader.read(&mut out[offset..]) {
            Ok(0) if offset == 0 => return Ok(None),
            Ok(0) => bail!("truncated vendor record header ({offset}/{N} bytes)"),
            Ok(n) => offset += n,
            Err(err) if err.kind() == ErrorKind::Interrupted => continue,
            Err(err) => return Err(err).context("reading vendor record header"),
        }
    }
    Ok(Some(out))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_mediatek_record() {
        let mut bytes = vec![0x01, 0x1d, 1, 2, 3, 4, 3, 0, 1];
        bytes.extend_from_slice(&[0x7e, 0x00, 0x41]);
        let mut got = Vec::new();
        read_records(bytes.as_slice(), RecordFormat::Mediatek, None, |record| {
            got.push(record);
            Ok(())
        })
        .unwrap();
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].vendor, Vendor::Mediatek);
        assert_eq!(got[0].log_code, 0x1d01);
        assert_eq!(got[0].packet.len(), 12);
    }

    #[test]
    fn parses_samsung_synthetic_command() {
        let mut header = [0_u8; 16];
        header[..4].copy_from_slice(&0x4d49_5043_u32.to_le_bytes());
        header[4..6].copy_from_slice(&0x2060_u16.to_le_bytes());
        header[6..8].copy_from_slice(&2_u16.to_le_bytes());
        let mut bytes = header.to_vec();
        bytes.extend_from_slice(&[0x7e, 0x41]);
        let mut got = Vec::new();
        read_records(bytes.as_slice(), RecordFormat::Samsung, None, |record| {
            got.push(record);
            Ok(())
        })
        .unwrap();
        assert_eq!(got[0].vendor, Vendor::Samsung);
        assert_eq!(got[0].log_code, 0x2060);
    }
}
