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
