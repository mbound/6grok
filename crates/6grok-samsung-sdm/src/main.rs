// SPDX-FileCopyrightText: 2026 mbound
// SPDX-License-Identifier: GPL-2.0-or-later
//
// Native Samsung Shannon SDM collector derived from the framing/control model
// in fgsect/scat, commit 361ff551a4fbb30789c46750c00586682a7a9b26.
// See sdm.rs and THIRD_PARTY.md for file-level provenance.

mod sdm;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use sdm::{
    group_name, init_packets, stop_packet, SdmDecoder, SdmProfile, DEFAULT_START_MAGIC,
};
use sixgrok_core::{encode_wire_frame, parser_payload, CaptureFrame, Vendor};
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

const DEFAULT_DEVICE: &str = "/dev/umts_dm0";

#[derive(Debug, Parser)]
#[command(name = "6grok-samsung-sdm")]
#[command(about = "Native Samsung Shannon SDM acquisition backend for 6grok")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Initialize SDM and capture from a Shannon diagnostic character device.
    Capture {
        /// Shannon diagnostic device, commonly /dev/umts_dm0 on Samsung devices.
        #[arg(long, default_value = DEFAULT_DEVICE)]
        device: String,
        /// Curated SDM item selection.
        #[arg(long, value_enum, default_value_t = SdmProfile::Signaling)]
        profile: SdmProfile,
        /// Ask SDM for every item in the COMMON/LTE/EDGE/HSPA/CDMA groups.
        /// This can generate very high log volume.
        #[arg(long)]
        all_items: bool,
        /// Skip CONTROL_START/item-selection writes and only read an already-running SDM stream.
        #[arg(long)]
        passive: bool,
        /// CONTROL_START magic, decimal or 0x-prefixed hexadecimal.
        #[arg(long, value_parser = parse_u32_auto, default_value = "0x41414141")]
        start_magic: u32,
        /// Save the exact incoming SDM byte stream for lossless replay.
        #[arg(long)]
        raw_capture: Option<PathBuf>,
        /// Save normalized CaptureFrame objects as JSON Lines.
        #[arg(long)]
        frame_capture: Option<PathBuf>,
        /// Stream normalized frames to a 6grok-api ingest listener.
        #[arg(long)]
        server: Option<String>,
    },
    /// Send SDM CONTROL_STOP to a Shannon diagnostic character device.
    Stop {
        #[arg(long, default_value = DEFAULT_DEVICE)]
        device: String,
    },
    /// Replay a saved raw native SDM byte stream.
    Replay {
        path: PathBuf,
        #[arg(long)]
        frame_capture: Option<PathBuf>,
        #[arg(long)]
        server: Option<String>,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Capture {
            device,
            profile,
            all_items,
            passive,
            start_magic,
            raw_capture,
            frame_capture,
            server,
        } => {
            let mut io = open_device(&device)?;
            if !passive {
                configure_sdm(&mut io, start_magic, profile, all_items)?;
            }
            eprintln!(
                "6grok-samsung-sdm: capturing {} SDM stream from {}{}",
                if all_items { "all-item" } else { profile_name(profile) },
                device,
                if passive { " (passive)" } else { "" }
            );
            run_stream(
                io,
                true,
                open_optional(raw_capture.as_deref())?,
                open_optional(frame_capture.as_deref())?,
                WireSink::connect_optional(server.as_deref())?,
            )
        }
        Command::Stop { device } => {
            let mut io = open_device(&device)?;
            io.write_all(&stop_packet()?)
                .with_context(|| format!("writing SDM CONTROL_STOP to {device}"))?;
            io.flush().context("flushing SDM CONTROL_STOP")?;
            eprintln!("6grok-samsung-sdm: CONTROL_STOP sent to {device}");
            Ok(())
        }
        Command::Replay {
            path,
            frame_capture,
            server,
        } => {
            let input = File::open(&path)
                .with_context(|| format!("opening SDM capture {}", path.display()))?;
            run_stream(
                input,
                false,
                None,
                open_optional(frame_capture.as_deref())?,
                WireSink::connect_optional(server.as_deref())?,
            )
        }
    }
}

