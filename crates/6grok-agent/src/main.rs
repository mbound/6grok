//! 6grok edge acquisition agent.
//!
//! The initial backend implements passive Qualcomm DIAG HDLC ingestion without
//! incorporating source code from GPL diagnostic projects.

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use crc::{Crc, CRC_16_IBM_SDLC};
use nix::sys::termios::{self, BaudRate, ControlFlags, SetArg};
use sixgrok_core::{parser_payload, qualcomm_log_code, CaptureFrame, Vendor};
use std::fs::{File, OpenOptions};
use std::io::{self, Read};
use std::path::PathBuf;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

const DIAG_CRC: Crc<u16> = Crc::<u16>::new(&CRC_16_IBM_SDLC);

#[derive(Debug, Parser)]
#[command(name = "6grok-agent")]
#[command(about = "Cellular diagnostic acquisition agent for 6grok")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Read a live diagnostic serial byte stream.
    Serial {
        #[arg(long)]
        port: String,
        #[arg(long, default_value_t = 115_200)]
        baud: u32,
        #[arg(long, value_enum, default_value_t = VendorArg::Qualcomm)]
        vendor: VendorArg,
    },
    /// Replay a saved raw diagnostic byte stream.
    Replay {
        path: PathBuf,
        #[arg(long, value_enum, default_value_t = VendorArg::Qualcomm)]
        vendor: VendorArg,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum VendorArg {
    Qualcomm,
    Mediatek,
    Samsung,
}

impl From<VendorArg> for Vendor {
    fn from(value: VendorArg) -> Self {
        match value {
            VendorArg::Qualcomm => Vendor::Qualcomm,
            VendorArg::Mediatek => Vendor::Mediatek,
            VendorArg::Samsung => Vendor::Samsung,
        }
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Serial { port, baud, vendor } => {
            ensure_mvp_vendor(vendor)?;
            let reader = open_serial(&port, baud)?;
            run_stream(reader, vendor.into())
        }
        Command::Replay { path, vendor } => {
            ensure_mvp_vendor(vendor)?;
            let reader = File::open(&path)
                .with_context(|| format!("opening capture {}", path.display()))?;
            run_stream(reader, vendor.into())
        }
    }
}

fn ensure_mvp_vendor(vendor: VendorArg) -> Result<()> {
    if !matches!(vendor, VendorArg::Qualcomm) {
        bail!("the first acquisition backend supports Qualcomm DIAG framing only; MediaTek and Samsung collectors are planned behind the same normalized frame interface");
    }
    Ok(())
}

/// Open a POSIX TTY in raw 8N1 mode using the MIT-licensed `nix` crate.
///
/// USB modem diagnostic endpoints generally ignore baud rate, while UART-backed
/// modules may require it. We intentionally avoid the MPL-2.0 `serialport` crate
/// to keep the shipped dependency set permissively licensed.
fn open_serial(path: &str, baud: u32) -> Result<File> {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
        .with_context(|| format!("opening diagnostic serial port {path}"))?;

    let mut attrs = termios::tcgetattr(&file)
        .with_context(|| format!("reading termios settings for {path}"))?;
    termios::cfmakeraw(&mut attrs);

    attrs.control_flags.remove(
        ControlFlags::CSIZE | ControlFlags::PARENB | ControlFlags::CSTOPB,
    );
    attrs.control_flags.insert(
        ControlFlags::CS8 | ControlFlags::CLOCAL | ControlFlags::CREAD,
    );

    let rate = baud_rate(baud)?;
    termios::cfsetispeed(&mut attrs, rate)?;
    termios::cfsetospeed(&mut attrs, rate)?;
    termios::tcsetattr(&file, SetArg::TCSANOW, &attrs)
        .with_context(|| format!("configuring raw serial mode for {path}"))?;

    Ok(file)
}

fn baud_rate(baud: u32) -> Result<BaudRate> {
    Ok(match baud {
        9_600 => BaudRate::B9600,
        19_200 => BaudRate::B19200,
        38_400 => BaudRate::B38400,
        57_600 => BaudRate::B57600,
        115_200 => BaudRate::B115200,
        230_400 => BaudRate::B230400,
        460_800 => BaudRate::B460800,
        921_600 => BaudRate::B921600,
        _ => bail!("unsupported termios baud rate {baud}; use one of 9600, 19200, 38400, 57600, 115200, 230400, 460800, 921600"),
    })
}

fn run_stream<R: Read>(mut reader: R, vendor: Vendor) -> Result<()> {
    let start = Instant::now();
    let mut sequence = 0_u64;
    let mut hdlc = QualcommHdlcDecoder::default();
    let mut buf = [0_u8; 8192];

    loop {
        let n = match reader.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(err) if err.kind() == io::ErrorKind::Interrupted => continue,
            Err(err) => return Err(err.into()),
        };

        for &byte in &buf[..n] {
            let Some(result) = hdlc.push(byte) else {
                continue;
            };
            let packet = match result {
                Ok(packet) => packet,
                Err(err) => {
                    eprintln!("6grok-agent: dropping invalid DIAG frame: {err}");
                    continue;
                }
            };

            let Some(log_code) = qualcomm_log_code(&packet) else {
                // Command responses and other DIAG packet families are not log
                // records. A future control plane will consume them separately.
                continue;
            };

            sequence += 1;
            let frame = CaptureFrame {
                sequence,
                timestamp_wall: unix_ms(),
                timestamp_mono: start.elapsed().as_millis() as u64,
                vendor,
                log_code,
                payload: parser_payload(log_code, &packet),
            };

            let decoded = frame.decode();
            println!("{}", serde_json::to_string(&decoded)?);
        }
    }

    Ok(())
}

