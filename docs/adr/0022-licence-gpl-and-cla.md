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
- **CLA** (`CLA.md`, version 1.0, reviewed by the owner 2026-11-11): the contributor keeps the
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
- **Bundled fonts (SIL OFL 1.1)**: Inter, and since 2026-11-11 Montserrat,
  Playfair Display, Lora, Nunito, Oswald, Bebas Neue, Caveat, Dancing
  Script, JetBrains Mono (from github.com/google/fonts, pinned commit,
  checksums in `assets/fonts/SOURCES.md`). The OFL explicitly permits
  bundling and embedding the fonts with any software; the fonts stay under
  the OFL (separate work, aggregate), each family's `OFL.txt` (own
  copyright line) must ship. Fonts are never modified (no subsetting), so
  the Reserved Font Names of Dancing Script, Lora and Playfair Display do
  not restrict us. Compatible.
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

### CLA drafting notes (background)

Points considered while drafting `CLA.md` and handed to the owner for the
review (done 2026-11-11); kept here so the published CLA contains only the
agreement.

- **Unknown types of use (§ 31a UrhG):** a grant for types of use unknown at
  the time of signing needs written form unless it is a simple right granted
  free of charge *to everyone*. A click-through signature probably does not
  meet the written form; the grant in section 2 is therefore limited to known types
  of use. Ask the lawyer whether that is enough or whether a different
  signing method is wanted.
- **Relicensing clause (section 2, "any other licence"):** this is the core
  of the decision (dual licensing / commercial editions stay possible). The
  promise to keep the code available under an open source licence is a
  goodwill clause taken from the Harmony "any licence" option – keep, adjust
  or drop it consciously.
- **Liability (section 5):** under German law liability for intent and
  gross negligence cannot be excluded in advance; the clause says so.
- **Corporate CLA:** not drafted; the Apache CCLA v2.2 would be the
  matching base if it is ever needed.
- Version set from `1.0-draft` to `1.0` (2026-11-11) after the owner's
  review.

## Consequences

- Distributors (including the owner's own builds) must offer the
  corresponding source; the website/GitHub release does that.
- The owner can still publish builds under other terms (e.g. App Store
  distribution, which some see as conflicting with GPL v3 – the owner as
  sole licensor is not bound by that) because every contribution is
  licensed to them through the CLA.
- Contributors without a signed CLA cannot be merged.
- Open for the owner: CLA bot setup, optional repository rename.
  (`CLA.md` reviewed by the owner on 2026-11-11.)

## Amendment 2026-11-12: CI integrations are MIT

Owner decision: glue code that only runs the `vitrine` CLI as a separate
program is licensed under **MIT** (copyright Looking4Cache UG
(haftungsbeschränkt), 2026): the GitHub Action
(`integrations/github-action/`, repository
`vitrinestudio-app/vitrine-github-action`), the GitLab CI/CD components
(`integrations/gitlab/`, gitlab.com `vitrinestudio/vitrine-gitlab-ci`) and
the fastlane plugin (`integrations/fastlane-plugin/`, gem
`fastlane-plugin-vitrine`). Each folder carries its own `LICENSE` and says
so in its README.

- Why: users copy these files into their own pipelines, Fastfiles and
  repositories; a permissive licence avoids any GPL question for their
  build configuration. They link no Vitrine Studio code – they download
  and execute the separately distributed program ("aggregate" / separate
  program, not a derivative work), so GPL-3.0 does not reach them and MIT
  is compatible either way.
- The app, the CLI, all crates and the container image's binary stay
  **GPL-3.0-or-later** (the image ships the release archive's licence
  files under `/usr/share/doc/vitrine/licenses/`).
- Contributions to the integrations go through the CLA as well (`CLA.md`
  covers contributions "to the Project … on the Project's repositories",
  which includes the separate integration repositories, and allows any
  outbound licence). The CLA bot has to be enabled on those repositories
  too (GitLab: no CLA bot – accept contributions only via GitHub or ask
  for a signed CLA).
