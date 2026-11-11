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
from grouped portrait pages.

Website: <https://www.vitrinestudio.app>

## Status

**Pre-alpha. Phase 1 (core + command line) and Phase 2 (desktop editor)
are complete; Phase 3 (languages & import) is nearly done** – per-language
text overrides, font fallbacks, opt-in automatic hyphenation, screenshot
import (fastlane and folder layouts), translation exchange via CSV and
XLIFF, and imported Apple Product Bezels (still to be verified with the
real package). Projects can be created, designed, translated, validated
and exported in the desktop app, or headless with the `vitrine` command
line tool; AI assistants (e.g. Claude Desktop, Claude Code) can work on
projects through MCP – the desktop app hosts the server, so you watch the
assistant's edits live and can undo them; store
upload is not built yet. The full
concept (in German) is in [`docs/KONZEPT.md`](docs/KONZEPT.md), the task
lists in [`docs/plan/`](docs/plan/) (`phase-1.md` … `phase-4.md`, with
known gaps under "Follow-ups"), the current state in
[`docs/plan/STATUS.md`](docs/plan/STATUS.md).

### Desktop editor

One canvas per strip with the store gaps hatched, direct manipulation
(move, resize, rotate, snapping), an inspector per element kind, layers,
assets with drag & drop import, a translations table and inline text
editing, targets from store presets, a validation panel and an export
dialog. Every preview is rendered by the same Rust pipeline as the
export; every change is saved immediately as plain YAML.

![The editor with the example project](docs/gui/editor-dark.jpg)

![A text added from the Add menu and edited on the canvas](docs/gui/text-editing.jpg)

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
cargo run -p vitrine-cli -- --help  # command line interface (binary: vitrine)
cargo run -p vitrine-gui            # desktop app (start screen)
cargo run -p vitrine-gui -- tests/fixtures/example   # … opening a project
```

## Command line

Quick start:

```sh
cargo install --path crates/cli                # or: cargo run -p vitrine-cli --release -- …
vitrine new my-app --lang en --lang de        # a phone project from a template
# put screenshots into my-app/screenshots/en/ios-phone/<key>.png (see project.yaml)
vitrine validate --project my-app             # what is missing or does not fit
vitrine export --project my-app               # PNG/JPEG files in my-app/export/<target>/<lang>/
```

Try it on the example: `vitrine export --project tests/fixtures/example`.

The `vitrine` binary works on a project folder (`--project <DIR>`, default:
the current folder):

```sh
vitrine new my-app --lang en --lang de      # project from a template (--template phone|minimal)
vitrine validate --project my-app           # findings by target/language; --format json, --preview, --strict
vitrine render --project my-app --canvas    # preview PNG: --page N | --image N | --canvas | --gallery
vitrine render --project my-app --target play-phone --page 2 --width 400 -o page.png
vitrine export --project my-app             # all targets × languages into my-app/export/
vitrine export --target ios-iphone-69 --lang de --force   # --force ignores the incremental cache

# translations for translators (CSV or XLIFF 1.2/2.0; markup becomes protected inline tags)
vitrine strings export --format xliff20 --lang fr -o translations/   # --format csv --context --missing-only
vitrine strings import translations/fr.xlf --dry-run                 # report: changed/new/unknown keys, overflows

# screenshots from fastlane snapshot, a Vitrine Studio folder tree or a flat folder
vitrine import screenshots fastlane/screenshots --dry-run            # --device-map "iPhone 17 Pro Max=ios-phone" --save-mapping

