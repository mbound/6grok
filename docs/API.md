# 6grok service API and agent wire protocol

## Processes

- `6grok-agent` runs close to a modem (for example on OpenWrt or a test laptop).
- `6grok-api` receives normalized frames, decodes them through `fivegrok-parser`, retains a bounded in-memory history and exposes REST/WebSocket clients.

Default listeners:

- agent ingest: TCP `0.0.0.0:5566`
- HTTP/WebSocket: TCP `0.0.0.0:8080`

## Agent MessagePack wire format

The MessagePack payload intentionally follows the public 5grok documentation:

```text
["Frame", [sequence, timestamp_wall, timestamp_mono, log_code, payload, vendor]]
```

Fields:

| field | type | meaning |
|---|---|---|
| `sequence` | u64 | monotonically increasing agent frame sequence |
| `timestamp_wall` | i64 | Unix epoch milliseconds |
| `timestamp_mono` | u64 | milliseconds since acquisition process start |
| `log_code` | u16 | normalized diagnostic log code |
| `payload` | bytes | parser-compatible payload (`log_code_le || raw_vendor_packet`) |
| `vendor` | u8 | 0 Qualcomm, 1 MediaTek, 2 Samsung |

TCP framing adds an outer unsigned 32-bit **big-endian** payload length:

```text
+----------------------+-------------------------------+
| 4-byte BE length N   | N bytes MessagePack payload   |
+----------------------+-------------------------------+
```

The API currently rejects zero-length messages and messages larger than 8 MiB.

Example agent use:

```bash
6grok-agent serial \
  --port /dev/ttyUSB0 \
  --server 192.0.2.10:5566
```

Replay can also feed the service:

```bash
6grok-agent replay session.diag --server 127.0.0.1:5566
```

## REST

### `GET /health`

```json
{"status":"ok"}
```

### `GET /api/v1/stats`

Returns service start/uptime, total received and fully decoded counts, decode ratio, active WebSocket subscribers, history length and counters grouped by vendor, RAT and protocol layer.

### `GET /api/v1/packets?limit=100`

Returns the most recent decoded packets in chronological order. `limit` is clamped to 1..1000. The service-wide retained history size defaults to 5000 and is configurable with `6grok-api --history N`.

## WebSocket

### `GET /ws`

Each WebSocket text message is one decoded packet serialized as JSON. Slow clients that overrun the broadcast ring receive a control message:

```json
{"type":"lagged","skipped":123}
```

## Starting the server

```bash
cargo run -p sixgrok-api -- \
  --ingest 0.0.0.0:5566 \
  --http 0.0.0.0:8080 \
  --history 5000
```

The current service is deliberately stateless except for bounded RAM history. Persistent indexed storage will be added behind a storage trait so edge and lab deployments are not forced to run a database.
