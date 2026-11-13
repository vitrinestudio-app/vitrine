# AGENTS.md

Guide for coding agents (and humans) working in this repository. It says
where things are and how to work here; the binding description of the
system and the decisions behind it is `ARCHITECTURE.md`, the current state
`docs/plan/STATUS.md`.

## Product

Vitrine Studio is an open-source (GPL-3.0-or-later) desktop app, written in
Rust, for App Store and Play Store screenshots: device frames, texts,
translations, panorama sets, store upload and an MCP interface. A layout is
designed once and rendered for every language, device class and store.
Projects are plain folders of YAML files and images (Git-friendly, no
account). GUI (Tauri 2), CLI (`vitrine`) and MCP server are three front ends
on the same core.

## Core concepts

The binding description is **`ARCHITECTURE.md`**; read it before
changing the model, renderer or architecture. Glossary: its §2; user-facing
explanation: `website/content/en/docs/concepts.md`. "ADR 00NN" and
"KONZEPT §n" references in code resolve through its §14.

- **Canvas, not single images.** A canvas is a row of equal pages;
  backgrounds run across pages, a device spanning two pages exists once.
  Each page becomes one export image (or a group of pages becomes one).
- **lu.** A page is 1000 lu wide; all sizes are in lu. Pixel size comes
  only from the target.
- **Home page.** Every element hangs on a page (`page`); its position is
  relative to that page and it may reach into neighbours. Texts are clipped
  to their page by default (`clip: page`).
- **Gaps.** Gaps between pages come from the target's gallery profile, like
  in the store; content inside a gap is left out of page exports.
- **Targets, variants, anchors.** A target renders a canvas at a pixel
  size (optionally a store slot, a device swap, page grouping such as
  2 portrait pages → 1 landscape image). Variants override geometry per
  format; vertical anchors (`top`/`bottom`/`center`/`stretch`) adapt to
  other aspect ratios. Content is never maintained twice.
- **One renderer.** GUI preview, export and MCP `render_preview` all use
  the same Rust pipeline. The GUI never renders in the browser: it shows
  Rust pixmaps plus HTML/SVG overlays.

## Repo map

