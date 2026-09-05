# 6grok

6grok is an open cellular diagnostic acquisition and analysis toolkit for 4G/5G and future 6G experimentation.

It is inspired by the publicly documented architecture of **5grok** and uses the surviving MIT-licensed `5grok-parser` as its initial decoding library. The acquisition, transport and service layers are developed openly in this repository, with compatible upstream modem tooling reusable under the repository's multi-license policy.

## Current status

The working implementation currently provides:

- Rust workspace suitable for Linux/OpenWrt development;
- pinned `mbound/5grok-parser` dependency;
- vendor-neutral `CaptureFrame` representation;
- Qualcomm DIAG HDLC framing/unescaping and CRC validation;
- passive Qualcomm DIAG capture and raw replay;
- active Qualcomm `DIAG_LOG_CONFIG_F` capability probing and log-mask configuration;
- capability-aware `signaling`, `radio` and `full` Qualcomm profiles;
- normalized JSONL capture/replay;
- MediaTek 9-byte parser-record ingestion;
- Samsung MIPC-style and raw-PDU ingestion using the parser's synthetic namespaces;
- lightweight MessagePack/TCP agent uplink;
- `6grok-api` aggregation service;
- REST statistics/history and live WebSocket streaming;
- optional Qualcomm QCDIAG mirroring to Wireshark over GSMTAP.

```text
modem / phone
     |
     | DIAG / SDM / vendor trace
     v
+-------------+       +-------------+       +------------------+
| 6grok-agent | ----> | 6grok-core  | ----> | fivegrok-parser  |
+-------------+       +-------------+       +------------------+
       |                     |                       |
       | raw capture         | MessagePack           | decoded packets
       v                     v                       v
    replay              +-----------+          JSON / history
                        | 6grok-api |
                        +-----------+
                         /    |     \
                       REST   WS   GSMTAP -> Wireshark
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

The produced executable is named `6grok-agent`.

## Qualcomm capture

Passive capture from a modem that is already producing DIAG logs:

```bash
cargo run -p sixgrok-agent -- serial --port /dev/ttyUSB0
```

Probe the supported Qualcomm DIAG log equipment ranges:

```bash
cargo run -p sixgrok-agent -- probe --port /dev/ttyUSB0
```

Enable a parser-derived profile:

```bash
cargo run -p sixgrok-agent -- serial \
  --port /dev/ttyUSB0 \
  --profile signaling
```

Explicit log codes can also be requested with repeatable `--log 0x....` options. Explicit codes are strict; profile codes unsupported by a particular modem are skipped with a warning.

Save the exact incoming HDLC stream and normalized frames:

```bash
cargo run -p sixgrok-agent -- serial \
  --port /dev/ttyUSB0 \
  --profile signaling \
  --raw-capture capture.bin \
  --frame-capture frames.jsonl
```

Replay a raw Qualcomm capture:

```bash
cargo run -p sixgrok-agent -- replay capture.bin
```

## Service / remote agents

Start the aggregation service:

```bash
cargo run -p sixgrok-api
```

Send frames from an edge agent:

```bash
cargo run -p sixgrok-agent -- serial \
  --port /dev/ttyUSB0 \
  --profile signaling \
  --server 10.0.0.2:5566
```

Default service endpoints include:

- `GET /health`
- `GET /api/v1/stats`
- `GET /api/v1/packets`
- `GET /ws`

See [`docs/API.md`](docs/API.md).

## Wireshark

`6grok-api` can mirror received Qualcomm frames using the standardized GSMTAP QCDIAG type:

```bash
cargo run -p sixgrok-api -- --gsmtap 127.0.0.1:4729
```

See [`docs/WIRESHARK.md`](docs/WIRESHARK.md).

## MediaTek and Samsung

The current vendor boundary supports parser-compatible MediaTek records, Samsung MIPC-style records and extracted raw vendor PDUs. Native device-specific collection transports are being added behind this boundary rather than coupling them to parser internals.

```bash
cargo run -p sixgrok-agent -- records capture.bin --format mediatek
cargo run -p sixgrok-agent -- records capture.bin --format samsung
cargo run -p sixgrok-agent -- records pdu.bin --format raw --log 0x2060
```

See [`docs/MULTI_VENDOR.md`](docs/MULTI_VENDOR.md).

## Licensing

6grok intentionally uses a **multi-license architecture**.

- The combined `6grok-agent` application is `GPL-3.0-or-later`.
- Original reusable `sixgrok-core` and `sixgrok-api` code is available under `MIT OR GPL-3.0-or-later` where indicated by repository metadata.
- Third-party files retain their exact upstream license, copyright and notices.
- QCSuper (`GPL-3.0-or-later`) and SCAT (`GPL-2.0-or-later`) source may be reused/adapted in the GPL application with explicit provenance.
- MIT, Apache-2.0 and compatible BSD material may also be incorporated while retaining its original terms.
- `GPL-2.0-only`, AGPL and other licenses outside the reviewed compatibility policy are not imported into the combined application without explicit review.

The root [`LICENSE`](LICENSE) contains the GPLv3 license text. See [`docs/LICENSING.md`](docs/LICENSING.md) for the component model and [`THIRD_PARTY.md`](THIRD_PARTY.md) for import/provenance requirements.

`cargo-deny` and REUSE metadata are used to make license drift visible in CI.

## Next milestones

- native MediaTek mdlogger/CCCI acquisition;
- validated native Samsung Shannon SDM acquisition;
- additional Qualcomm Android/USB transports and QCSuper-derived capabilities;
- GPS/NMEA/gpsd synchronized location frames;
- AT-monitor fallback for DIAG-locked devices;
- persistent capture/database backend;
- web dashboard;
- reproducible OpenWrt/musl release packages and real-hardware regression captures.

## Acknowledgements

6grok builds on and interoperates with public work including `fivegrok-parser`, QCSuper, SCAT, MobileInsight, FirmWire, ShannonBaseband, Osmocom and Wireshark. Source reuse is governed by the exact upstream license of the material used and is recorded before merge.

No affiliation with the original 5grok authors is implied.
