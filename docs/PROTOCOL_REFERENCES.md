# Protocol reference policy

6grok implements cellular diagnostic protocols independently from publicly documented wire behavior. This file records public references used to validate protocol facts. It is not a source-code provenance list; code provenance is tracked in `THIRD_PARTY.md`.

## Qualcomm DIAG

Publicly documented facts used by the MIT implementation include:

- HDLC-like framing with `0x7e` delimiter, `0x7d` escaping and CCITT/X.25 CRC-16.
- `DIAG_LOG_F = 0x10` for log records.
- `DIAG_LOG_CONFIG_F = 0x73` for log-mask configuration.
- Log-config operations: disable (0), retrieve equipment-ID ranges (1), retrieve valid mask (2), set mask (3), get log mask (4).
- Log codes contain a 4-bit equipment ID and a 12-bit item ID.
- `RETRIEVE_ID_RANGES` responses expose 16 `last_item` values, one for each equipment ID.
- `SET_MASK` carries equipment ID, last item and a bit mask indexed by item ID.

Reference implementations and public headers consulted include Android/Qualcomm-derived diagnostic headers, Android kernel diag mask handling, Quectel's public QLog headers, Osmocom-derived documentation and GPL tools such as QCSuper/SCAT. GPL source is not copied into 6grok.

Where a protocol fact is ambiguous across modem generations, 6grok prefers conservative behavior and exposes raw packet capture for validation.