| Path | What lives there |
|---|---|
| `crates/core` (`vitrine-core`) | model, YAML IO, Operations API (`ops`), edit intents (`edit`), resolve, geometry, text layout + fonts (`layout`), validation (`validate`), catalogs (`catalog`, data in `crates/core/data/*.yaml`), colour profiles (`colorspace`), import/exchange, fastlane paths, frame import + frame packs (`frame_import`), templates (`template`, built-ins in `crates/core/templates/`). No rendering, no network. |
| `crates/render` (`vitrine-render`) | tiny-skia rasterizer (`scene`), parametric frames (`frames`), glyph outlines (`text`), export (`export`: files, gallery previews, manifest, `index.html`, fastlane layout), design checks on pixels (`quality`), frame packs, template previews |
| `crates/publish` (`vitrine-publish`) | App Store Connect (`apple`, incl. App Asset Library), Google Play (`google`), gallery client (`gallery`), keychain (`credentials`), CI credentials from env (`env_credentials`) |
| `crates/app` (`vitrine-app`) | transport-agnostic application layer shared by GUI and MCP: `commands/` (one module per area, `dispatch` by name), `session` (open/apply/undo/save/watch), `view`, `preview`, `settings`, `stores`, `upload/` (plan → confirm → execute, upload log), `claude_desktop`, `cli_install`, `mcp` status. No Tauri. |
| `crates/mcp` (`vitrine-mcp`) | MCP server over `vitrine-app`: tools in `src/tools/<area>.rs`, server instructions `src/instructions.md`, op reference `src/ops.md`, `docs_get` topics in `src/docs/*.md`, HTTP host (GUI) and stdio proxy |
| `crates/cli` (`vitrine-cli`) | binary `vitrine` (clap), one module per command |
| `crates/gui` (`vitrine-gui`) | Tauri 2 app (`src/main.rs`), dev HTTP bridge (`src/bridge.rs`, `src/bin/vitrine-devbridge.rs`), updater, native menu; frontend in `crates/gui/ui/` (vanilla ES modules, no npm) |
| `crates/api-server` (`vitrine-api-server`) | backend of api.vitrinestudio.app (axum, SQLite, Docker; binary `vitrine-api`): template gallery + GitHub login. Not part of the app. |
| `assets/` | bundled OFL fonts (`assets/fonts/<family>/` with its `OFL.txt`, Inter + Inter Display in `inter/`; `assets/fonts/SOURCES.md`), hyphenation patterns (`assets/hyphenation/`, `LICENSES.md`), brand files (`assets/brand/`, not GPL) |
| `tests/fixtures/example/` | example project (all target kinds; en, de, fr, fr-CA with deliberate fallbacks) + golden images (`golden/`) |
| `tests/fixtures/scripts/` | non-Latin script fixture (render tests) |
| `website/` | www.vitrinestudio.app (Hugo, no theme): landing page, user guide `content/{en,de}/docs/`, template gallery page; see `website/README.md` |
| `integrations/` | unpublished MIT drafts, each folder self-contained for its own repo: `github-action/`, `gitlab/`, `fastlane-plugin/`, `skill/` (agent skill); see `integrations/README.md` |
| `scripts/release/` | release helpers (licence files, CLI sidecar, Linux CLI builds, collect artifacts, `latest.json`) |
| `scripts/docs/cli-help.sh` | regenerates `website/data/cli_help.json` |
| `docker/cli/` | Dockerfile of the CLI image `ghcr.io/vitrinestudio-app/vitrine-cli` |
| `.github/workflows/` | `ci.yml` (fmt/clippy/test per OS + licences), `release.yml` (tag `v*` → draft release + CLI image) |
| `.signpath/` | Windows signing configuration drafts (dormant) |
| `deny.toml`, `about.toml` | licence allow-list (`cargo deny`), third-party notices (cargo-about) |
| `ARCHITECTURE.md` | binding description of the system; decision index (§14) and decision log (§15) |
| `docs/archive/` | historical: the German concept `KONZEPT-de.md` and the former ADRs `adr/NNNN-title.md` (+ `adr/images/`), superseded by `ARCHITECTURE.md` |
| `docs/plan/` | `STATUS.md` (state + decision log), `phase-N.md` task lists, `phase-N.2.md` = a finished side work package of phase N |
| `docs/guide/` | `NOTES.md` (user-guide notes), `frame-packs.md` |
| `docs/research/` | dated research notes and MCP test reports |
| `docs/api/` | `API.md` (gallery API), `DEPLOY.md` (server deployment) |
| `docs/dev/` | `releasing.md`, `adding-devices.md`, inactive CLA workflow draft |
| `docs/example/`, `docs/gui/` | images for the README |

Version: `[workspace.package] version` in `Cargo.toml` is the single source.

## Where do I change X?

| Change | Where |
|---|---|
| New model field | `crates/core/src/model/` (serde, canonical YAML form in `io`), a patch field in `ops` (`ops/patch.rs` or `model::ElementPatch`) so it has an inverse op, resolve/render if it draws, `crates/mcp/src/ops.md` (+ `src/docs/*.md` if relevant), `website/content/{en,de}/docs/file-format.md`, NOTES.md; `ARCHITECTURE.md` if it changes the concept |
| New model change in general | an `Op` in `crates/core/src/ops/` → `Project::apply` → inverse op. Several ops for one gesture: a Rust command/edit intent (`crates/core/src/edit/`), never in JS |
| New app command | module in `crates/app/src/commands/`, entry in `command_table!` (`crates/app/src/commands/mod.rs`) **and** in `json_commands!`/`image_commands!` + `generate_handler!` in `crates/gui/src/main.rs` |
| New MCP tool | the matching `#[tool_router]` block in `crates/mcp/src/tools/<area>.rs` (args = `schemars::JsonSchema` struct with doc comments, `deny_unknown_fields`; body via `VitrineMcp::run` calling an app command), tool list in `docs/guide/NOTES.md` (section "AI assistants / MCP"), `crates/mcp/src/instructions.md` / `src/docs/*.md` if it changes usage, `website/content/{en,de}/docs/ai-mcp.md` |
| New validation finding code | `FindingCode` in `crates/core/src/validate/mod.rs` (+ `ALLOWABLE` if `allow`/`checks.off` may switch it off), the check in `validate/` or `crates/render/src/quality/` (pixel checks), `crates/mcp/src/docs/checks.md` (a test requires every code there), the `allow` code list in `crates/mcp/src/docs.rs`, `website/content/{en,de}/docs/troubleshooting.md` |
| GUI panel, dialog, view | `crates/gui/ui/` – conventions in `crates/gui/ui/README.md`; strings in **both** `crates/gui/ui/i18n/en.json` and `de.json` |
| Store sizes, display types, rules, gallery gaps, locales, fastlane names | `crates/core/data/*.yaml`, each value with source URL + verification date; unmeasured gaps marked `not yet measured` |
| Devices / built-in frames | `crates/core/data/devices.yaml` + `crates/render/src/frames/`; guide: `docs/dev/adding-devices.md` |
| CLI command or flag | `crates/cli/src/`, then `scripts/docs/cli-help.sh` (regenerates `website/data/cli_help.json`) and `website/content/{en,de}/docs/cli.md` if needed |
| Website page or text | `website/content/en/` **and** `website/content/de/` (same file name); UI strings `website/i18n/{en,de}.yaml`; landing texts in the front matter of `content/<lang>/_index.md` |
| Gallery API | `crates/api-server/src/routes/<area>.rs`, `docs/api/API.md`; deployment changes in `docs/api/DEPLOY.md` |
| Upload flow | `crates/app/src/upload/`, store clients in `crates/publish/src/{apple,google}` (ADR 0017–0019) |
| Release process | `.github/workflows/release.yml`, `scripts/release/`, `docs/dev/releasing.md` |
| Architecture decision | the relevant section of `ARCHITECTURE.md` + a dated row in its decision log (§15; next number = last row + 1); no new ADR files |