# Apple Product Bezels (downloaded by you; stored app-wide, never in the project)
vitrine frames import /Volumes/Bezel-iPhone-18 --dry-run             # folder, zip or DMG
vitrine frames list
vitrine frames remove --model iphone-island                          # --color, --orientation; --all removes everything
```

Exit codes: `0` success (warnings fail only with `validate --strict`), `1`
the project has problems (validation errors, export failures), `2` the
command could not run (usage error, project not loadable, IO error).
`render` prints the path of the written PNG (default
`<project>/.cache/render/`). Colours are used on a terminal unless
`NO_COLOR` is set.

## AI assistants (MCP)

Vitrine Studio speaks [MCP](https://modelcontextprotocol.io), so any MCP-capable
assistant can create a project, add devices and texts, translate, look at
rendered previews (`render_preview` returns a PNG), check the layout,
validate and export. The desktop app hosts the server: the assistant works
on the project you have open, every change appears live (briefly outlined,
with a small note naming the client, e.g. "Claude Desktop: …"), is one undo
step labelled with the client's name, and you discuss it in your assistant.
Nothing goes over the network (ADR 0014, ADR 0015). The settings live under
App settings → *AI assistants (MCP)*.

**Any MCP client**: add a stdio server named `vitrine-studio` that runs the
`vitrine` command with the argument `mcp` (the exact path is shown under
*Other MCP clients*), e.g.

```json
{
  "mcpServers": {
    "vitrine-studio": { "command": "/path/to/vitrine", "args": ["mcp"] }
  }
}
```

Clients that speak Streamable HTTP can also use
`http://127.0.0.1:<port>/mcp` with the header `Authorization: Bearer <token>`
(both shown in the settings; they change per start of the app unless you set
a fixed port, the token always changes).

**Claude Desktop** (no terminal needed):

1. Start Vitrine Studio.
2. App settings → *AI assistants (MCP)* → **Add to Claude Desktop…** (shows
   the entry and the file, asks before writing, keeps a backup).
3. Restart Claude Desktop and ask it to work on your screenshots.

Claude Desktop then starts `vitrine mcp`, which connects to the running
app (and starts it when it is closed; setting *Start on demand*).

**Claude Code**: copy the command from the same settings section, e.g.

```sh
claude mcp add vitrine-studio -- /path/to/vitrine mcp      # recommended: always reaches the app
# or, for this run of the app only (port and token change per start):
claude mcp add --transport http vitrine-studio http://127.0.0.1:<port>/mcp --header "Authorization: Bearer <token>"
```

Without the app (CI, scripts): `vitrine mcp --headless --project
/path/to/my-app` works on the folder directly.

Example prompts: "Create a Vitrine Studio project in ~/Screens/Fernwood for a
garden planner app in English and German with three iPhone pages, then
show me the preview", "Translate all missing texts into French and check
that nothing overflows", "Make the title on page 2 smaller and move the
phone down by 100 lu". Tools, resources and the op reference:
[`docs/guide/NOTES.md`](docs/guide/NOTES.md) (MCP section).

## Workspace layout

| Path | Crate | Purpose |
|---|---|---|
| `crates/core` | `vitrine-core` | Project model, YAML IO, operations API, resolution, layout, validation, catalogs |
| `crates/render` | `vitrine-render` | Rasterization, parametric device frames, text rendering, export |
| `crates/publish` | `vitrine-publish` | App Store Connect and Google Play upload, keychain |
| `crates/app` | `vitrine-app` | Shared application layer: session, commands, preview cache, settings (GUI and MCP) |
| `crates/mcp` | `vitrine-mcp` | MCP server (rmcp): tools and resources over the application layer |
| `crates/cli` | `vitrine-cli` | Command line interface, binary `vitrine` |
| `crates/gui` | `vitrine-gui` | Tauri 2 desktop app, frontend in plain HTML/CSS/JS |
| `docs/` | | Concept and architecture decision records (`docs/adr/`) |

## Open questions

- **License: not yet decided.** Options are MIT OR Apache-2.0 or a copyleft
  license (MPL-2.0/GPL). Until this is settled there is no LICENSE file, no
  `license` field in the manifests, and all crates are `publish = false`.
  The bundled fonts (Inter) are under the SIL Open Font License
  (`assets/fonts/OFL.txt`).
- **Gallery gap widths** of the native store apps (iOS App Store on iPhone
  and iPad, Mac App Store, Play Store app) are estimates marked
  `not yet measured` in `crates/core/data/galleries.yaml`; only the store
  websites have been measured. They still have to be measured on real
  devices.
