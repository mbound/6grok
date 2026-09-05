// SPDX-FileCopyrightText: 2026 mbound
// SPDX-License-Identifier: GPL-3.0-or-later
//
// Interoperability backend for the TCP bridge shipped by P1sec/QCSuper.
// QCSuper provenance used by this module:
//   repository: https://github.com/P1sec/QCSuper
//   commit: aa555b4f7f25f7a8bf4e5afd4dcb884edf2f6735
//   bridge client: src/qcsuper/inputs/adb.py
//   bridge server: src/qcsuper/inputs/adb_bridge/adb_bridge.c
// The wire-compatible implementation below is written in Rust for 6grok;
// QCSuper's bridge protocol and active log selections are GPL-3.0-or-later.

mod profiles;
mod protocol;

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use profiles::{merge_profiles, QcsuperProfile};
use protocol::{
    disable_logging_request, encode_hdlc, group_log_codes, parse_id_ranges_response,
    parse_log_config_header, retrieve_id_ranges_request, set_mask_request, split_log_code,
    HdlcDecoder, LOG_CONFIG_DISABLE_OP, LOG_CONFIG_RETRIEVE_ID_RANGES_OP,
    LOG_CONFIG_SET_MASK_OP,
};
use sixgrok_core::{
    encode_wire_frame, parser_payload, qualcomm_log_code, CaptureFrame, Vendor,
};
use std::collections::BTreeSet;
use std::fs::File;
use std::io::{self, Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const DEFAULT_QCSUPER_BRIDGE: &str = "127.0.0.1:43555";

#[derive(Debug, Parser)]
#[command(name = "6grok-qcsuper")]
#[command(about = "GPL Qualcomm backend for QCSuper Android /dev/diag bridge interoperability")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Capture Qualcomm DIAG from a running QCSuper adb_bridge TCP endpoint.
    Capture {
        /// QCSuper bridge address. Its default TCP port is 43555.
        #[arg(long, default_value = DEFAULT_QCSUPER_BRIDGE)]
        bridge: String,
        /// Explicit Qualcomm log code to enable, e.g. --log 0xb821.
        #[arg(long = "log", value_parser = parse_u16_auto)]
        logs: Vec<u16>,
        /// QCSuper-derived capture profile. May be repeated.
        #[arg(long = "profile", value_enum)]
        profiles: Vec<QcsuperProfile>,
        /// Save normalized CaptureFrame objects as JSON Lines.
        #[arg(long)]
        frame_capture: Option<PathBuf>,
        /// Stream normalized frames to a 6grok-api ingest listener.
        #[arg(long)]
        server: Option<String>,
    },
    /// Query the bridge-connected modem's DIAG log equipment-ID ranges.
    Probe {
        #[arg(long, default_value = DEFAULT_QCSUPER_BRIDGE)]
        bridge: String,
    },
    /// Disable DIAG logging on the bridge-connected modem.
    Disable {
        #[arg(long, default_value = DEFAULT_QCSUPER_BRIDGE)]
        bridge: String,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Capture {
            bridge,
            logs,
            profiles,
            frame_capture,
            server,
        } => {
            let mut stream = connect_bridge(&bridge)?;
            if !logs.is_empty() || !profiles.is_empty() {
                configure_logs(&mut stream, &logs, &profiles)?;
            }
            run_capture(
                stream,
                open_optional(frame_capture.as_deref())?,
                WireSink::connect_optional(server.as_deref())?,
            )
        }
        Command::Probe { bridge } => {
            let mut stream = connect_bridge(&bridge)?;
            let ranges = query_log_ranges(&mut stream)?;
            println!("equipment_id,last_item,max_log_code");
            for (equip, last_item) in ranges.into_iter().enumerate() {
                let max_code = ((equip as u32) << 12) | (last_item & 0x0fff);
                println!("{equip},0x{last_item:03x},0x{max_code:04x}");
            }
            Ok(())
        }
        Command::Disable { bridge } => {
            let mut stream = connect_bridge(&bridge)?;
            send_diag_packet(&mut stream, &disable_logging_request())?;
            let response = wait_for_log_config_response(&mut stream, LOG_CONFIG_DISABLE_OP)?;
            let (header, _) = parse_log_config_header(&response)?;
            if header.status != 0 {
                bail!("modem rejected disable request with status {}", header.status);
            }
            eprintln!("6grok-qcsuper: modem DIAG logging disabled");
            Ok(())
        }
    }
}

fn connect_bridge(address: &str) -> Result<TcpStream> {
    let stream = TcpStream::connect(address)
        .with_context(|| format!("connecting to QCSuper adb_bridge at {address}"))?;
    stream
        .set_nodelay(true)
        .context("enabling TCP_NODELAY on QCSuper bridge")?;
    stream
        .set_read_timeout(Some(Duration::from_secs(1)))
        .context("setting QCSuper bridge read timeout")?;
    eprintln!("6grok-qcsuper: connected to QCSuper bridge at {address}");
    Ok(stream)
}

