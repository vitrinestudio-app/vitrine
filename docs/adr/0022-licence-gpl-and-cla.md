# 0022 – Licence: GPL-3.0-or-later with a contributor licence agreement

- Status: accepted
- Date: 2026-11-11

## Context

KONZEPT §10.4/§13 left the licence open (MIT OR Apache-2.0 vs. copyleft).
Until now the repository had no `LICENSE`, no `license` field, and the
Swedish hyphenation patterns (LPPL 1.2) waited for the decision. The owner
(Looking4Cache UG (haftungsbeschränkt), GitHub `Looking4Cache`) wants forks
to stay open, but keep the option of other licences later (e.g. a
commercial edition, store-distribution exceptions, relicensing).

## Decision

**Vitrine Studio's own code is licensed `GPL-3.0-or-later`; contributions
require an individual contributor licence agreement (CLA) of the licence
type.** Decided with the user on 2026-11-11.

- `LICENSE` is the verbatim GNU GPL v3 text (gnu.org `gpl-3.0.txt`, 35 149
  bytes, SHA-256 `3972dc97…b36986`; copied from a local GNU package
  copy, identical in eight installed packages).
- Manifests: `license = "GPL-3.0-or-later"` and `authors` in
  `[workspace.package]`, inherited by every crate (`license.workspace`);
  `publish = false` stays until crates.io names are settled.
- Notices: README "License" section; CLI help footer and the macOS About
  panel (copyright, licence, website) from `vitrine_app::about`; Tauri
  bundle `publisher`/`copyright`/`license`. No `licenseFile` in the bundle
  (it would make the NSIS/DMG installers show an accept-the-licence page,
  which the GPL does not require).
- **CLA** (`CLA.md`, draft for legal review): the contributor keeps the
  copyright and grants the owner a perpetual, irrevocable, non-exclusive
  copyright licence including sublicensing and distribution under any
  other licence, plus a patent licence. Structure and wording from the
  Apache ICLA v2.2, outbound "any licence" option from the Harmony
  HA-CLA-I-ANY. A licence CLA fits German law, where copyright cannot be
  transferred, only rights of use granted (§ 31 UrhG). Signing via a CLA
  bot on pull requests; **CLA Assistant (hosted app, gist)** recommended,
  CLA Assistant Lite (action, signatures in the repo) as alternative with a
  ready workflow in `docs/dev/cla-workflow.yml`. Not activated yet.
- **Brand assets are excluded from the GPL**: name, logo and app icons
  (`assets/brand/`, `crates/gui/icons/`, `crates/gui/ui/img/logo.svg`) are
  © the owner, all rights reserved; unmodified copies may carry them,
  forks must rebrand (`assets/brand/README.md`). GPL v3 §7(e) explicitly
  allows declining trademark rights; as sole copyright holder the owner can
  grant the redistribution permission for unmodified copies.
- **No SPDX header in every source file for now.** One owner and one
  licence for all code; the manifests and `LICENSE` carry it. Adding
  headers to ~all files is churn without a reader; revisit when files with
  a different licence enter the tree or before 1.0 (`reuse lint` would then
  be the tool).

### Dependencies and bundled data (reviewed 2026-11-11)

- **Rust crates** (`cargo metadata`, 677 packages incl. dev and all
  platforms; no `cargo deny`/`cargo about` installed): all are MIT,
  Apache-2.0, BSD-2/3-Clause, ISC, Zlib, MPL-2.0, Unicode-3.0, CC0-1.0,
  BSL-1.0, 0BSD, Unlicense (as an OR alternative) or CDLA-Permissive-2.0
  (`webpki-root-certs`, data) – all compatible with GPL v3. Notes:
  Apache-2.0-only crates (e.g. `rmcp`, `tao`) are GPL-3-compatible, not
  GPL-2 – one more reason for "v3 or later". MPL-2.0 crates (`cssparser`,
  `selectors`, `option-ext`, `dtoa-short`) are file-level copyleft; we use
  them unmodified from crates.io, which satisfies their source obligation.
  `r-efi` offers LGPL-2.1 only as one option. No GPL-incompatible crate.
- **Inter fonts (SIL OFL 1.1)**: the OFL explicitly permits bundling and
  embedding the fonts with any software; the fonts stay under the OFL
  (separate work, aggregate), `OFL.txt` must ship. Compatible.
- **Hyphenation patterns**: the bundled sets are MIT, BSD-3-Clause or
  permissive notices – compatible; their notices must ship
  (`assets/hyphenation/LICENSES.md`). **Swedish (LPPL 1.2 only)** stays
  out: the FSF lists LPPL 1.2 as free but GPL-incompatible, and the
  patterns are compiled into the binary (`include_str!`), which is weaker
  as "aggregate" than a separate data file. Revisit if Swedish is
  requested: load it as a separate unmodified data file next to the binary,
  or ask the author for MIT. Flagged for the owner.
- Third-party notices must ship with the installers (Phase 6.5: generate
  with `cargo about`, plus OFL and hyphenation notices). A `deny.toml`
  (licence allow-list) is to be added together with `cargo deny` in CI.

## Consequences

- Distributors (including the owner's own builds) must offer the
  corresponding source; the website/GitHub release does that.
- The owner can still publish builds under other terms (e.g. App Store
  distribution, which some see as conflicting with GPL v3 – the owner as
  sole licensor is not bound by that) because every contribution is
  licensed to them through the CLA.
- Contributors without a signed CLA cannot be merged.
- Open for the owner: legal review of `CLA.md`, CLA bot setup, optional
  repository rename.
