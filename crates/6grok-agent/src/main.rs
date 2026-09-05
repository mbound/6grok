//! 6grok edge acquisition agent.
//!
//! Qualcomm DIAG acquisition and active log-mask control are implemented as
//! original MIT-licensed Rust from publicly documented wire behavior.

mod profiles;
mod qualcomm;
mod vendor_records;

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use nix::sys::termios::{self, BaudRate, ControlFlags, SetArg, SpecialCharacterIndices};
use profiles::{merge_profiles, CaptureProfile};
use qualcomm::{
    disable_logging_request, encode_hdlc, group_log_codes, parse_id_ranges_response,
    parse_log_config_header, retrieve_id_ranges_request, set_mask_request, split_log_code,
    HdlcDecoder, LOG_CONFIG_DISABLE_OP, LOG_CONFIG_RETRIEVE_ID_RANGES_OP,
    LOG_CONFIG_SET_MASK_OP,
};
use sixgrok_core::{
    encode_wire_frame, parser_payload, qualcomm_log_code, CaptureFrame, Vendor,
};
use std::collections::BTreeSet;
use std::fs::{File, OpenOptions};
use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use vendor_records::{read_records, RecordFormat};

#[derive(Debug, Parser)]
#[command(name = "6grok-agent")]
#[command(about = "Cellular diagnostic acquisition agent for 6grok")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Read a live Qualcomm diagnostic serial byte stream.
    Serial {
        #[arg(long)]
        port: String,
        #[arg(long, default_value_t = 115_200)]
        baud: u32,
        #[arg(long, value_enum, default_value_t = VendorArg::Qualcomm)]
        vendor: VendorArg,
        /// Explicit Qualcomm log code to enable, e.g. --log 0xb0c0.
        /// May be repeated. Explicit codes are strict: unsupported items fail.
        #[arg(long = "log", value_parser = parse_u16_auto)]
        logs: Vec<u16>,
        /// Parser-derived Qualcomm capture profile. May be repeated.
        #[arg(long = "profile", value_enum)]
        profiles: Vec<CaptureProfile>,
        /// Save the exact incoming HDLC byte stream for lossless replay.
        #[arg(long)]
        raw_capture: Option<PathBuf>,
        /// Save normalized CaptureFrame objects as JSON Lines.
        #[arg(long)]
        frame_capture: Option<PathBuf>,
        /// Stream normalized frames to a 6grok-api ingest listener, e.g. 10.0.0.2:5566.
        #[arg(long)]
        server: Option<String>,
    },
    /// Query the modem's DIAG log equipment-ID ranges without changing masks.
    Probe {
        #[arg(long)]
        port: String,
        #[arg(long, default_value_t = 115_200)]
        baud: u32,
    },
    /// Disable DIAG logging using LOG_CONFIG_DISABLE_OP.
    Disable {
        #[arg(long)]
        port: String,
        #[arg(long, default_value_t = 115_200)]
        baud: u32,
    },
    /// Replay a saved raw Qualcomm diagnostic HDLC byte stream.
    Replay {
        path: PathBuf,
        #[arg(long, value_enum, default_value_t = VendorArg::Qualcomm)]
        vendor: VendorArg,
        /// Save normalized CaptureFrame objects as JSON Lines.
        #[arg(long)]
        frame_capture: Option<PathBuf>,
        /// Stream replayed frames to a 6grok-api ingest listener.
        #[arg(long)]
        server: Option<String>,
    },
    /// Decode MediaTek/Samsung vendor record streams or a raw vendor PDU.
    Records {
        path: PathBuf,
        #[arg(long, value_enum)]
        format: RecordFormat,
        /// Fixed parser/synthetic log code. Required for raw PDU input and for
        /// Samsung MIPC commands that are not already in the 0x20xx..0x23xx range.
        #[arg(long, value_parser = parse_u16_auto)]
        log: Option<u16>,
        #[arg(long)]
        frame_capture: Option<PathBuf>,
        #[arg(long)]
        server: Option<String>,
    },
    /// Replay normalized CaptureFrame JSON Lines for any supported vendor.
    Frames {
        path: PathBuf,
        #[arg(long)]
        server: Option<String>,
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
        Command::Serial {
            port,
            baud,
            vendor,
            logs,
            profiles,
            raw_capture,
            frame_capture,
            server,
        } => {
            ensure_qualcomm_serial(vendor)?;
            let mut reader = open_serial(&port, baud)?;
            if !logs.is_empty() || !profiles.is_empty() {
                configure_qualcomm_logs(&mut reader, &logs, &profiles)?;
            }
            run_qualcomm_stream(
                reader,
                true,
                open_optional(raw_capture.as_deref())?,
                open_optional(frame_capture.as_deref())?,
                WireSink::connect_optional(server.as_deref())?,
            )
        }
        Command::Probe { port, baud } => {
            let mut port = open_serial(&port, baud)?;
            let ranges = query_log_ranges(&mut port)?;
            println!("equipment_id,last_item,max_log_code");
            for (equip, last_item) in ranges.into_iter().enumerate() {
                let max_code = ((equip as u32) << 12) | (last_item & 0x0fff);
                println!("{equip},0x{last_item:03x},0x{max_code:04x}");
            }
            Ok(())
        }
        Command::Disable { port, baud } => {
            let mut port = open_serial(&port, baud)?;
            send_diag_packet(&mut port, &disable_logging_request())?;
            let response = wait_for_log_config_response(&mut port, LOG_CONFIG_DISABLE_OP)?;
            let (header, _) = parse_log_config_header(&response)?;
            if header.status != 0 {
                bail!("modem rejected disable request with status {}", header.status);
            }
            eprintln!("6grok-agent: modem DIAG logging disabled");
            Ok(())
        }
        Command::Replay {
            path,
            vendor,
            frame_capture,
            server,
        } => {
            ensure_qualcomm_serial(vendor)?;
            let reader = File::open(&path)
                .with_context(|| format!("opening capture {}", path.display()))?;
            run_qualcomm_stream(
                reader,
                false,
                None,
                open_optional(frame_capture.as_deref())?,
                WireSink::connect_optional(server.as_deref())?,
            )
        }
        Command::Records {
            path,
            format,
            log,
            frame_capture,
            server,
        } => run_vendor_records(
            &path,
            format,
            log,
            open_optional(frame_capture.as_deref())?,
            WireSink::connect_optional(server.as_deref())?,
        ),
        Command::Frames { path, server } => {
            replay_normalized_frames(&path, WireSink::connect_optional(server.as_deref())?)
        }
    }
}