fn configure_logs(
    stream: &mut TcpStream,
    explicit: &[u16],
    profiles: &[QcsuperProfile],
) -> Result<()> {
    let ranges = query_log_ranges(stream)?;
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
                "6grok-qcsuper: profile log 0x{code:04x} unsupported by modem (equipment {equip} last_item=0x{last_item:03x}); skipping"
            );
        }
    }
    selected.sort_unstable();
    selected.dedup();

    for (equip, items) in group_log_codes(&selected) {
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
        send_diag_packet(stream, &request)?;
        let response = wait_for_log_config_response(stream, LOG_CONFIG_SET_MASK_OP)?;
        let (header, _) = parse_log_config_header(&response)?;
        if header.status != 0 {
            bail!(
                "modem rejected log mask for equipment ID {equip} with status {}",
                header.status
            );
        }

        let codes: Vec<String> = items
            .iter()
            .map(|item| format!("0x{:04x}", (u16::from(equip) << 12) | item))
            .collect();
        eprintln!(
            "6grok-qcsuper: enabled equipment {equip} through last_item 0x{last_item:03x}: {}",
            codes.join(", ")
        );
    }

    Ok(())
}

fn query_log_ranges(stream: &mut TcpStream) -> Result<[u32; 16]> {
    send_diag_packet(stream, &retrieve_id_ranges_request())?;
    let response = wait_for_log_config_response(stream, LOG_CONFIG_RETRIEVE_ID_RANGES_OP)?;
    Ok(parse_id_ranges_response(&response)?)
}

fn send_diag_packet(stream: &mut TcpStream, packet: &[u8]) -> Result<()> {
    stream
        .write_all(&encode_hdlc(packet))
        .context("writing DIAG request to QCSuper bridge")?;
    stream.flush().context("flushing QCSuper bridge request")?;
    Ok(())
}

fn wait_for_log_config_response(
    stream: &mut TcpStream,
    expected_operation: u32,
) -> Result<Vec<u8>> {
    let mut hdlc = HdlcDecoder::default();
    let mut buf = [0_u8; 8192];
    let mut idle_timeouts = 0_u8;

    loop {
        let n = match stream.read(&mut buf) {
            Ok(0) => bail!("QCSuper bridge closed while waiting for DIAG response"),
            Ok(n) => {
                idle_timeouts = 0;
                n
            }
            Err(err)
                if err.kind() == io::ErrorKind::WouldBlock
                    || err.kind() == io::ErrorKind::TimedOut =>
            {
                idle_timeouts += 1;
                if idle_timeouts >= 5 {
                    bail!(
                        "timed out waiting for DIAG log-config operation {expected_operation}"
                    );
                }
                continue;
            }
            Err(err) if err.kind() == io::ErrorKind::Interrupted => continue,
            Err(err) => return Err(err).context("reading QCSuper bridge DIAG response"),
        };

        for &byte in &buf[..n] {
            let Some(result) = hdlc.push(byte) else {
                continue;
            };
            let packet = match result {
                Ok(packet) => packet,
                Err(err) => {
                    eprintln!("6grok-qcsuper: ignoring invalid DIAG frame: {err}");
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

fn run_capture(
    mut stream: TcpStream,
    mut frame_capture: Option<File>,
    mut wire_sink: Option<WireSink>,
) -> Result<()> {
    // The bridge is a streaming endpoint once setup is complete. Remove the
    // request/response timeout so an idle radio does not cause a busy loop.
    stream
        .set_read_timeout(None)
        .context("clearing QCSuper bridge read timeout")?;

    let start = Instant::now();
    let mut sequence = 0_u64;
    let mut hdlc = HdlcDecoder::default();
    let mut buf = [0_u8; 64 * 1024];

    loop {
        let n = match stream.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(err) if err.kind() == io::ErrorKind::Interrupted => continue,
            Err(err) => return Err(err).context("reading QCSuper bridge stream"),
        };

        for &byte in &buf[..n] {
            let Some(result) = hdlc.push(byte) else {
                continue;
            };
            let packet = match result {
                Ok(packet) => packet,
                Err(err) => {
                    eprintln!("6grok-qcsuper: dropping invalid DIAG frame: {err}");
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

    flush_optional(&mut frame_capture)?;
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
                eprintln!("6grok-qcsuper: streaming frames to {address}");
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
    if let Some(hex) = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
    {
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
        assert_eq!(parse_u16_auto("0xb821").unwrap(), 0xb821);
        assert_eq!(parse_u16_auto("47137").unwrap(), 47137);
    }
}