## Rules (must follow)

- **Model changes only through the Operations API** in `core` (`Op` →
  `Project::apply` → inverse op for undo). GUI, CLI and MCP never modify
  model fields directly.
- **No `unsafe`** (workspace lint `unsafe_code = "forbid"`).
- **Platform-gated code is lint-clean on every OS.** Keep `cfg` branches
  symmetric: an item, variant, import or helper used only in a
  `#[cfg(target_os = "macos")]` (or `unix`) branch is gated the same way.
  Check with the cross-clippy commands below when touching `cfg` code.
- **Licences.** New dependencies and bundled data need a
  GPL-3.0-compatible licence (MIT, Apache-2.0, BSD, ISC, Zlib, MPL-2.0,
  Unicode-3.0, CC0, OFL for fonts) and their notice; `cargo deny check
  licenses` must pass. No SPDX headers per file. `publish = false` in
  every crate. Integrations are MIT; name, logo and icons are not GPL.
- **No vendor graphics** (Apple/Google device images etc.) in the repo.
  Built-in frames are drawn parametrically; Apple bezels are imported by
  the user (`.gitignore` guards the usual paths).
- **Store values are data** in `crates/core/data/*.yaml` with source URL
  and date, never hard-coded.
- **Golden and render tests use only the bundled fonts**, never system
  fonts.
- **UI i18n:** every key in both `crates/gui/ui/i18n/en.json` and
  `de.json`.
- **Never touch the owner's real state in tests or manual runs:** keychain
  via `VITRINE_KEYRING=mock` (unit tests use `SecretStore::mock()`), app
  data via `VITRINE_APP_DIR` / dev bridge `--app-dir`, Claude Desktop config
  via `VITRINE_CLAUDE_DESKTOP_CONFIG`, CLI symlink via
  `VITRINE_CLI_LINK_DIR`. No real network in tests (wiremock, loopback
  endpoint overrides such as `VITRINE_ASC_BASE_URL`).
- **Tauri CSP** (`crates/gui/tauri.conf.json`) forbids inline styles and
  scripts. The dev bridge has no CSP, so check the real app after
  touching resource loading. The web view gets only `core:default`; all
  file/system access goes through custom commands.
- **Language:** code, comments, docs and commit messages in English. The
  owner speaks German; answer him in German.

## How to work

- **Read `docs/plan/STATUS.md` first** (and the current phase plan).
- **Small, thematic commits**, message style `Area: what changed` (e.g.
  `GUI: …`, `Core: …`, `Docs: …`). Agents end commit messages with a
  `Co-Authored-By:` trailer naming their model, as in `git log`.
- **Don't push** unless asked.
- **Document as you go:** user-facing features or findings → short bullets
  in `docs/guide/NOTES.md`; decisions with the owner and phase state →
  `docs/plan/STATUS.md`; architecture decisions → update the matching
  section of `ARCHITECTURE.md` and add a row to its decision log (take the
  next number after the last row; parallel agents may take the same one –
  renumber on merge). Visible features also update the user guide in
  `website/content/{en,de}/docs/`.
