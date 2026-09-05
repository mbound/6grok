# Third-party licensing and provenance

6grok is an MIT-licensed project. This file records which upstream projects may influence the implementation and what may or may not be copied into this repository.

## Policy

1. **Original 6grok source code is MIT.**
2. Third-party code is imported only when its license is compatible with distribution inside this project.
3. Imported files keep their original copyright/SPDX headers and remain under their original license where required. The repository's top-level MIT license does not erase those terms.
4. Required license and NOTICE texts are preserved under `LICENSES/` and/or adjacent to imported components.
5. GPL/AGPL code is not copied, translated line-for-line, or linked into the MIT 6grok binaries.
6. GPL tools may be supported as **separate external programs** over stable interfaces such as files, pipes, UDP/GSMTAP, sockets or subprocess invocation, without incorporating their source into 6grok.
7. LGPL/MPL/CDDL and other weak/file-level copyleft code requires an explicit licensing review before import.
8. Protocol facts, packet layouts, numeric constants and behavior learned from public documentation are implemented independently in original 6grok code. References are documented where useful.

## Current dependencies / references

| Project | License | Use in 6grok | Status |
|---|---|---|---|
| `mbound/5grok-parser` / `think-evil/5grok-parser` | MIT | Rust parser dependency pinned to commit `1d9099d5706a55f4624c8fb01c3a2a09fa5497ad` | **Allowed**; copyright/license preserved |
| QCSuper (`P1sec/QCSuper`) | GPL-3.0 | Qualcomm DIAG protocol/reference; possible external adapter | **Reference only**; no source copied |
| SCAT (`fgsect/scat`) | GPL-2.0-or-later | Qualcomm/Samsung protocol/reference; possible GSMTAP interop | **Reference only**; no source copied |
| MobileInsight | Apache-2.0 | MediaTek/Android diagnostic research; possible future selective reuse | **Permissive**, but imported files must retain Apache-2.0 notices and NOTICE obligations |
| ShannonBaseband (`grant-h/ShannonBaseband`) | Mixed; top-level project states MIT only for explicitly SPDX-marked files | Samsung/Shannon research | **File-by-file review required** |

## fivegrok-parser

The parser dependency is MIT licensed and states copyright:

> Copyright (c) 2024 5grok Contributors

A copy of its MIT license is stored in `LICENSES/fivegrok-parser-MIT.txt` for redistribution/provenance purposes.

## Dependency additions

Before adding or vendoring a dependency, record:

- upstream repository and immutable revision/tag;
- license SPDX identifier;
- copyright holder(s);
- whether code is linked, vendored, modified, or only used as a reference;
- location of the retained license/NOTICE text;
- any modifications made by 6grok.

When in doubt, do not import the code until the licensing status has been reviewed.
