# 6grok licensing architecture

6grok intentionally uses a **multi-license** source tree so that modem tooling with different compatible open-source licenses can be integrated without erasing upstream terms.

This document describes the repository policy; it is not a substitute for legal advice for a particular distribution or commercial product.

## Component model

| Component/material | License policy |
|---|---|
| `sixgrok-agent` combined application | `GPL-3.0-or-later` |
| Original reusable `sixgrok-core` | `MIT OR GPL-3.0-or-later` |
| Original reusable `sixgrok-api` | `MIT OR GPL-3.0-or-later` |
| Original source files identified as dual licensed by SPDX/REUSE metadata | `MIT OR GPL-3.0-or-later` |
| `fivegrok-parser` | upstream `MIT`, retained unchanged |
| QCSuper-derived source | upstream `GPL-3.0-or-later`, retained |
| SCAT-derived source | upstream `GPL-2.0-or-later`, retained |
| Apache/BSD/MIT imports | retain their exact upstream license and notices |

The repository root `LICENSE` contains the GNU GPL version 3 text because the complete 6grok agent/application distribution is GPL-covered once GPL-derived modem components are incorporated.

## Dual licensing does not relicense imports

`MIT OR GPL-3.0-or-later` applies only where the copyright holder has offered those choices. It does **not** convert an imported GPL, Apache, BSD or MIT file into another license.

For example:

```text
sixgrok-core source               MIT OR GPL-3.0-or-later
fivegrok-parser source            MIT
SCAT-derived Samsung module       GPL-2.0-or-later
QCSuper-derived Qualcomm module   GPL-3.0-or-later

              linked into sixgrok-agent
                         |
                         v
             combined distributed work
                  GPL-3.0-or-later
```

The original file-level licenses and notices remain applicable to those files inside the combined work.

## GPL version compatibility rule

SCAT declares `GPL-2.0-or-later`. The `or-later` grant permits use under GPLv3 terms when it is combined with a GPLv3 application.

QCSuper declares GPL-3.0+ / GPL-3.0-or-later and therefore fits the same combined GPLv3 application.

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
5. upstream copyright holder(s);
6. whether 6grok changed or translated the file;
7. date/summary of material modifications;
8. retained license and NOTICE location.

A typical adapted QCSuper-derived Rust module should carry a notice similar to:

```text
SPDX-License-Identifier: GPL-3.0-or-later
Derived from P1sec/QCSuper, commit <sha>, <upstream/path>
Upstream copyright: <preserved copyright notice>
Modified for 6grok: <date and short description>
```

A SCAT-derived file should continue to identify itself as `GPL-2.0-or-later`; it should not be relabeled GPLv3 merely because the complete executable is conveyed under compatible GPLv3 terms.

## Permissive reusable boundary

GPL-derived acquisition implementations belong in the GPL application side of the architecture, not in `sixgrok-core`.

`sixgrok-core` should contain neutral data models, wire formats, original decoders/utilities, and permissively licensed code only. This preserves its useful MIT reuse option.

A good dependency direction is therefore:

```text
              sixgrok-core
          MIT OR GPL-3.0-or-later
                    ^
                    |
      +-------------+-------------+
      |                           |
sixgrok-api                 sixgrok-agent
MIT OR GPL-3+                   GPL-3+
                                   |
                    +--------------+--------------+
                    |                             |
             QCSuper-derived                SCAT-derived
                GPL-3+                       GPL-2+
```

## Distribution obligations

When distributing a GPL-covered 6grok binary, make the corresponding source available in a GPL-compliant manner and retain applicable copyright/license/NOTICE material for bundled dependencies and imported files.

For releases, the preferred model is to publish the exact source revision, build scripts/configuration and retained license notices alongside binaries from the same release/tag.

If 6grok is embedded in a consumer product, GPLv3's installation-information provisions may also become relevant depending on how the product is distributed and controlled.

## Repository enforcement

- Cargo package metadata states the intended component license.
- `cargo-deny` rejects licenses outside the reviewed compatibility allowlist.
- `REUSE.toml` assigns licenses to current files without using a broad wildcard that could accidentally classify a future imported GPL file as MIT.
- `THIRD_PARTY.md` records project-level provenance and import requirements.
- Imported code must be reviewed file-by-file before merge.

## Adding a new upstream project

Do not decide compatibility from a README badge alone. Check the exact source revision and exact files to be imported. Record the result in `THIRD_PARTY.md`, preserve required notices, and update the license allowlist only if the architecture has been reviewed.