- **Keep every description in sync – part of "done", not optional.** Before
  you finish a change, check and update everything that describes the
  changed behaviour, so nothing goes stale:
  - `ARCHITECTURE.md` (the affected section; decision log if a decision
    changed) and this file (repo map, "where do I change X", commands);
  - the user guide `website/content/en/` **and** `de/` (feature pages,
    `troubleshooting.md`, `file-format.md`, `cli.md`), the landing page
    texts if they mention it, and `website/data/cli_help.json` for CLI
    changes;
  - screenshots in `website/assets/images/` (and `docs/gui/`) that show
    changed UI – retake them as light/dark pairs (`website/README.md`,
    "Screenshots") or note them as stale in `docs/plan/STATUS.md`;
  - MCP docs (`crates/mcp/src/instructions.md`, `ops.md`, `docs/*.md`) and
    the agent skill (`integrations/skill/`) when tools, fields or
    behaviour change;
  - `README.md`, `docs/guide/NOTES.md`, `docs/plan/STATUS.md`, the phase
    plan (tick what is done).

  Grep for the old name/behaviour (`git grep`) to find stale mentions.
- **Parallel agents / worktrees** (`.claude/worktrees/`):
  - use your own `CARGO_TARGET_DIR` (e.g. `target/<agent-name>`) so you
    never hold the cargo lock on `target/`, and delete it when done (disk
    space); keep `target/debug` – the owner runs the GUI from it;
  - `git merge main` before finishing, resolve STATUS/NOTES/i18n conflicts
    by keeping both sides;
  - stop only your own processes: never `pkill -f vitrine-devbridge`; kill
    by your port (`lsof -ti tcp:<port> | xargs kill`);
  - check `git status` before committing: the owner edits files in the
    same tree.
- Never use tokens the owner pastes into chat; ask him to run the command
  (`--token-env`).

## Commands

```sh
cargo build                                   # everything
cargo run -p vitrine-gui                      # GUI (start screen)
cargo run -p vitrine-gui -- tests/fixtures/example   # … with a project
cargo run -p vitrine-cli -- --help            # CLI (binary `vitrine`)

# dev bridge: the UI + all GUI commands in a normal browser (prints the URL).
# --copy = temp copy, --port N, --app-dir DIR, --pick-folder/--pick-file
# answer dialogs, --mcp also hosts MCP; ?theme=dark|light, ?lang=en|de
cargo run -p vitrine-gui --features devbridge --bin vitrine-devbridge -- --project tests/fixtures/example --copy

# MCP: the GUI hosts it; `vitrine mcp` proxies stdio to the running GUI
cargo run -p vitrine-cli -- mcp --headless --project tests/fixtures/example --app-dir /tmp/vitrine-mcp

# CLI on the example (release: a full export takes ~2 s)
cargo run -p vitrine-cli --release -- --project tests/fixtures/example validate
cargo run -p vitrine-cli --release -- --project tests/fixtures/example export
cargo run -p vitrine-cli --release -- --project tests/fixtures/example render --target ios-iphone-69 --gallery --max-edge 800 -o /tmp/gallery.png
```

**Gate before every commit** (CI runs the same on macOS, Linux, Windows):

```sh
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
VITRINE_KEYRING=mock cargo test --all
```

More checks:

```sh
# cross-clippy from the Mac when touching cfg code (one-time: brew install mingw-w64,
# brew install messense/macos-cross-toolchains/x86_64-unknown-linux-gnu,
# rustup target add x86_64-pc-windows-gnu x86_64-unknown-linux-gnu)
CARGO_TARGET_DIR=target/cross cargo clippy --target x86_64-pc-windows-gnu --workspace --all-targets --all-features -- -D warnings
CARGO_TARGET_DIR=target/cross cargo clippy --target x86_64-unknown-linux-gnu --workspace --exclude vitrine-gui --all-targets --all-features -- -D warnings
cargo deny check licenses                     # after adding/upgrading dependencies

# golden images: only after an intended visible change; review the diff, commit with the change
VITRINE_UPDATE_GOLDEN=1 cargo test -p vitrine-render --test golden

# website (in website/): must build without warnings, links must resolve
hugo server
hugo --minify -d /tmp/site && python3 scripts/check-links.py /tmp/site
scripts/docs/cli-help.sh                      # (repo root) after changing CLI commands/flags

# regenerate the example screenshots (deterministic fake app UI)
cargo run -p vitrine-render --release --example screens -- tests/fixtures/example/screenshots
```

