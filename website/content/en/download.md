---
title: Download
eyebrow: "Download"
description: "Vitrine Studio for macOS, Windows and Linux – installers are coming soon; build from source in the meantime."
lead: "Vitrine Studio is in **pre-alpha**. Signed installers for macOS, Windows and Linux are being prepared – until then you can build it from source."
---

{{< os-cards soon="Coming soon" macos="Planned: signed and notarised .dmg." windows="Planned: signed installer." linux="Planned: AppImage and .deb." >}}

## Build from source

You need [rustup](https://rustup.rs) – the pinned Rust toolchain is installed
automatically – and, for the desktop app, the
[Tauri 2 prerequisites](https://v2.tauri.app/start/prerequisites/) of your
platform. No Node.js or npm is needed.

```sh
git clone https://github.com/vitrinestudio-app/vitrine.git
cd vitrine
cargo run -p vitrine-gui --release                       # the desktop app
cargo install --path crates/cli                           # the `vitrine` command line tool
vitrine export --project tests/fixtures/example           # try it on the example project
```

## What you get

- **The desktop app** (`vitrine-gui`) – the editor with its built-in MCP server for AI assistants.
- **The command line tool** (`vitrine`) – validate, render, export, translate, import and upload from scripts and CI.

Both work on the same project folders. Releases will be published on
[GitHub](https://github.com/vitrinestudio-app/vitrine) with checksums and the
corresponding source.

## Licence

Vitrine Studio is free software under the GNU General Public License v3 or
later. The name "Vitrine Studio" and the logo are not covered by the GPL:
unmodified copies may carry them, forks must use their own name and icon.
