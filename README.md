# Vitrine Studio

Vitrine Studio is an open-source desktop app for creating App Store and Play Store
screenshots: device frames, texts, translations and panorama sets, designed once
and rendered for every language, device class and store – with store upload and
an MCP interface so AI assistants such as Claude can design and translate
alongside you.

Instead of composing individual images, you design one continuous canvas. A
device that spans two screenshots exists exactly once, backgrounds run across
all pages, and gaps between pages match the store gallery so transitions line
up. The same layout adapts to other aspect ratios and to landscape sets built
from grouped portrait pages. Projects are plain folders of YAML files and
images: no account, no cloud, Git-friendly. The desktop app, the `vitrine`
command line tool and the MCP server work on the same core.

**Website:** [www.vitrinestudio.app](https://www.vitrinestudio.app) ·
**Documentation:** [user guide](https://www.vitrinestudio.app/docs/) ·
**Templates:** [template gallery](https://www.vitrinestudio.app/gallery/) ·
**Download:** [releases](https://github.com/vitrinestudio-app/vitrine/releases)

## Status

**Pre-alpha – the first public release (0.1.0) is being prepared.** The
features planned for 1.0 are built: core, command line tool, desktop editor,
languages & import, MCP, store upload, templates with an online gallery, own
frame packages, colour management, installers and the in-app updater. Store
upload works and is being tested with live store accounts. What remains
before the first release – signing, publishing, the website going live,
real-app checks – is tracked in [`docs/plan/phase-7.md`](docs/plan/phase-7.md).

The concept (in German) is in [`docs/KONZEPT.md`](docs/KONZEPT.md), the task
lists per phase in [`docs/plan/`](docs/plan/), the current state in
[`docs/plan/STATUS.md`](docs/plan/STATUS.md) and the architecture decisions
in [`docs/adr/`](docs/adr/).

## Features

- **Devices**: iPhone (including the foldable iPhone Duo, open or folded),
  iPad, Mac, Android phones and tablets, Apple Watch and Wear OS
  (screenshot-only exports), browser windows and notebooks – parametric
  frames in two styles, Apple Product Bezels you import yourself, or your
  own frame packages (PNG/SVG + `frame.yaml`). Imported bezels can be
  stored in a private project so CI renders them too.
- **Store formats** from presets: App Store and Google Play screenshot
  sets, App Store header and search results creatives, the Play feature
  graphic, web slides and banners cut from the same canvas.
- **Design**: texts with inline markup, auto-fit and highlighted spans
  (box with padding and corner radius), images clipped to a rectangle or
  ellipse (`clip_to`), groups that scale as one (also per variant),
  variants per format instead of maintaining content twice.
- **Fonts**: Inter and nine more bundled OFL families, your project's own
  fonts and the fonts installed on your computer; used system fonts can be
  stored in the project so every machine renders the same. Bundled script
  fallbacks for Arabic, Hebrew, Devanagari, Bengali, Tamil and Thai.
- **Languages**: fallback chains for texts and screenshots, per-language
  overrides, opt-in automatic hyphenation, CSV/XLIFF for translators.
- **Design checks**: `validate` reports missing screenshots and overflowing
  texts, and also looks at the rendered images: low contrast, text too
  close to a device, covered or cut screens, devices cut at the edge,
  seams and corners in the store gallery.
- **Templates**: start a project from a template, save your own canvases as
  a template, insert canvases from one. The online
  [template gallery](https://www.vitrinestudio.app/gallery/) is built into
  the *New project* dialog; browsing and downloading need no account,
  publishing uses a GitHub login.
- **Import & export**: screenshots from fastlane snapshot/screengrab (in
  their own captures folder) or plain folders; export in Vitrine Studio's
  own layout or the fastlane `deliver`/`supply` layout, with an
  `index.html` overview; incremental and colour-managed (Display P3 and
  other profiles converted to sRGB).
- **Store upload** to App Store Connect (screenshot sets and the App Asset
  Library) and Google Play from the app, the CLI or an AI assistant –
  always plan → confirm → execute; keys stay in the system keychain.
- **AI assistants**: an MCP server hosted by the desktop app, with an *Add
  to Claude Desktop* button; the assistant creates and edits projects,
  translates, looks at rendered previews, validates, exports and plans
  uploads. Its edits appear live and can be undone.
- **Automation**: `validate --format github|gitlab|junit` for CI reports,
  store keys from environment variables, standalone CLI archives and a CLI
  container image with every release; drafts of a GitHub Action, GitLab CI
  components and a fastlane plugin in [`integrations/`](integrations/)
  (published after the first release).

### Desktop editor

Each canvas in one continuous view with the store gaps hatched, direct manipulation
(move, resize, rotate, snapping), an inspector per element kind, layers,
assets with drag & drop import, a translations table and inline text
editing, canvases, targets from store presets and variants
managed in one dialog, a validation panel, an export dialog and an upload
dialog. On macOS the app menu also lives in the native menu bar. Every
preview is rendered by the same Rust pipeline as the export; every change
is saved immediately as plain YAML.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="docs/gui/editor-dark.jpg">
  <img src="docs/gui/editor.jpg" alt="The editor with a project made from the Synthwave Sunset template: layers on the left, six pages on one canvas, a selected phone in the inspector">
</picture>

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="docs/gui/text-editing-dark.jpg">
  <img src="docs/gui/text-editing.jpg" alt="A selected headline of the Park Poster template with its settings in the inspector">
</picture>

### Example

[`tests/fixtures/example`](tests/fixtures/example) is a complete project
for a fictional garden planner, exported from one YAML file into iPhone,
Google Play, iPad landscape, Mac App Store, web slides and banner targets
in English, German, French and Canadian French (which inherits all but
two texts from French). Store gallery previews as Vitrine Studio renders them:

![iPhone 6.9" set: one continuous canvas, a phone crossing pages 3 and 4](docs/example/iphone-gallery-en.jpg)

The same phone pages as an iPad landscape set (two pages per image,
tablet frames, a variant with adjusted positions), in German:

![iPad 13" landscape set in German](docs/example/ipad-gallery-de.jpg)

In French, with the French app screenshots for the first three pages and
the English ones as fallback for the rest; the German set above shows a
language override that lets "Mischkultur-partner" be hyphenated
automatically instead of shrinking the headline:

![iPhone 6.9" set in French](docs/example/iphone-gallery-fr.jpg)

Mac App Store set and a web app in a browser window and in a notebook
with browser chrome:

![Mac App Store set](docs/example/mac-gallery-en.jpg)

![Web app slides](docs/example/web-gallery-en.jpg)

A blog header cut from the same canvas without gaps:

![Blog header](docs/example/blog-header-de.jpg)

The app screenshots inside the frames are generated
(`crates/render/examples/screens.rs`); device frames are drawn
parametrically, no vendor artwork is included.

## Install

No release has been published yet. Releases will appear on
[GitHub Releases](https://github.com/vitrinestudio-app/vitrine/releases):
a signed and notarized universal `.dmg` for macOS 11+, a Windows setup
(not code-signed for now – SmartScreen asks once: *More info* → *Run
anyway*), Linux packages, standalone command line tool archives and the
container image `ghcr.io/vitrinestudio-app/vitrine-cli`. Details:
[Install & update](https://www.vitrinestudio.app/docs/install/).

Until then, build from source.

## Building

Requirements:

- [rustup](https://rustup.rs); the toolchain (Rust 1.99.1) is pinned in
  `rust-toolchain.toml` and installed automatically.
- For the GUI, the [Tauri 2 prerequisites](https://v2.tauri.app/start/prerequisites/)
  of your platform (on Linux e.g. `libwebkit2gtk-4.1-dev`, `libgtk-3-dev`,
  `libayatana-appindicator3-dev`, `librsvg2-dev`, `libxdo-dev`).
  No Node.js or npm is needed.

```sh
git clone https://github.com/vitrinestudio-app/vitrine.git && cd vitrine
cargo run -p vitrine-gui --release   # desktop app (start screen)
cargo run -p vitrine-gui -- tests/fixtures/example   # … opening a project
cargo install --path crates/cli      # the `vitrine` command line tool
cargo test --all                     # run tests
```

Contributor notes (commands, conventions, architecture):
[`CLAUDE.md`](CLAUDE.md) and [`CONTRIBUTING.md`](CONTRIBUTING.md).

## Command line

```sh
vitrine new my-app --lang en --lang de        # new project (--template <name|id|file>)
# put screenshots into my-app/screenshots/en/ios-phone/<key>.png (see project.yaml)
vitrine validate --project my-app             # what is missing or does not fit
vitrine render --project my-app --gallery     # preview PNG of a store gallery
vitrine export --project my-app               # PNG/JPEG files in my-app/export/<target>/<lang>/
```

Try it on the example: `vitrine export --project tests/fixtures/example`.
Further commands: `strings` (CSV/XLIFF), `import screenshots`, `frames`,
`fonts`, `template`, `gallery`, `upload`, `mcp` and `install-cli` – see
`vitrine --help` and the
[CLI reference](https://www.vitrinestudio.app/docs/cli/). Exit codes: `0`
success, `1` the project has problems, `2` the command could not run.
For CI see [Automation](https://www.vitrinestudio.app/docs/automation/).

## AI assistants (MCP)

The desktop app hosts an [MCP](https://modelcontextprotocol.io) server: the
assistant works on the project you have open, every change appears live and
is one undo step labelled with the client's name. Nothing goes over the
network (ADR 0014, ADR 0015).

- **Claude Desktop**: App settings → *AI assistants (MCP)* → **Add to
  Claude Desktop…**, then restart Claude Desktop.
- **Claude Code**: `claude mcp add vitrine-studio -- /path/to/vitrine mcp`
  (the exact command is shown in the same settings section).
- **Any MCP client**: a stdio server running `vitrine mcp`; it connects to
  the running app or starts it. Without the app (CI, scripts):
  `vitrine mcp --headless --project /path/to/my-app`.

Setup, example prompts and tools:
[AI assistants](https://www.vitrinestudio.app/docs/ai-mcp/).

## Workspace layout

| Path | Crate | Purpose |
|---|---|---|
| `crates/core` | `vitrine-core` | Project model, YAML IO, operations API, resolution, layout, validation, catalogs, templates |
| `crates/render` | `vitrine-render` | Rasterization, parametric device frames, text rendering, export, design checks |
| `crates/publish` | `vitrine-publish` | App Store Connect, Google Play and gallery clients, keychain |
| `crates/app` | `vitrine-app` | Shared application layer: session, commands, preview cache, settings (GUI and MCP) |
| `crates/mcp` | `vitrine-mcp` | MCP server (rmcp): tools and resources over the application layer |
| `crates/cli` | `vitrine-cli` | Command line interface, binary `vitrine` |
| `crates/gui` | `vitrine-gui` | Tauri 2 desktop app, frontend in plain HTML/CSS/JS |
| `crates/api-server` | `vitrine-api-server` | Backend of api.vitrinestudio.app (template gallery, GitHub login) |
| `integrations/` | | CI integration drafts (GitHub Action, GitLab CI, fastlane plugin; MIT) |
| `website/` | | www.vitrinestudio.app (Hugo) with the user guide |
| `docs/` | | Concept, plans, architecture decision records (`docs/adr/`) |

## License

Copyright © 2026 Looking4Cache UG (haftungsbeschränkt).

Vitrine Studio is free software: you can redistribute it and/or modify it
under the terms of the GNU General Public License as published by the Free
Software Foundation, either version 3 of the License, or (at your option)
any later version (`GPL-3.0-or-later`, see [`LICENSE`](LICENSE)). It is
distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY;
without even the implied warranty of MERCHANTABILITY or FITNESS FOR A
PARTICULAR PURPOSE.

- **Contributions** need a signed contributor licence agreement
  ([`CLA.md`](CLA.md)): you keep your copyright and grant the project owner
  a licence that also allows distributing the code under other licences.
  See [`CONTRIBUTING.md`](CONTRIBUTING.md).
- **CI integrations** in [`integrations/`](integrations/) are MIT-licensed
  (a `LICENSE` in each folder): they only run the `vitrine` program.
- **Name and logo** (`assets/brand/`, the app icons in `crates/gui/icons/`,
  `crates/gui/ui/img/logo.svg`) are not covered by the GPL: © Looking4Cache
  UG (haftungsbeschränkt), all rights reserved. Unmodified copies of Vitrine
  Studio may carry them; forks must use their own name and logo
  ([`assets/brand/README.md`](assets/brand/README.md)).
- **Bundled third-party data** keeps its own licence: the bundled fonts
  (Inter, Montserrat, Playfair Display, Lora, Nunito, Oswald, Bebas Neue,
  Caveat, Dancing Script, JetBrains Mono, and Noto script fallbacks) under
  the SIL Open Font License 1.1 (one `OFL.txt` per family in
  [`assets/fonts/`](assets/fonts/), sources and checksums in
  [`assets/fonts/SOURCES.md`](assets/fonts/SOURCES.md)), the hyphenation
  patterns under MIT/BSD/permissive notices
  ([`assets/hyphenation/LICENSES.md`](assets/hyphenation/LICENSES.md)).
  Rust dependencies are under permissive licences (MIT, Apache-2.0, BSD,
  ISC, Zlib, MPL-2.0, Unicode-3.0, …), all compatible with the GPL v3.
  Decision record: [ADR 0022](docs/adr/0022-licence-gpl-and-cla.md).
- **Gallery templates** are published under CC0 by their uploaders
  ([terms of use](https://www.vitrinestudio.app/terms/)).

## Open questions

- **Gallery gap widths** of the native store apps (iOS App Store on iPhone
  and iPad, Mac App Store, Play Store app) are estimates marked
  `not yet measured` in `crates/core/data/galleries.yaml`; only the store
  websites have been measured. They still have to be measured on real
  devices.