fn unix_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

#[derive(Debug, Default)]
struct QualcommHdlcDecoder {
    frame: Vec<u8>,
    escaped: bool,
}

impl QualcommHdlcDecoder {
    /// Feed one byte from a Qualcomm DIAG HDLC stream.
    ///
    /// Protocol facts independently implemented here: frames are delimited by
    /// 0x7e; 0x7d escapes the next byte using XOR 0x20; the final two unescaped
    /// bytes are a little-endian CCITT/X.25 FCS. Public DIAG documentation and
    /// multiple independent tools describe this wire format; no implementation
    /// source was copied into this file.
    fn push(&mut self, byte: u8) -> Option<Result<Vec<u8>, FrameError>> {
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

#[derive(Debug)]
enum FrameError {
    TooShort,
    BadCrc { expected: u16, actual: u16 },
}

impl std::fmt::Display for FrameError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooShort => write!(f, "frame shorter than DIAG payload + CRC"),
            Self::BadCrc { expected, actual } => {
                write!(f, "CRC mismatch (wire=0x{expected:04x}, calculated=0x{actual:04x})")
            }
        }
    }
}

fn validate_diag_frame(mut raw: Vec<u8>) -> Result<Vec<u8>, FrameError> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc_variant_matches_standard_check_value() {
        assert_eq!(DIAG_CRC.checksum(b"123456789"), 0x906e);
    }

    #[test]
    fn hdlc_unescapes_and_validates() {
        let payload = [0x10, 0x7e, 0x7d, 0x01];
        let crc = DIAG_CRC.checksum(&payload).to_le_bytes();
        let mut encoded = Vec::new();
        for byte in payload.into_iter().chain(crc) {
            match byte {
                0x7e | 0x7d => {
                    encoded.push(0x7d);
                    encoded.push(byte ^ 0x20);
                }
                _ => encoded.push(byte),
            }
        }
        encoded.push(0x7e);

        let mut decoder = QualcommHdlcDecoder::default();
        let mut out = None;
        for byte in encoded {
            if let Some(result) = decoder.push(byte) {
                out = Some(result.unwrap());
            }
        }
        assert_eq!(out.unwrap(), payload);
    }
}
