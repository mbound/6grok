# Qualcomm active DIAG control

6grok can operate passively or explicitly configure Qualcomm DIAG log masks.

Passive capture is the default: if no `--log` arguments are supplied, the agent does not alter modem log masks.

## Probe supported equipment-ID ranges

```bash
6grok-agent probe --port /dev/ttyUSB0
```

The agent sends `DIAG_LOG_CONFIG_F / RETRIEVE_ID_RANGES` and prints the modem-reported `last_item` for each of the 16 equipment IDs.

## Enable selected log codes

```bash
6grok-agent serial \
  --port /dev/ttyUSB0 \
  --log 0xb821 \
  --log 0xb0c2 \
  --raw-capture session.diag \
  --frame-capture session.frames.jsonl
```

Each 16-bit Qualcomm log code is split into its four-bit equipment ID and twelve-bit item ID. 6grok queries the modem's equipment-ID ranges, builds one bit mask per affected equipment ID, validates requested item IDs against the modem's reported range, sends `SET_MASK`, and checks the response status.

`--raw-capture` stores the exact incoming HDLC byte stream and can be replayed later:

```bash
6grok-agent replay session.diag
```

`--frame-capture` stores normalized `CaptureFrame` objects as JSON Lines independently of decoded stdout output.

## Disable logging

```bash
6grok-agent disable --port /dev/ttyUSB0
```

This sends `LOG_CONFIG_DISABLE_OP` explicitly. It is intentionally not performed automatically because diagnostic ports may be shared with another tool and globally disabling logging could interfere with that tool.

## Safety and coexistence

`SET_MASK` replaces the mask for an equipment ID on many Qualcomm implementations. For that reason:

- 6grok never changes masks merely by opening a serial port;
- active configuration requires explicit `--log` arguments;
- the modem-reported range is queried before any mask is written;
- response status is checked;
- 6grok does not currently claim transparent mask restoration when sharing a DIAG endpoint with QXDM/QCAT or another active controller.

Future mask-coexistence work will only be enabled after current-mask behavior is validated across modem generations.

## Clean-room implementation

HDLC framing and `DIAG_LOG_CONFIG_F` packet handling are original MIT-licensed Rust implementations based on public protocol facts. GPL diagnostic projects may be used for behavioral comparison but their source is not incorporated into 6grok. See `THIRD_PARTY.md` and `docs/PROTOCOL_REFERENCES.md`.
