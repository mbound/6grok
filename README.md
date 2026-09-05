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
- GPL QCSuper interoperability backend for rooted Android `/dev/diag` through QCSuper `adb_bridge`;
- QCSuper-derived signaling and IP/DPL capture profiles with pinned provenance;
- native Samsung Shannon/Exynos SDM capture through `/dev/umts_dm0`, adapted from pinned SCAT source;
- lossless native SDM raw capture/replay plus `signaling`, `radio`, `full`, passive and all-item modes;
- normalized JSONL capture/replay;
- MediaTek 9-byte parser-record ingestion;
- legacy Samsung MIPC-style and raw-PDU ingestion using the parser's historical synthetic namespaces;
- lightweight MessagePack/TCP agent uplink;
- `6grok-api` aggregation service;
- REST statistics/history and live WebSocket streaming;
- optional Qualcomm QCDIAG mirroring to Wireshark over GSMTAP.

```text
modem / phone
     |
     | DIAG / QCSuper bridge / Shannon SDM / vendor trace
     v
+--------------------+     +-------------+       +------------------+
| 6grok-agent /      | --> | 6grok-core  | ----> | fivegrok-parser  |
| 6grok-qcsuper /    |     +-------------+       +------------------+
| 6grok-samsung-sdm  |            |                       |
+--------------------+            | MessagePack           | decoded packets
         |                        v                       v
         v                   +-----------+          JSON / history
    raw / JSONL              | 6grok-api |
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

Primary edge executables are `6grok-agent`, `6grok-qcsuper`, and `6grok-samsung-sdm`.

## Qualcomm serial/USB capture

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

## Rooted Android via QCSuper

`6grok-qcsuper` interoperates with the TCP endpoint created by QCSuper's GPL `adb_bridge`. This is useful on Qualcomm Android devices where `/dev/diag` requires the diagchar setup logic already implemented and tested by QCSuper.

QCSuper's default bridge port is TCP 43555. Once its bridge is running and forwarded by ADB:

```bash
cargo run -p sixgrok-qcsuper -- probe
cargo run -p sixgrok-qcsuper -- capture --profile signaling
cargo run -p sixgrok-qcsuper -- capture --profile ip
cargo run -p sixgrok-qcsuper -- capture --profile full --server 10.0.0.2:5566
```

The backend does not vendor QCSuper's Android executable. QCSuper remains the source of the on-device `/dev/diag` bridge; 6grok speaks its HDLC-over-TCP interface and performs DIAG log configuration itself. The integration is pinned to QCSuper commit `aa555b4f7f25f7a8bf4e5afd4dcb884edf2f6735` and its source-level provenance is recorded in [`THIRD_PARTY.md`](THIRD_PARTY.md).

## Samsung Shannon SDM

`6grok-samsung-sdm` is a native Samsung Shannon/Exynos diagnostic collector adapted from SCAT's Samsung SDM implementation at pinned commit `361ff551a4fbb30789c46750c00586682a7a9b26`.

On devices exposing the conventional Shannon diagnostic node:

```bash
cargo run -p sixgrok-samsung-sdm -- capture --device /dev/umts_dm0 --profile signaling
```

Other useful modes:

```bash
cargo run -p sixgrok-samsung-sdm -- capture --profile radio
cargo run -p sixgrok-samsung-sdm -- capture --profile full --server 10.0.0.2:5566
cargo run -p sixgrok-samsung-sdm -- capture --passive
cargo run -p sixgrok-samsung-sdm -- capture --all-items
```

Lossless native SDM capture and replay:

```bash
cargo run -p sixgrok-samsung-sdm -- capture \
  --raw-capture shannon.sdm \
  --frame-capture shannon.frames.jsonl
cargo run -p sixgrok-samsung-sdm -- replay shannon.sdm
```

Stop an initialized SDM stream:

```bash
cargo run -p sixgrok-samsung-sdm -- stop --device /dev/umts_dm0
```

Native SDM is kept distinct from the surviving parser's historical synthetic Samsung IDs. Full SDM packets are preserved under `0x2400 + group`, with actual direction/radio-ID/group/command/timestamp exposed in local JSON. This avoids inventing NAS/RRC labels before a native SDM message has been semantically mapped.

See [`docs/MULTI_VENDOR.md`](docs/MULTI_VENDOR.md) for framing and namespace details.

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

## MediaTek and imported records

The current generic vendor boundary supports parser-compatible MediaTek records, historical Samsung MIPC-style records and extracted raw vendor PDUs:

```bash
cargo run -p sixgrok-agent -- records capture.bin --format mediatek
cargo run -p sixgrok-agent -- records capture.bin --format samsung
cargo run -p sixgrok-agent -- records pdu.bin --format raw --log 0x2060
```

Native MediaTek collection is still a separate milestone. The pinned SCAT tree does not provide a MediaTek collector, so future `mdlogger`/CCCI work will use MediaTek-specific interfaces and separately reviewed compatible sources such as MobileInsight where applicable.

See [`docs/MULTI_VENDOR.md`](docs/MULTI_VENDOR.md).

## Licensing

6grok intentionally uses a **multi-license architecture**.

- `6grok-agent` and `6grok-qcsuper` are `GPL-3.0-or-later`.
- The SCAT-derived `6grok-samsung-sdm` collector remains `GPL-2.0-or-later` and links reusable 6grok code under its independent MIT option.
- Original reusable `sixgrok-core` and `sixgrok-api` code is available under `MIT OR GPL-3.0-or-later` where indicated by repository metadata.
- Third-party files retain their exact upstream license, copyright and notices.
- QCSuper (`GPL-3.0-or-later`) and SCAT (`GPL-2.0-or-later`) source reuse is tracked with immutable upstream revision/path provenance.
- MIT, Apache-2.0 and compatible BSD material may also be incorporated while retaining its original terms.
- `GPL-2.0-only`, AGPL and other licenses outside the reviewed compatibility policy are not imported into incompatible combined components without explicit review.

The root [`LICENSE`](LICENSE) contains GPLv3 for the principal GPLv3 application distribution. Standard SPDX texts for both GPLv3 and GPLv2-or-later material are retained under [`LICENSES/`](LICENSES/). See [`docs/LICENSING.md`](docs/LICENSING.md) and [`THIRD_PARTY.md`](THIRD_PARTY.md).

`cargo-deny` and REUSE metadata make license drift visible in CI.

## Next milestones

- hardware validation of native Shannon SDM across more modem/ICD generations;
- semantic native-SDM parsing while preserving the raw envelope and avoiding synthetic-ID conflation;
- native MediaTek `mdlogger`/CCCI acquisition from MediaTek-specific compatible sources;
- direct Android diagchar backend where it adds value beyond QCSuper bridge interoperability;
- GPS/NMEA/gpsd synchronized location frames;
- AT-monitor fallback for DIAG-locked devices;
- persistent capture/database backend;
- web dashboard;
- reproducible OpenWrt/musl release packages and real-hardware regression captures.

## Acknowledgements

6grok builds on and interoperates with public work including `fivegrok-parser`, QCSuper, SCAT, MobileInsight, FirmWire, ShannonBaseband, Osmocom and Wireshark. Source reuse is governed by the exact upstream license of the material used and is recorded before merge.

No affiliation with the original 5grok authors is implied.
