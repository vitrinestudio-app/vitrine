# Arrangia

Arrangia is an open-source desktop app for creating App Store and Play Store
screenshots: device frames, texts, translations and panorama sets, designed once
and rendered for every language, device class and store – with store upload and
an MCP interface so Claude can design and translate alongside you.

Instead of composing individual images, you design one continuous canvas. A
device that spans two screenshots exists exactly once, backgrounds run across
all pages, and gaps between pages match the store gallery so transitions line
up. The same layout adapts to other aspect ratios and to landscape sets built
from grouped portrait pages.

Website: <https://arrangia.app>

## Status

**Pre-alpha, Phase 0 (rendering spike).** Nothing usable yet; the workspace
contains crate stubs only. The full concept (in German) is in
[`docs/KONZEPT.md`](docs/KONZEPT.md).

## Building

Requirements:

- [rustup](https://rustup.rs); the toolchain (Rust 1.99.1) is pinned in
  `rust-toolchain.toml` and installed automatically.
- For the GUI, the [Tauri 2 prerequisites](https://v2.tauri.app/start/prerequisites/)
  of your platform (on Linux e.g. `libwebkit2gtk-4.1-dev`, `libgtk-3-dev`,
  `libayatana-appindicator3-dev`, `librsvg2-dev`, `libxdo-dev`).
  No Node.js or npm is needed.

```sh
cargo build                          # build everything
cargo test --all                     # run tests
cargo run -p arrangia-cli -- --help  # command line interface (binary: arrangia)
cargo run -p arrangia-gui            # desktop app
```

## Command line

The `arrangia` binary works on a project folder (`--project <DIR>`, default:
the current folder):

```sh
arrangia new my-app --lang en --lang de      # project from a template (--template phone|minimal)
arrangia validate --project my-app           # findings by target/language; --format json, --preview, --strict
arrangia render --project my-app --canvas    # preview PNG: --page N | --image N | --canvas | --gallery
arrangia render --project my-app --target play-phone --page 2 --width 400 -o page.png
arrangia export --project my-app             # all targets × languages into my-app/export/
arrangia export --target ios-iphone-69 --lang de --force   # --force ignores the incremental cache
```

Exit codes: `0` success (warnings fail only with `validate --strict`), `1`
the project has problems (validation errors, export failures), `2` the
command could not run (usage error, project not loadable, IO error).
`render` prints the path of the written PNG (default
`<project>/.cache/render/`). Colours are used on a terminal unless
`NO_COLOR` is set.

## Workspace layout

| Path | Crate | Purpose |
|---|---|---|
| `crates/core` | `arrangia-core` | Project model, YAML IO, operations API, resolution, layout, validation, catalogs |
| `crates/render` | `arrangia-render` | Rasterization, parametric device frames, text rendering, export |
| `crates/publish` | `arrangia-publish` | App Store Connect and Google Play upload, keychain |
| `crates/mcp` | `arrangia-mcp` | MCP server |
| `crates/cli` | `arrangia-cli` | Command line interface, binary `arrangia` |
| `crates/gui` | `arrangia-gui` | Tauri 2 desktop app, frontend in plain HTML/CSS/JS |
| `docs/` | | Concept and architecture decision records (`docs/adr/`) |

## Open questions

- **License: not yet decided.** Options are MIT OR Apache-2.0 or a copyleft
  license (MPL-2.0/GPL). Until this is settled there is no LICENSE file, no
  `license` field in the manifests, and all crates are `publish = false`.
  Bundled fonts will be under the SIL Open Font License.
- **Gallery gap widths** for each store profile still have to be measured on
  real store views.