fn ensure_qualcomm_serial(vendor: VendorArg) -> Result<()> {
    if !matches!(vendor, VendorArg::Qualcomm) {
        bail!("serial/replay HDLC framing is the Qualcomm backend; use `records` for MediaTek/Samsung vendor records")
    }
    Ok(())
}

/// Open a POSIX TTY in raw 8N1 mode using the MIT-licensed `nix` crate.
///
/// VMIN=0 and VTIME=10 give a one-second read timeout. That allows active DIAG
/// requests to fail cleanly rather than blocking forever while live capture
/// treats a timeout as an idle interval rather than EOF.
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
    attrs.control_chars[SpecialCharacterIndices::VMIN as usize] = 0;
    attrs.control_chars[SpecialCharacterIndices::VTIME as usize] = 10;

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

fn configure_qualcomm_logs(
    port: &mut File,
    explicit: &[u16],
    profiles: &[CaptureProfile],
) -> Result<()> {
    let ranges = query_log_ranges(port)?;
    let explicit_set: BTreeSet<u16> = explicit.iter().copied().collect();
    let profile_codes = merge_profiles(&[], profiles);
    let mut selected = explicit.to_vec();

    for code in profile_codes {
        if explicit_set.contains(&code) {
            continue;
        }
        let (equip, item) = split_log_code(code);
        let last_item = ranges[usize::from(equip)];
        if u32::from(item) <= last_item {
            selected.push(code);
        } else {
            eprintln!(
                "6grok-agent: profile log 0x{code:04x} unsupported by modem (equipment {equip} last_item=0x{last_item:03x}); skipping"
            );
        }
    }
    selected.sort_unstable();
    selected.dedup();

    let grouped = group_log_codes(&selected);
    for (equip, items) in grouped {
        let last_item = ranges[usize::from(equip)];
        for &item in &items {
            let code = (u16::from(equip) << 12) | item;
            if explicit_set.contains(&code) && u32::from(item) > last_item {
                bail!(
                    "explicit log 0x{code:04x} exceeds modem equipment {equip} last_item 0x{last_item:03x}"
                );
            }
        }

        let request = set_mask_request(equip, last_item, &items)?;
        send_diag_packet(port, &request)?;
        let response = wait_for_log_config_response(port, LOG_CONFIG_SET_MASK_OP)?;
        let (header, _) = parse_log_config_header(&response)?;
        if header.status != 0 {
            bail!("modem rejected log mask for equipment ID {equip} with status {}", header.status);
        }

        let codes: Vec<String> = items
            .iter()
            .map(|item| format!("0x{:04x}", (u16::from(equip) << 12) | item))
            .collect();
        eprintln!(
            "6grok-agent: enabled equipment {equip} log mask through item 0x{last_item:03x}: {}",
            codes.join(", ")
        );
    }
    Ok(())
}

