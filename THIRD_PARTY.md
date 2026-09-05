# Third-party licensing and provenance

6grok is a multi-license project. The combined 6grok application may include GPL-covered components, while original reusable components can remain available under permissive terms. This file records upstream provenance and the rules for importing code without erasing upstream rights or obligations.

## Repository licensing model

1. The complete `6grok-agent` application is distributed under **GPL-3.0-or-later**.
2. Original reusable 6grok files/components may be **MIT OR GPL-3.0-or-later** when their SPDX metadata says so.
3. Third-party files **retain their original license and copyright**. The top-level project license never silently relicenses imported material.
4. A combined executable containing GPL-covered code is distributed under compatible GPL terms even when constituent files are also available under permissive licenses.
5. `GPL-2.0-or-later` code may participate in a GPLv3 combined work by selecting GPLv3 terms; a separate GPL-2.0-or-later executable may also link dual-licensed 6grok libraries under their MIT option.
6. `GPL-2.0-only` code must not be linked or copied into the GPLv3 combined application without a different compatibility architecture.
7. `GPL-3.0-only` is rejected by default because it would remove the project's intended "or later" licensing option. Review explicitly if ever needed.
8. AGPL is not accepted by default. MPL/LGPL/CDDL and other weak/file-level copyleft licenses require explicit architecture and redistribution review before use.
9. Standard SPDX license texts are retained under `LICENSES/`; upstream-specific license/NOTICE copies may be retained under `THIRD_PARTY_LICENSES/` or alongside vendored material.
10. Protocol facts, packet layouts and numeric constants are not treated as source-code imports, but references should still be recorded when useful.

## Import checklist

Every copied or adapted third-party source file must record:

- upstream project and canonical repository;
- immutable upstream commit/tag;
- original upstream path;
- SPDX license identifier;
- original upstream copyright holder(s), where stated upstream;
- whether the file is copied verbatim or modified/translated;
- a short modification/provenance notice when changed;
- location of the corresponding license and NOTICE text.

Prefer an SPDX header in the imported file. Do not replace an upstream SPDX identifier with the repository's preferred license.

## Telecom projects

| Project | Upstream license | Permitted use in 6grok | Provenance requirement |
|---|---|---|---|
| `mbound/5grok-parser` / `think-evil/5grok-parser` | MIT | Linked parser dependency | Pinned commit + retained MIT text |
| QCSuper (`P1sec/QCSuper`) | GPL-3.0-or-later | Source reuse/adaptation in GPL application components | Preserve GPL/copyright; record exact commit/path/modifications |
| SCAT (`fgsect/scat`) | GPL-2.0-or-later | Source reuse/adaptation in compatible GPL components | Preserve GPL-2.0-or-later identity; record exact commit/path/modifications |
| MobileInsight | Apache-2.0 | Source reuse/adaptation allowed | Preserve Apache-2.0 notices and any upstream NOTICE obligations |
| FirmWire (`FirmWire/FirmWire`) | BSD-3-Clause | Source reuse/adaptation allowed where useful | Preserve BSD copyright/conditions/disclaimer |
| ShannonBaseband (`grant-h/ShannonBaseband`) | Mixed/file-specific | Only files with a clearly compatible license may be reused | File-by-file SPDX/license review required |
| Wireshark/libosmocore/Osmocom definitions | Various | Protocol facts and interoperability references; source import only after file-specific review | Record source/license if code is copied |

## fivegrok-parser

The parser dependency is pinned to commit:

`1d9099d5706a55f4624c8fb01c3a2a09fa5497ad`

It is MIT licensed and states copyright:

> Copyright (c) 2024 5grok Contributors

Its upstream-specific MIT copy remains preserved in `THIRD_PARTY_LICENSES/fivegrok-parser-MIT.txt`, while the standard MIT text used by REUSE is in `LICENSES/MIT.txt`. The parser is not relicensed by its use inside a GPL-covered executable.

## QCSuper

QCSuper is pinned for provenance at:

`aa555b4f7f25f7a8bf4e5afd4dcb884edf2f6735` (QCSuper 2.1.3, 2026-07-23)

QCSuper declares GPL-3.0+ / GPL-3.0-or-later. The `crates/6grok-qcsuper` integration is a GPL-3.0-or-later Rust interoperability backend for QCSuper's Android `/dev/diag` TCP bridge.

Upstream material used for that backend:

