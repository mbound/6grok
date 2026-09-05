# Multi-vendor capture interfaces

6grok separates **device transport** from **record decoding**. Qualcomm currently has a native serial DIAG transport. MediaTek and Samsung have record-level adapters so device-specific collectors can feed 6grok without being linked into the parser/API.

## Qualcomm profiles

Capture profiles are curated from log codes already present in the MIT-licensed `fivegrok-parser` metadata table.

```bash
6grok-agent serial --port /dev/ttyUSB0 --profile signaling
6grok-agent serial --port /dev/ttyUSB0 --profile radio
6grok-agent serial --port /dev/ttyUSB0 --profile full --server 10.0.0.2:5566
```

Profiles are capability-aware: before writing a mask, 6grok queries the modem's equipment-ID ranges. Profile codes beyond a modem's reported range are skipped with a warning. Explicit `--log` codes remain strict and fail if unsupported.

## MediaTek records

The current parser consumes records with its documented 9-byte MediaTek envelope:

```text
u16_le log_code
u32_le timestamp_ticks
u16_le payload_length
u8     direction       # 0 DL, non-zero UL
bytes  payload
```

Decode a record stream:

```bash
6grok-agent records mtk.bin --format mediatek
```

The adapter repeats records until EOF and can simultaneously write normalized JSONL frames and stream them to `6grok-api`.

## Samsung records

The surviving parser understands a 16-byte MIPC-style envelope:

```text
u32_le magic           # 0x4d495043 in parser metadata
u16_le command
u16_le payload_length
u64_le timestamp
bytes  payload
```

5grok assigned synthetic log-code families:

- `0x2000..0x20ff` NAS
- `0x2100..0x21ff` RRC
- `0x2200..0x22ff` ML1/PHY
- `0x2300..0x23ff` MAC/PDCP/RLC

If the MIPC command is already in that range, it is used as the synthetic code. Otherwise provide the intended parser code explicitly:

```bash
6grok-agent records samsung.bin --format samsung --log 0x2150
```

For extracted raw PDUs with no Samsung envelope:

```bash
6grok-agent records nr-rrc.bin --format raw --log 0x2150
```

For MediaTek raw PDUs use a parser-supported MediaTek code such as `0x1c01` or `0x1d01`.

## Normalized frame replay

Any vendor can be replayed once converted to `CaptureFrame` JSON Lines:

```bash
6grok-agent frames session.frames.jsonl --server 127.0.0.1:5566
```

This is also the stable integration boundary for external collectors. A GPL collector such as SCAT may write/export records that are then consumed by 6grok as a **separate program**; its source code is not incorporated into the MIT 6grok binary.

## Acquisition roadmap

Record-level support does not imply every device transport is solved. Planned native collectors are:

1. MediaTek Android `mdlogger`/CCCI and external-modem logging transports, implemented from public interfaces or permissively licensed sources.
2. Samsung Shannon `/dev/umts_dm*` / SDM transports, after validating framing and command mapping across modem generations.
3. AT-command fallback for devices that expose measurements but lock diagnostic ports.

Until a transport is validated on hardware, 6grok keeps it as an explicit record adapter rather than guessing framing and silently producing incorrect decodes.