fn query_log_ranges(port: &mut File) -> Result<[u32; 16]> {
    send_diag_packet(port, &retrieve_id_ranges_request())?;
    let response = wait_for_log_config_response(port, LOG_CONFIG_RETRIEVE_ID_RANGES_OP)?;
    Ok(parse_id_ranges_response(&response)?)
}

fn send_diag_packet(port: &mut File, packet: &[u8]) -> Result<()> {
    let encoded = encode_hdlc(packet);
    port.write_all(&encoded).context("writing Qualcomm DIAG request")?;
    port.flush().context("flushing Qualcomm DIAG request")?;
    Ok(())
}

fn wait_for_log_config_response(port: &mut File, expected_operation: u32) -> Result<Vec<u8>> {
    let mut hdlc = HdlcDecoder::default();
    let mut buf = [0_u8; 4096];
    let mut idle_timeouts = 0_u8;

    loop {
        let n = match port.read(&mut buf) {
            Ok(0) => {
                idle_timeouts += 1;
                if idle_timeouts >= 5 {
                    bail!("timed out waiting for DIAG log-config operation {expected_operation}");
                }
                continue;
            }
            Ok(n) => {
                idle_timeouts = 0;
                n
            }
            Err(err) if err.kind() == io::ErrorKind::Interrupted => continue,
            Err(err) => return Err(err).context("reading Qualcomm DIAG response"),
        };

        for &byte in &buf[..n] {
            let Some(result) = hdlc.push(byte) else {
                continue;
            };
            let packet = match result {
                Ok(packet) => packet,
                Err(err) => {
                    eprintln!("6grok-agent: ignoring invalid DIAG response frame: {err}");
                    continue;
                }
            };

            if let Ok((header, _)) = parse_log_config_header(&packet) {
                if header.operation == expected_operation {
                    return Ok(packet);
                }
            }
        }
    }
}

fn run_qualcomm_stream<R: Read>(
    mut reader: R,
    live: bool,
    mut raw_capture: Option<File>,
    mut frame_capture: Option<File>,
    mut wire_sink: Option<WireSink>,
) -> Result<()> {
    let start = Instant::now();
    let mut sequence = 0_u64;
    let mut hdlc = HdlcDecoder::default();
    let mut buf = [0_u8; 8192];

    loop {
        let n = match reader.read(&mut buf) {
            Ok(0) if live => continue,
            Ok(0) => break,
            Ok(n) => n,
            Err(err) if err.kind() == io::ErrorKind::Interrupted => continue,
            Err(err) => return Err(err.into()),
        };

        if let Some(file) = raw_capture.as_mut() {
            file.write_all(&buf[..n])?;
        }

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
                continue;
            };

            sequence += 1;
            let frame = CaptureFrame {
                sequence,
                timestamp_wall: unix_ms(),
                timestamp_mono: start.elapsed().as_millis() as u64,
                vendor: Vendor::Qualcomm,
                log_code,
                payload: parser_payload(log_code, &packet),
            };
            emit_frame(&frame, &mut frame_capture, &mut wire_sink)?;
        }
    }

    flush_optional(&mut raw_capture)?;
    flush_optional(&mut frame_capture)?;
    Ok(())
}

