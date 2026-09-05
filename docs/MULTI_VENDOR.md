# Multi-vendor capture interfaces

6grok separates **device transport** from **record decoding**. Qualcomm has native DIAG transports, Samsung now has a native Shannon SDM collector, and MediaTek currently has a record-level adapter while a native Android collector is developed from appropriate MediaTek-specific sources.

## Qualcomm profiles

Capture profiles are curated from log codes already present in the MIT-licensed `fivegrok-parser` metadata table.

```bash
6grok-agent serial --port /dev/ttyUSB0 --profile signaling
6grok-agent serial --port /dev/ttyUSB0 --profile radio
6grok-agent serial --port /dev/ttyUSB0 --profile full --server 10.0.0.2:5566
```

Profiles are capability-aware: before writing a mask, 6grok queries the modem's equipment-ID ranges. Profile codes beyond a modem's reported range are skipped with a warning. Explicit `--log` codes remain strict and fail if unsupported.

Rooted Qualcomm Android devices can also use the GPL QCSuper interoperability backend; see the repository README.

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

Native MediaTek `mdlogger`/CCCI acquisition is **not** sourced from SCAT: the pinned SCAT tree used by 6grok has Qualcomm/Samsung and other modem support but no MediaTek collector. Native MediaTek work should instead use MediaTek-specific public interfaces and compatible sources such as MobileInsight where applicable.

## Native Samsung Shannon SDM

`6grok-samsung-sdm` is a native collector for Samsung Shannon/Exynos SDM diagnostic streams, adapted from SCAT's GPL-2.0-or-later Samsung implementation at pinned commit `361ff551a4fbb30789c46750c00586682a7a9b26`.

The default device path is the commonly used:

```text
/dev/umts_dm0
```

Start a curated signaling capture:

```bash
6grok-samsung-sdm capture --device /dev/umts_dm0 --profile signaling
```

Radio-focused or combined selections are also available:

```bash
6grok-samsung-sdm capture --profile radio
6grok-samsung-sdm capture --profile full --server 10.0.0.2:5566
```

SCAT's default SDM start magic is retained (`0x41414141`) and can be overridden:

```bash
6grok-samsung-sdm capture --start-magic 0x41414141 --profile signaling
```

To attach to an SDM stream initialized by another process without writing selection commands:

```bash
6grok-samsung-sdm capture --passive
```

Request every item only when the log volume is acceptable:

```bash
6grok-samsung-sdm capture --all-items
```

Lossless native capture/replay is supported:

```bash
6grok-samsung-sdm capture --raw-capture shannon.sdm --frame-capture shannon.frames.jsonl
6grok-samsung-sdm replay shannon.sdm
```

Stop SDM collection explicitly:

```bash
6grok-samsung-sdm stop --device /dev/umts_dm0
```

### Native SDM framing

The collector follows SCAT's native SDM framing rather than the surviving parser's synthetic MIPC envelope:

```text
0x7f
u16_le length1
u8     zero
u16_le length2
u16_le stamp
u8     direction
u8     group_with_radio_id
u8     command
u32_le modem_timestamp
bytes  payload
0x7e
```

The decoder checks `length1 == length2 + 3`, bounds the packet size, requires the final `0x7e`, supports fragmented reads, and resynchronizes on the next `0x7f` after malformed data.

Native SDM packets are preserved intact in normalized frames under a dedicated synthetic namespace:

```text
0x2400 + SDM group
```

This intentionally does **not** reuse `0x2000..0x23ff`, because those are message-level synthetic IDs assigned by the surviving 5grok parser. Pretending a raw SDM group/command is one of those IDs would produce misleading NAS/RRC labels. Local output includes the actual SDM group, command, radio ID, direction, and modem timestamp while preserving the full wire packet for future semantic decoding.

## Legacy/synthetic Samsung records

The surviving parser separately understands a 16-byte MIPC-style envelope:

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

Existing record imports remain available:

```bash
6grok-agent records samsung.bin --format samsung --log 0x2150
6grok-agent records nr-rrc.bin --format raw --log 0x2150
```

These synthetic MIPC IDs and native SDM `0x24xx` frames are intentionally kept distinct.

For MediaTek raw PDUs use a parser-supported MediaTek code such as `0x1c01` or `0x1d01`.

## Normalized frame replay

Any vendor can be replayed once converted to `CaptureFrame` JSON Lines:

```bash
6grok-agent frames session.frames.jsonl --server 127.0.0.1:5566
```

This remains the stable integration boundary for external collectors as well as the native backends in this repository.

## Acquisition roadmap

1. Validate native Samsung Shannon SDM selection behavior across additional ICD generations and real hardware, while retaining lossless raw capture as the compatibility fallback.
2. Add native MediaTek Android `mdlogger`/CCCI and external-modem transports using MediaTek-specific public interfaces and appropriately licensed implementations (for example MobileInsight where applicable).
3. Add AT-command fallback for devices that expose measurements but lock diagnostic ports.
4. Extend semantic parsing of native SDM envelopes without conflating them with the historical synthetic MIPC namespace.

Until a transport or mapping is validated, 6grok keeps the raw envelope intact rather than guessing framing or silently producing incorrect decodes.