Release bundles are built by CI on a `v*` tag; local unsigned bundle and
the owner's checklist: `docs/dev/releasing.md`.

CLI usage: `vitrine --help` / `vitrine <cmd> --help`,
`website/content/en/docs/cli.md`, `website/data/cli_help.json`. Exit codes:
0 ok, 1 project problems, 2 could not run. `vitrine mcp` must never print
to stdout outside the protocol.

## Tests

| Suite | Guards | Run when |
|---|---|---|
| Unit tests (in `src/`) | module logic everywhere | always (`cargo test -p <crate>`) |
| `crates/core/tests/` (`ops`, `roundtrip`, `resolve`, `validate`, `edit`, `store`, `clip_to`) | Operations API + inverses, YAML round trip, resolve, validation, edit intents on `crates/core/tests/data/sample`; insta snapshots in `tests/snapshots/` | model, IO, resolve, validate changes |
| `crates/render/tests/golden.rs` | every output image of every target of the example (en + extra langs: de iPhone + blog header, fr iPhone + iPad) at 25 % and gallery previews at 320 px vs `tests/fixtures/example/golden/` (pixel > 8/255, image fails > 0.5 %; writes `*.actual.png`/`*.diff.png`) | any renderer, frame, font or fixture change |
| other `crates/render/tests/` | export layouts + `index.html`, effects, group scale, `clip_to`, imported/stored frames (synthetic bezels), non-Latin scripts | renderer/export changes |
| `crates/cli/tests/cli.rs`, `import.rs`, `strings.rs` | `vitrine` binary end to end | CLI changes |
| `crates/cli/tests/scenario.rs` | 3 languages × 3 targets exported with fallbacks, overflow reported | i18n/fallback/export changes |
| `crates/cli/tests/mcp.rs`, `mcp_proxy.rs` | `vitrine mcp` over stdio JSON-RPC; proxy to an in-process GUI endpoint | MCP tools, host, proxy |
| `crates/cli/tests/upload.rs`, `gallery.rs` | upload plan/log without network; `vitrine gallery` against an in-process API + fake GitHub | upload / gallery CLI |
| `crates/app/tests/` | store connection test, store setup, upload plan + executor against fake stores | app upload/store code |
| `crates/publish/tests/` | App Store Connect, App Asset Library, Google Play clients (wiremock) | store clients |
| `crates/api-server/tests/` | gallery API in-process, GitHub login (wiremock) | api-server |
| `crates/gui/tests/bridge.rs` | dev bridge over real HTTP | bridge, command wiring |

Golden tests run in the normal debug `cargo test` (dependencies use
`opt-level = 2`). Changing the fixture or the renderer visibly means
updating the references in the same commit.

Cross-platform rules: build paths with `Path::join`, never compare paths
as strings; line endings are LF (`.gitattributes`), tests that read
checked-in text normalise `\r\n` anyway.

## Where things are documented

| Document | Content |
|---|---|
| `ARCHITECTURE.md` | binding description: concepts, format, pipeline, front ends, rules; decision index + log |
| `docs/archive/` | historical concept (German) and ADRs 0000–0032 (reasons, research, measurements) |
| `docs/plan/STATUS.md` | current state, decision log, open owner questions |
| `docs/plan/phase-*.md` | task lists per phase (`phase-7.md` = release) |
| `docs/guide/NOTES.md` | user-facing notes incl. the MCP tool list |
| `docs/guide/frame-packs.md` | own frame package format |
| `crates/mcp/src/instructions.md`, `ops.md`, `docs/*.md` | MCP server instructions, op reference, `docs_get` topics |
| `crates/gui/ui/README.md` | frontend structure and panel/dialog conventions |
| `docs/api/API.md`, `docs/api/DEPLOY.md` | gallery API, server deployment |
| `docs/dev/releasing.md` | releases, signing, secrets |
| `docs/dev/adding-devices.md` | adding devices and frames |
| `website/README.md` | website structure, screenshots, gallery page, deployment |
| `integrations/README.md` | CI integration and skill drafts |
| `CONTRIBUTING.md`, `CLA.md` | contribution rules, CLA |
| `website/content/{en,de}/docs/` | the user guide |

## Current state

Pre-alpha, version 0.1.0. The features planned for 1.0 are built (phases
0–6); Phase 7 is the release work: signing, publishing the first version,
website and gallery going live, integrations, store test runs. Details and
open owner decisions: `docs/plan/STATUS.md` and `docs/plan/phase-7.md`.