fn run_vendor_records(
    path: &Path,
    format: RecordFormat,
    fixed_log_code: Option<u16>,
    mut frame_capture: Option<File>,
    mut wire_sink: Option<WireSink>,
) -> Result<()> {
    let reader = File::open(path)
        .with_context(|| format!("opening vendor record stream {}", path.display()))?;
    let start = Instant::now();
    let mut sequence = 0_u64;

    read_records(reader, format, fixed_log_code, |record| {
        sequence += 1;
        let frame = CaptureFrame {
            sequence,
            timestamp_wall: unix_ms(),
            timestamp_mono: start.elapsed().as_millis() as u64,
            vendor: record.vendor,
            log_code: record.log_code,
            payload: parser_payload(record.log_code, &record.packet),
        };
        emit_frame(&frame, &mut frame_capture, &mut wire_sink)
    })?;

    flush_optional(&mut frame_capture)?;
    Ok(())
}

fn replay_normalized_frames(path: &Path, mut wire_sink: Option<WireSink>) -> Result<()> {
    let file = File::open(path)
        .with_context(|| format!("opening normalized frame capture {}", path.display()))?;
    for (line_no, line) in BufReader::new(file).lines().enumerate() {
        let line = line.with_context(|| format!("reading line {}", line_no + 1))?;
        if line.trim().is_empty() {
            continue;
        }
        let frame: CaptureFrame = serde_json::from_str(&line)
            .with_context(|| format!("decoding CaptureFrame on line {}", line_no + 1))?;
        if let Some(sink) = wire_sink.as_mut() {
            sink.send(&frame)?;
        }
        println!("{}", serde_json::to_string(&frame.decode())?);
    }
    Ok(())
}

fn emit_frame(
    frame: &CaptureFrame,
    frame_capture: &mut Option<File>,
    wire_sink: &mut Option<WireSink>,
) -> Result<()> {
    if let Some(file) = frame_capture.as_mut() {
        serde_json::to_writer(&mut *file, frame)?;
        file.write_all(b"\n")?;
    }
    if let Some(sink) = wire_sink.as_mut() {
        sink.send(frame)?;
    }
    println!("{}", serde_json::to_string(&frame.decode())?);
    Ok(())
}

struct WireSink {
    stream: TcpStream,
}

impl WireSink {
    fn connect_optional(address: Option<&str>) -> Result<Option<Self>> {
        address
            .map(|address| {
                let stream = TcpStream::connect(address)
                    .with_context(|| format!("connecting to 6grok-api ingest at {address}"))?;
                stream
                    .set_nodelay(true)
                    .context("enabling TCP_NODELAY for 6grok-api uplink")?;
                eprintln!("6grok-agent: streaming frames to {address}");
                Ok(Self { stream })
            })
            .transpose()
    }

    fn send(&mut self, frame: &CaptureFrame) -> Result<()> {
        let payload = encode_wire_frame(frame).context("encoding MessagePack agent frame")?;
        let len = u32::try_from(payload.len()).context("agent frame exceeds u32 wire length")?;
        self.stream
            .write_all(&len.to_be_bytes())
            .context("writing agent frame length")?;
        self.stream
            .write_all(&payload)
            .context("writing agent MessagePack frame")?;
        Ok(())
    }
}

fn open_optional(path: Option<&Path>) -> Result<Option<File>> {
    path.map(|path| {
        File::create(path).with_context(|| format!("creating capture {}", path.display()))
    })
    .transpose()
}

fn flush_optional(file: &mut Option<File>) -> Result<()> {
    if let Some(file) = file.as_mut() {
        file.flush()?;
    }
    Ok(())
}

fn parse_u16_auto(value: &str) -> std::result::Result<u16, String> {
    let value = value.trim();
    if let Some(hex) = value.strip_prefix("0x").or_else(|| value.strip_prefix("0X")) {
        u16::from_str_radix(hex, 16).map_err(|err| err.to_string())
    } else {
        value.parse::<u16>().map_err(|err| err.to_string())
    }
}

fn unix_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_hex_and_decimal_log_codes() {
        assert_eq!(parse_u16_auto("0xb0c0").unwrap(), 0xb0c0);
        assert_eq!(parse_u16_auto("45248").unwrap(), 45248);
    }
}
