# Wireshark / GSMTAP integration

6grok can mirror normalized Qualcomm frames from the aggregation service to Wireshark using the canonical GSMTAP v2 `QC_DIAG` packet type.

Start Wireshark listening for GSMTAP/UDP and run:

```bash
6grok-api \
  --ingest 0.0.0.0:5566 \
  --http 0.0.0.0:8080 \
  --gsmtap 127.0.0.1:4729
```

Remote 6grok agents continue using the normal MessagePack service uplink:

```bash
6grok-agent serial \
  --port /dev/ttyUSB0 \
  --profile signaling \
  --server SERVER:5566
```

The API removes 6grok's two-byte normalized log-code prefix and encapsulates the original de-framed Qualcomm DIAG packet in a 16-byte GSMTAP v2 header.

## Protocol facts

The exporter is an original MIT implementation of public GSMTAP wire facts. The canonical Wireshark/Osmocom definition assigns:

- GSMTAP version 2
- header length 4 x 32-bit words (16 bytes)
- UDP port 4729 by convention
- packet type `0x11` = `GSMTAP_TYPE_QC_DIAG`

Wireshark added/expanded an actual QCDIAG dissector using that existing GSMTAP type in 2026. See:

- https://www.wireshark.org/docs/wsar_html/packet-gsmtap_8h_source.html
- https://gitlab.com/wireshark/wireshark/-/merge_requests/23388

Wireshark and libosmocore source code are GPL; none of that implementation source is incorporated into 6grok.

## 5G/NR note

GSMTAP's canonical type table currently defines LTE RRC/MAC/NAS and Qualcomm DIAG types but does not define a generic NR/5G RRC/NAS type. 6grok therefore does **not** invent an incompatible NR type. Qualcomm NR records are exported as `QC_DIAG`, allowing Wireshark's QCDIAG layer to decode whatever log codes it supports while retaining the raw frame for unsupported codes.
