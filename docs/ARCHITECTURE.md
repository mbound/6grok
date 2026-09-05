# 6grok architecture

## Goal

Reconstruct the useful public behavior described for 5grok as an independent, maintainable cellular protocol analysis stack, while extending acquisition coverage using legally compatible public research.

The design intentionally separates **acquisition** from **decoding**. Vendor transports are messy and device-specific; protocol decoding and higher-layer analysis should not depend on how bytes arrived.

## Components

### `6grok-agent`

Small edge collector intended to run on Linux, OpenWrt routers, SBCs or engineering hosts.

Responsibilities:

- identify/open modem diagnostic transports;
- frame and validate vendor diagnostic traffic;
- preserve raw packets and timing;
- attach vendor identity and later GPS/location metadata;
- forward normalized frames or decode locally;
- eventually configure logging masks safely and restore modem state on exit.

Target: static/minimal `aarch64-unknown-linux-musl` builds where practical.

### `6grok-core`

Vendor-neutral types and normalization boundary.

A normalized frame carries:

```text
sequence
timestamp_wall
timestamp_mono
vendor
log_code
payload
```

The payload representation deliberately matches the surviving 5grok parser convention: a little-endian 16-bit log code followed by the raw vendor packet.

### `fivegrok-parser`

MIT-licensed parser dependency. It already contains decoders/metadata for Qualcomm, MediaTek and Samsung and covers NR/LTE RRC/NAS plus a useful set of ML1/MAC/PDCP/RLC/PHY messages.

The dependency is pinned to an immutable commit. 6grok does not assume every parser heuristic is correct; captures and regression tests will be used to improve it.

### Future `6grok-api`

Server-side ingestion and query layer:

- agent registration/health;
- raw + decoded frame ingestion;
- WebSocket/SSE live stream;
- filtering by RAT/layer/log code/UE/session;
- metrics aggregation;
- capture management;
- PCAP/GSMTAP export.

### Future dashboard

Web UI for:

- serving cell and neighbors;
- RSRP/RSRQ/SINR/CQI/MCS/BLER;
- NR beams/SSB;
- NAS/RRC state and message timeline;
- handover/reselection events;
- MAC/RLC/PDCP statistics;
- GPS map + radio measurements;
- raw packet/detail inspector.

## Qualcomm acquisition

The initial backend accepts the classic DIAG HDLC byte stream.

Publicly documented wire properties used by the clean-room implementation:

- frame trailer `0x7e`;
- escape byte `0x7d`, with escaped byte XOR `0x20`;
- CCITT CRC-16 / X.25-style FCS;
- DIAG log response command `0x10`;
- the surviving parser expects the unescaped DIAG packet with FCS removed.

The MVP is passive. Planned active support includes `DIAG_LOG_CONFIG_F` capabilities/range discovery and log-mask installation, implemented independently from protocol documentation and verified captures.

## Samsung Shannon acquisition

The parser already includes Samsung envelope handling, but acquisition is not yet implemented.

Plan:

1. document SDM framing and USB/serial session setup from public research and observed captures;
2. implement framing/session code independently;
3. validate against `.sdm` files and physical Exynos/Shannon devices;
4. optionally interoperate with SCAT through GSMTAP or capture files without incorporating SCAT's GPL source.

`ShannonBaseband` may be used only on a file-by-file basis where an explicit permissive SPDX license applies.

## MediaTek acquisition

Public MobileInsight research demonstrates interaction with Android MediaTek mdlogger via abstract Unix sockets such as `com.mediatek.mdlogger.socket` / `socket1` and commands such as starting/pausing deep logging. MobileInsight is Apache-2.0, but the initial 6grok implementation will still be clean-room unless importing a specific file provides a clear benefit.

Plan:

- Android mdlogger control adapter;
- CCCI/MD log file/stream ingestion;
- parser-envelope normalization;
- serial/module-specific variants where available.

Any Apache-derived code will retain its original Apache-2.0 header/license and required notices rather than being relabeled MIT.

## AT fallback

For devices where diagnostic access is locked, a separate AT monitor will collect standardized and vendor-specific information (registration, serving cell, radio measurements, bands, temperature where exposed). This is lower fidelity than raw DIAG/SDM but useful for broad hardware support.

## Interoperability rather than copying

GPL tooling remains useful:

- QCSuper can act as a behavioral/protocol reference and potentially an external capture producer;
- SCAT can be consumed over GSMTAP/PCAP or invoked as a separate external program.

Keeping these interfaces process-separated avoids contaminating the MIT implementation while allowing users who choose to install GPL tools to combine them operationally.

## Testing strategy

Every acquisition backend should have:

- byte-level framing tests;
- malformed/truncated input tests;
- fixture captures with provenance/license documented;
- decoder regression tests;
- round-trip tests for any framing encoder;
- hardware compatibility notes.

Captured user/network data must never be committed without explicit sanitization and permission.