| Upstream path | 6grok use |
|---|---|
| `src/qcsuper/inputs/adb.py` | bridge address/transport behavior and HDLC-over-TCP interoperability |
| `src/qcsuper/inputs/adb_bridge/adb_bridge.c` | bridge framing/stream behavior and Android `/dev/diag` implementation reference |
| `src/qcsuper/modules/_enable_log_mixin.py` | translated/adapted signaling and IP/DPL capture selections |
| `src/qcsuper/inputs/_hdlc_mixin.py` | DIAG HDLC interoperability reference |

The bridge client itself is written in Rust for 6grok and carries GPL-3.0-or-later SPDX metadata. QCSuper-derived log selections explicitly record the upstream commit/path in source comments. The standard GPLv3 text is retained under `LICENSES/GPL-3.0-or-later.txt` and as the repository root `LICENSE`.

### Qualcomm log-mask semantics cross-check

QCSuper calls the range value a log-mask bit size in parts of its implementation, but Qualcomm DIAG sources and Osmocom model the protocol field as inclusive `last_item`. 6grok therefore deliberately retains an inclusive mask length of `floor(last_item / 8) + 1` bytes. This is covered by regression tests, including a boundary where `last_item == 8` and bit 8 occupies a second byte.

References used for this protocol cross-check include Qualcomm `diaglog.c` implementations and `osmocom/osmo-qcdiag/src/diag_log.c`; no Qualcomm source is copied into the dual-licensed core.

## SCAT / Samsung Shannon SDM

SCAT is pinned for provenance at:

`361ff551a4fbb30789c46750c00586682a7a9b26` (2026-09-03)

SCAT declares `GPL-2.0-or-later`. Its exact `COPYING` file at that commit has Git blob SHA `d159169d1050894d3ea3b98e1c965c4058208fe1`. That exact text is retained as `LICENSES/GPL-2.0-or-later.txt` and was hash-verified before commit.

The native Samsung collector is:

`crates/6grok-samsung-sdm`

and remains **GPL-2.0-or-later** at the crate/source level. It links `sixgrok-core` under the core's independent MIT grant, so the standalone collector does not require relabeling SCAT-derived files as GPLv3.

Source material adapted/translated from SCAT:

| Upstream path | 6grok use |
|---|---|
| `src/scat/parsers/samsung/sdmcmd.py` | SDM packet header, command/group constants, item-selection encoding, packet generation |
| `src/scat/parsers/samsung/samsungparser.py` | CONTROL_START/update-period/item-selection initialization and streaming frame extraction/resynchronization behavior |

The implementation preserves the native SDM wire packet rather than converting it into the surviving 5grok parser's unrelated synthetic MIPC envelope. Native frames use 6grok namespace `0x2400 + SDM group`; the historical parser-assigned `0x2000..0x23ff` message families remain untouched.

The collector validates both SDM length fields (`length1 == length2 + 3`), bounds packet length, requires `0x7e` termination, supports fragmented reads, resynchronizes on `0x7f`, and retains SCAT's default `CONTROL_START` magic `0x41414141` with an override option.

### MediaTek correction

The pinned SCAT tree used here does **not** provide the MediaTek `mdlogger`/CCCI collector previously contemplated in the roadmap. SCAT is therefore not the source for future native MediaTek integration. MediaTek work should use MediaTek-specific public interfaces and separately reviewed compatible sources, such as MobileInsight where applicable.

## Apache/BSD sources

Apache-2.0 and BSD-3-Clause sources can be included in a GPLv3 combined application while retaining their original licenses and notices. Apache NOTICE material, when present and applicable, must be propagated as required.

## Rust dependencies

`cargo-deny` checks the transitive Rust dependency graph against the reviewed allowlist. It currently allows permissive licenses plus `GPL-2.0-or-later` and `GPL-3.0-or-later`; strict GPLv2, AGPL and unreviewed weak-copyleft licenses remain rejected.

## New dependency or import review

Before adding a dependency or vendored source, answer all of the following:

1. What exact upstream revision are we using?
2. What is the license of the exact file/revision, not merely the repository homepage?
3. Is it compatible with the component it will enter and the final distributed work?
4. Which notices/source-offer obligations apply when distributing binaries?
5. Have original headers, copyright, license and NOTICE files been retained?
6. Have modifications been clearly identified?

If any answer is unclear, do not merge the import until it is resolved.
