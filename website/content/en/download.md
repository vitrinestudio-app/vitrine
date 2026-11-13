---
title: Download
eyebrow: "Download"
description: "Vitrine Studio 1.0 for macOS, Windows and Linux – installers, the command line tool and the source."
lead: "**Vitrine Studio 1.0** – free for macOS, Windows and Linux. All files, checksums and the command line archives are on [GitHub Releases](https://github.com/vitrinestudio-app/vitrine/releases/latest)."
lead_pending: "**Vitrine Studio 1.0.0** is being released. The downloads go live here with the release – until then you can build it from source."
---

{{< os-cards soon="With the 1.0.0 release" macos="Universal .dmg (Apple silicon and Intel), signed and notarised by Apple." windows="Setup (.exe) for your user account. Not code-signed yet – SmartScreen asks once: More info → Run anyway." linux="AppImage, .deb and .rpm (x86_64). Unsigned; verify with SHA256SUMS.txt." >}}

## Build from source

You need [rustup](https://rustup.rs) – the pinned Rust toolchain is installed
automatically – and, for the desktop app, the
[Tauri 2 prerequisites](https://v2.tauri.app/start/prerequisites/) of your
platform.

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

Both work on the same project folders. Releases are published on
[GitHub](https://github.com/vitrinestudio-app/vitrine/releases) with checksums
and the corresponding source. Installing, the `vitrine` command and updates:
[Install & update](/docs/install/).

## Licence

Vitrine Studio is free software under the GNU General Public License v3 or
later. The name "Vitrine Studio" and the logo are not covered by the GPL:
unmodified copies may carry them, forks must use their own name and icon.
