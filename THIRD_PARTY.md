# Third-party licensing and provenance

6grok is a multi-license project. The combined 6grok application may include GPL-covered components, while original reusable components can remain available under permissive terms. This file records upstream provenance and the rules for importing code without erasing upstream rights or obligations.

## Repository licensing model

1. The complete `6grok-agent` application is distributed under **GPL-3.0-or-later**.
2. Original reusable 6grok files/components may be **MIT OR GPL-3.0-or-later** when their SPDX metadata says so.
3. Third-party files **retain their original license and copyright**. The top-level project license never silently relicenses imported material.
4. A combined executable containing GPL-covered code is distributed under compatible GPL terms even when constituent files are also available under permissive licenses.
5. `GPL-2.0-or-later` code may participate in the GPLv3 application by selecting GPLv3 terms. `GPL-2.0-only` code must not be linked or copied into that combined work.
6. `GPL-3.0-only` is rejected by default because it would remove the project's intended "or later" licensing option. Review explicitly if ever needed.
7. AGPL is not accepted by default. MPL/LGPL/CDDL and other weak/file-level copyleft licenses require explicit architecture and redistribution review before use.
8. Standard SPDX license texts are retained under `LICENSES/`; upstream-specific license/NOTICE copies may be retained under `THIRD_PARTY_LICENSES/` or alongside vendored material.
9. Protocol facts, packet layouts and numeric constants are not treated as source-code imports, but references should still be recorded when useful.

## Import checklist

Every copied or adapted third-party source file must record:

- upstream project and canonical repository;
- immutable upstream commit/tag;
- original upstream path;
- SPDX license identifier;
- original upstream copyright holder(s);
- whether the file is copied verbatim or modified;
- a short modification/provenance notice when changed;
- location of the corresponding license and NOTICE text.

Prefer an SPDX header in the imported file. Do not replace an upstream SPDX identifier with the repository's preferred license.

## Telecom projects

| Project | Upstream license | Permitted use in 6grok | Provenance requirement |
|---|---|---|---|
| `mbound/5grok-parser` / `think-evil/5grok-parser` | MIT | Linked parser dependency | Pinned commit + retained MIT text |
| QCSuper (`P1sec/QCSuper`) | GPL-3.0-or-later | **Source reuse/adaptation allowed in GPL application** | Preserve GPL/copyright; record exact commit/path/modifications |
| SCAT (`fgsect/scat`) | GPL-2.0-or-later | **Source reuse/adaptation allowed in GPLv3 application** by choosing GPLv3 terms for the combined work | Preserve original GPL-2.0-or-later notice/copyright; record commit/path/modifications |
| MobileInsight | Apache-2.0 | Source reuse/adaptation allowed | Preserve Apache-2.0 notices and any upstream NOTICE obligations |
| FirmWire (`FirmWire/FirmWire`) | BSD-3-Clause | Source reuse/adaptation allowed where useful | Preserve BSD copyright/conditions/disclaimer |
| ShannonBaseband (`grant-h/ShannonBaseband`) | Mixed/file-specific | Only files with a clearly compatible license may be reused | File-by-file SPDX/license review required |
| Wireshark/libosmocore/Osmocom definitions | Various | Protocol facts and interoperability references; source import only after file-specific review | Record source/license if code is copied |

## fivegrok-parser

The parser dependency is pinned to commit:

`1d9099d5706a55f4624c8fb01c3a2a09fa5497ad`

It is MIT licensed and states copyright:

> Copyright (c) 2024 5grok Contributors

Its upstream-specific MIT copy remains preserved in `THIRD_PARTY_LICENSES/fivegrok-parser-MIT.txt`, while the standard MIT text used by REUSE is in `LICENSES/MIT.txt`. The parser is not relicensed by its use inside a GPL-covered 6grok executable.

## QCSuper

QCSuper declares GPL-3.0+ / GPL-3.0-or-later. Its code may now be copied or adapted into GPL-covered 6grok application components. When this is done, keep upstream copyright/license information and add a provenance comment identifying the upstream commit and source path.

Do not move QCSuper-derived code into a component advertised as MIT-only. If functionality needs to be shared with a permissive library, isolate an independently written interface/data model from the GPL-derived implementation.

## SCAT

SCAT declares `GPL-2.0-or-later`. This is compatible with the GPLv3 6grok application because the "or later" grant permits selecting GPLv3 terms for the combined work.

SCAT-derived files must retain their `GPL-2.0-or-later` identity and copyright. Do not rewrite their file-level SPDX identifier to GPL-3.0 merely because the combined binary is distributed under GPLv3 terms.

## Apache/BSD sources

Apache-2.0 and BSD-3-Clause sources can be included in the GPLv3 combined application while retaining their original licenses and notices. Apache NOTICE material, when present and applicable, must be propagated as required.

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
