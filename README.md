# 6grok

6grok is a clean-room, MIT-licensed cellular diagnostic acquisition and analysis toolkit for 4G/5G and future 6G experimentation.

It is inspired by the publicly documented architecture of **5grok**, but is an independent project. The surviving MIT-licensed `5grok-parser` is used as the initial protocol-decoding library; acquisition, transport, API and UI components are being independently implemented.

## Current status

The first working slice provides:

- a Rust workspace suitable for Linux/OpenWrt cross-compilation;
- a pinned dependency on `mbound/5grok-parser`;
- a vendor-neutral capture frame type;
- passive Qualcomm DIAG serial acquisition;
- DIAG HDLC framing/unescaping and CRC-16 validation;
- extraction of Qualcomm `DIAG_LOG_F` frames;
- decoding through `fivegrok-parser` to JSON Lines;
- replay of captured raw DIAG byte streams.

```text
modem / phone
     |
     | DIAG / SDM / vendor trace
     v
+-------------+       +-------------+       +------------------+
| 6grok-agent | ----> | 6grok-core  | ----> | fivegrok-parser  |
+-------------+       +-------------+       +------------------+
       |                                             |
       | raw/vendor frames                           | decoded packets
       v                                             v
  capture/export                              JSON / API / UI
```

## Build

```bash
cargo build --workspace
cargo test --workspace
```

For a typical 64-bit ARM OpenWrt target using musl:

```bash
rustup target add aarch64-unknown-linux-musl
cargo build --release --target aarch64-unknown-linux-musl -p sixgrok-agent
```

The produced executable is named `6grok-agent`. The agent deliberately builds `serialport` without its `libudev` feature to keep the OpenWrt dependency surface small.

## Qualcomm passive capture

Connect to a modem DIAG serial port that is already producing log packets:

```bash
cargo run -p sixgrok-agent -- serial --port /dev/ttyUSB0
```

USB ACM/serial baud rate is commonly ignored by the modem, but can be set explicitly:

```bash
cargo run -p sixgrok-agent -- serial --port /dev/ttyUSB0 --baud 115200
```

Each decoded packet is emitted as one JSON object on stdout.

Replay a previously saved raw byte stream:

```bash
cargo run -p sixgrok-agent -- replay capture.bin
```

This initial implementation is **passive**: it does not yet install Qualcomm DIAG log masks. Active DIAG configuration is the next acquisition milestone.

## Planned acquisition backends

| Backend | Status | Implementation strategy |
|---|---|---|
| Qualcomm DIAG serial/USB | MVP | Original Rust implementation from public protocol facts |
| Qualcomm log-mask control | next | Clean-room implementation of documented DIAG commands |
| Qualcomm Android DIAG bridge | planned | Native/ADB transport, no GPL code copied |
| Samsung Shannon SDM | planned | Clean-room implementation informed by public SDM documentation and captures |
| MediaTek mdlogger/CCCI | planned | Clean-room or explicitly Apache-2.0-compatible implementation |
| AT monitor fallback | planned | Vendor-neutral AT/3GPP fallback for DIAG-locked modems |
| GPS/NMEA/gpsd | planned | Attach synchronized location metadata to frames |

Longer term the repository will add a server/API, WebSocket streaming, PCAP/GSMTAP export, metrics/time-series storage and a web dashboard.

See [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md).

## Licensing rule

6grok's original code is MIT licensed. We do **not** copy code from GPL projects such as QCSuper or SCAT into the MIT codebase. They may be used as protocol references or supported as optional external processes/interfaces.

Permissively licensed third-party code may be imported only when its original copyright, SPDX marker, license and any required NOTICE material are preserved. See [`THIRD_PARTY.md`](THIRD_PARTY.md).

## Acknowledgements / public references

The project builds on public work and protocol research including:

- `fivegrok-parser` — MIT parser extracted from the 5grok project;
- QCSuper — Qualcomm DIAG protocol research and documentation (GPL-3.0; reference only);
- SCAT — Qualcomm/Samsung diagnostic research and GSMTAP tooling (GPL-2.0-or-later; reference only);
- MobileInsight — cellular diagnostic research including MediaTek mdlogger interaction (Apache-2.0; reference or separately attributed reuse only);
- ShannonBaseband and related Samsung baseband research — only explicitly permissively licensed material may be reused.

No affiliation with the original 5grok authors is implied.
