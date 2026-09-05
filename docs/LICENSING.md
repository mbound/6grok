# 6grok licensing architecture

6grok intentionally uses a **multi-license** source tree so that modem tooling with different compatible open-source licenses can be integrated without erasing upstream terms.

This document describes the repository policy; it is not a substitute for legal advice for a particular distribution or commercial product.

## Component model

| Component/material | License policy |
|---|---|
| `sixgrok-agent` application | `GPL-3.0-or-later` |
| `sixgrok-qcsuper` QCSuper interoperability backend | `GPL-3.0-or-later` |
| `sixgrok-samsung-sdm` SCAT-derived Shannon collector | `GPL-2.0-or-later` |
| Original reusable `sixgrok-core` | `MIT OR GPL-3.0-or-later` |
| Original reusable `sixgrok-api` | `MIT OR GPL-3.0-or-later` |
| Original source files identified as dual licensed by SPDX/REUSE metadata | `MIT OR GPL-3.0-or-later` |
| `fivegrok-parser` | upstream `MIT`, retained unchanged |
| QCSuper-derived source | upstream `GPL-3.0-or-later`, retained |
| SCAT-derived source | upstream `GPL-2.0-or-later`, retained |
| Apache/BSD/MIT imports | retain their exact upstream license and notices |

The repository root `LICENSE` contains GNU GPL version 3 because the principal 6grok agent/application distribution is GPL-3.0-or-later. File/component-specific terms remain authoritative where explicitly assigned, and standard GPLv2-or-later material is also retained under `LICENSES/GPL-2.0-or-later.txt`.

## Dual licensing does not relicense imports

`MIT OR GPL-3.0-or-later` applies only where the copyright holder has offered those choices. It does **not** convert an imported GPL, Apache, BSD or MIT file into another license.

The architecture supports both of these legitimate combinations:

```text
sixgrok-core (MIT OR GPL-3+)       sixgrok-core (MIT OR GPL-3+)
        |                                  |
        | choose GPL-3+                    | choose MIT
        v                                  v
sixgrok-qcsuper / agent             sixgrok-samsung-sdm
      GPL-3+                            GPL-2+
```

A SCAT-derived file continues to identify itself as `GPL-2.0-or-later`. It is not relabeled GPLv3 merely because GPL-2.0-or-later could alternatively be used under GPLv3 terms in another combined work.

## GPL version compatibility rule

SCAT declares `GPL-2.0-or-later`. The `or-later` grant permits either:

- keeping a standalone SCAT-derived component under `GPL-2.0-or-later` while linking separately dual-licensed libraries under a compatible permissive grant; or
- selecting GPLv3 terms when SCAT-derived material is actually combined into a GPLv3 work.

QCSuper declares GPL-3.0+ / GPL-3.0-or-later and fits the GPLv3 application side directly.

The following are **not accepted automatically**:

- `GPL-2.0-only`: do not link or copy into the GPLv3 combined application;
- `GPL-3.0-only`: requires explicit review because it removes 6grok's intended `or-later` option;
- AGPL: requires explicit review because of its additional network-interaction obligations;
- LGPL/MPL/CDDL and other weak/file-level copyleft: may be usable, but the component and binary-distribution architecture must be reviewed before inclusion.

## Source imports

Every copied or translated/adapted upstream source file must retain or add enough provenance to identify:

1. canonical upstream repository;
2. immutable upstream commit/tag;
3. original source path;
4. upstream SPDX/license;
5. upstream copyright holder(s), where stated upstream;
6. whether 6grok changed or translated the file;
7. date/summary of material modifications;
8. retained license and NOTICE location.

A typical adapted QCSuper-derived Rust module should state the GPL-3.0-or-later SPDX identifier, the pinned upstream repository/commit/path, and the date/summary of the 6grok modification.

A SCAT-derived module uses the SPDX license value `GPL-2.0-or-later` and records the pinned SCAT commit/source paths. The exact SCAT GPLv2 text used by this repository is hash-verified and retained under `LICENSES/GPL-2.0-or-later.txt`.

## Permissive reusable boundary

GPL-derived acquisition implementations belong in GPL components, not in `sixgrok-core`.

`sixgrok-core` contains neutral data models, wire formats, original decoders/utilities, and permissively/dual-licensed code. This preserves its MIT reuse option and allows a GPL-2.0-or-later standalone collector to consume the common `CaptureFrame`/MessagePack model under MIT without a license-version conflict.

```text
                         sixgrok-core
                    MIT OR GPL-3.0-or-later
                   /          |           \
             choose MIT   choose GPL3   choose GPL3
                /             |             \
   samsung-sdm GPL-2+     agent GPL-3+   qcsuper GPL-3+
```

## Distribution obligations

When distributing a GPL-covered 6grok binary, make the corresponding source available in a GPL-compliant manner and retain applicable copyright/license/NOTICE material for bundled dependencies and imported files.

For releases, the preferred model is to publish the exact source revision, build scripts/configuration and retained license notices alongside binaries from the same release/tag.

GPLv3 installation-information provisions can become relevant for GPLv3-covered software distributed in a qualifying User Product. Evaluate the obligations of the **actual component/license being conveyed**, rather than assuming every executable in the repository has the same GPL version.

## Repository enforcement

- Cargo package metadata states the intended component license.
- `cargo-deny` rejects licenses outside the reviewed compatibility allowlist.
- `REUSE.toml` assigns licenses to current files without using a broad wildcard that could accidentally classify a future imported GPL file as MIT.
- `THIRD_PARTY.md` records project-level provenance and import requirements.
- Imported code must be reviewed file-by-file before merge.

## Adding a new upstream project

Do not decide compatibility from a README badge alone. Check the exact source revision and exact files to be imported. Record the result in `THIRD_PARTY.md`, preserve required notices, and update the license allowlist only if the architecture has been reviewed.