fn open_device(path: &str) -> Result<File> {
    OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
        .with_context(|| format!("opening Samsung Shannon SDM device {path}"))
}

fn configure_sdm(
    io: &mut File,
    start_magic: u32,
    profile: SdmProfile,
    all_items: bool,
) -> Result<()> {
    for packet in init_packets(start_magic, profile, all_items)? {
        io.write_all(&packet).context("writing Samsung SDM control packet")?;
    }
    io.flush().context("flushing Samsung SDM initialization")?;
    eprintln!(
        "6grok-samsung-sdm: SDM initialized (start_magic=0x{start_magic:08x}, selection={})",
        if all_items { "all" } else { profile_name(profile) }
    );
    Ok(())
}

fn run_stream<R: Read>(
    mut reader: R,
    live: bool,
    mut raw_capture: Option<File>,
    mut frame_capture: Option<File>,
    mut wire_sink: Option<WireSink>,
) -> Result<()> {
    let start = Instant::now();
    let mut sequence = 0_u64;
    let mut decoder = SdmDecoder::default();
    let mut buf = [0_u8; 64 * 1024];

    loop {
        let n = match reader.read(&mut buf) {
            Ok(0) if live => continue,
            Ok(0) => break,
            Ok(n) => n,
            Err(err) if err.kind() == io::ErrorKind::Interrupted => continue,
            Err(err) => return Err(err).context("reading Samsung SDM stream"),
        };

        if let Some(file) = raw_capture.as_mut() {
            file.write_all(&buf[..n])?;
        }

        for result in decoder.push(&buf[..n]) {
            let sdm = match result {
                Ok(frame) => frame,
                Err(err) => {
                    eprintln!("6grok-samsung-sdm: dropping/resynchronizing invalid SDM frame: {err}");
                    continue;
                }
            };
            sequence += 1;
            let log_code = sdm.header.synthetic_log_code();
            let frame = CaptureFrame {
                sequence,
                timestamp_wall: unix_ms(),
                timestamp_mono: start.elapsed().as_millis() as u64,
                vendor: Vendor::Samsung,
                log_code,
                payload: parser_payload(log_code, &sdm.packet),
            };
            emit_frame(&frame, &sdm.header, &mut frame_capture, &mut wire_sink)?;
        }
    }

    flush_optional(&mut raw_capture)?;
    flush_optional(&mut frame_capture)?;
    Ok(())
}

fn emit_frame(
    frame: &CaptureFrame,
    header: &sdm::SdmHeader,
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

    // Preserve the standard decoded-packet object while adding native SDM
    // envelope metadata that the surviving parser does not yet model.
    let output = serde_json::json!({
        "sdm": {
            "direction": format!("0x{:02x}", header.direction),
            "radio_id": header.radio_id,
            "group": header.group,
            "group_name": group_name(header.group),
            "command": header.command,
            "timestamp": header.timestamp,
            "native_log_code": format!("0x{:04x}", frame.log_code),
        },
        "decoded": frame.decode(),
    });
    println!("{}", serde_json::to_string(&output)?);
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
                    .context("enabling TCP_NODELAY for Samsung SDM uplink")?;
                eprintln!("6grok-samsung-sdm: streaming frames to {address}");
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

fn parse_u32_auto(value: &str) -> std::result::Result<u32, String> {
    let value = value.trim();
    if let Some(hex) = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
    {
        u32::from_str_radix(hex, 16).map_err(|err| err.to_string())
    } else {
        value.parse::<u32>().map_err(|err| err.to_string())
    }
}

fn profile_name(profile: SdmProfile) -> &'static str {
    match profile {
        SdmProfile::Signaling => "signaling",
        SdmProfile::Radio => "radio",
        SdmProfile::Full => "full",
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
    fn parses_start_magic() {
        assert_eq!(parse_u32_auto("0x41414141").unwrap(), DEFAULT_START_MAGIC);
        assert_eq!(parse_u32_auto("1094795585").unwrap(), DEFAULT_START_MAGIC);
    }
}
