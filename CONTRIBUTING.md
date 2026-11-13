# Contributing to Vitrine Studio

Thanks for helping! Vitrine Studio is pre-alpha, so please open an issue
before larger changes – the architecture ([`ARCHITECTURE.md`](ARCHITECTURE.md))
is binding, and the current state is in
[`docs/plan/STATUS.md`](docs/plan/STATUS.md).

## Contributor licence agreement (CLA)

Vitrine Studio is licensed under **GPL-3.0-or-later** ([`LICENSE`](LICENSE)).
Before a pull request can be merged, you sign the
[contributor licence agreement](CLA.md) once:

- You **keep the copyright** in your contribution.
- You grant Looking4Cache UG (haftungsbeschränkt), the project owner, a
  perpetual, irrevocable, non-exclusive licence to use and distribute your
  contribution – under the GPL and also under other licences (for example
  a commercial licence or a store-distribution exception) – plus a patent
  licence for your contribution. This keeps the project's licensing in one
  hand without a copyright transfer.
- You confirm the contribution is your own work (or you say where it comes
  from) and that your employer, if it has rights to it, agrees.

Signing happens on your first pull request: the CLA bot posts a comment and
you reply with the sentence it asks for. Trivial fixes (typos) are welcome
too, but the bot asks for every contributor.

## Development gate

Before every commit (CI runs the same on macOS, Linux and Windows):

```sh
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
VITRINE_KEYRING=mock cargo test --all
```

- Code, comments, commit messages and repo docs are in English.
- Small, thematic commits; message style `Area: what changed` (for example
  `GUI: …`, `Core: …`, `Docs: …`).
- Model changes only through the Operations API in `vitrine-core`; no
  `unsafe`; platform-gated code must be lint-clean on every OS. See
  [`AGENTS.md`](AGENTS.md) for the full rules (it is written for coding
  agents but applies to everyone).
- Visible renderer or fixture changes update the golden images in the same
  commit (`VITRINE_UPDATE_GOLDEN=1 cargo test -p vitrine-render --test golden`,
  review the diff).
- User-facing features add a short note to
  [`docs/guide/NOTES.md`](docs/guide/NOTES.md).

## What can and cannot be contributed

- **No vendor graphics** (Apple/Google device images etc.): built-in frames
  are drawn parametrically from catalog data.
- **Third-party code or data** (fonts, hyphenation patterns, catalog data)
  only under a licence compatible with GPL-3.0-or-later and with its notice
  (see [`assets/hyphenation/LICENSES.md`](assets/hyphenation/LICENSES.md),
  ADR 0022); say so in the pull request.
- **Name and logo** (`assets/brand/`) are not under the GPL and not open for
  contributions without the owner's agreement; see
  [`assets/brand/README.md`](assets/brand/README.md).
- New Rust dependencies need a GPL-3.0-compatible licence (MIT,
  Apache-2.0, BSD, ISC, Zlib, MPL-2.0, Unicode-3.0, CC0, …).

## For the maintainer: CLA automation

Not switched on yet (decision 2026-11-11, ADR 0022). Two options:

1. **CLA Assistant (recommended)** – the hosted GitHub App from
   [cla-assistant.io](https://cla-assistant.io) (SAP, open source):
   1. Create a public GitHub **gist** with the text of `CLA.md` (file name
      e.g. `vitrine-studio-cla.md`; every gist revision becomes a new CLA
      version, signers are asked again).
   2. Sign in at cla-assistant.io with the owner account, "Configure CLA",
      pick the repository and the gist, save. The app adds a required status
      check and a comment on pull requests.
   3. Optional: in the repository's branch protection, require the
      `license/cla` check.
   No secrets, no workflow file, nothing to maintain. Signatures are stored
   by the hosted service (GitHub login, date, gist version) and can be
   exported as CSV there.
2. **CLA Assistant Lite** – a GitHub Action
   (`contributor-assistant/github-action`) that keeps the signatures as a
   JSON file in the repository itself (own data, no third-party service).
   A ready-to-copy workflow is in
   [`docs/dev/cla-workflow.yml`](docs/dev/cla-workflow.yml): copy it to
   `.github/workflows/cla.yml`. With signatures in the same repository the
   built-in `GITHUB_TOKEN` is enough; storing them in a separate (private)
   repository needs a fine-grained personal access token as secret
   `CLA_PAT`. Check the action's maintenance status and pin its current
   release before enabling it.

Recommendation: option 1 – one-time setup in the browser, no token, no
workflow that can break, and it handles CLA text versions via the gist.
Pick option 2 only if signature records must stay in the repository.
