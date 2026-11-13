---
title: Install & update
description: "Install the app on macOS, Windows or Linux, make the vitrine command available in your terminal, keep it up to date – or build it from source."
weight: 20
icon: download
---

{{< callout status pending >}}
Version 1.0.0 is being released. The download buttons go live with the
release – until then, [build from source](#build-from-source). The steps
below describe the release builds.
{{< /callout >}}

## Download

Releases are published on
[GitHub Releases](https://github.com/vitrinestudio-app/vitrine/releases)
(and linked from the [download page](/download/)):

| Platform | App | Command line tool only |
|---|---|---|
| macOS 11 or later (Apple silicon and Intel) | `VitrineStudio_macos_universal.dmg` | `vitrine-cli_macos_universal.tar.gz` |
| Windows 10/11 (x64) | `VitrineStudio_windows_x64-setup.exe` | `vitrine-cli_windows_x64.zip` |
| Linux (x86_64) | `VitrineStudio_linux_x86_64.AppImage`, `…_amd64.deb`, `…_x86_64.rpm` | `vitrine-cli_linux_x86_64.tar.gz` |
| Linux (aarch64 / arm64) | – | `vitrine-cli_linux_aarch64.tar.gz` |
| Linux, static build (any distribution, Alpine) | – | `vitrine-cli_linux_x86_64-musl.tar.gz`, `vitrine-cli_linux_aarch64-musl.tar.gz` |

The "command line tool only" archives are meant for build servers and CI,
where you don't need the app. The regular Linux builds need glibc 2.35 or
newer (Ubuntu 22.04, Debian 12 and later); the static `-musl` builds have no
dependencies and run on any Linux, including Alpine. For CI there is also a
container image with the command line tool (amd64 and arm64):
`ghcr.io/vitrinestudio-app/vitrine-cli` – see [Automation](../automation/).

Every release also has `SHA256SUMS.txt` to check your download:

```sh
sha256sum -c SHA256SUMS.txt --ignore-missing
```

## Install

**macOS:** open the `.dmg` and drag *Vitrine Studio* into *Applications*.

**Windows:** run the setup. It installs for your user account only (no
administrator rights needed) and offers to add the `vitrine` command to your
PATH.

**Linux:** make the AppImage executable (`chmod +x`) and start it, or install
the `.deb` / `.rpm` with your package manager.

### "Unknown developer" warnings

The release notes say which builds are signed. A build that is not signed
(yet) shows a warning on first start:

- **macOS:** right-click the app → *Open*, or *System Settings → Privacy &
  Security → Open Anyway*.
- **Windows SmartScreen:** *More info* → *Run anyway*.

**About Windows:** the Windows version is not code-signed for now, so every
release shows this warning. Windows displays "Windows protected your PC"
with *Publisher: Unknown publisher* – that only means the setup carries no
certificate, not that something is wrong with it. If you like, check the
download against `SHA256SUMS.txt` first (`Get-FileHash` in PowerShell shows
the SHA-256), then choose *More info* → *Run anyway*. Updates installed from
inside the app are checked against Vitrine Studio's own update signature.

When you first start a newly signed version, macOS may ask once whether
Vitrine Studio may keep using its own keychain entries – allow it (an entry
is bound to the app's signature, which has changed). This is only about the
entries Vitrine Studio created itself: the store keys you added in the app
and, if you use it, your template gallery login (service
`app.vitrinestudio`). The app does not read any other entries
in your keychain.

## The command line tool

The `vitrine` command line tool ships inside the app – nothing extra to
download:

| Install | Where `vitrine` is |
|---|---|
| macOS app | `/Applications/Vitrine Studio.app/Contents/MacOS/vitrine` |
| Windows setup | `vitrine.exe` in the install folder |
| Linux deb/rpm | `/usr/bin/vitrine` (already on your PATH) |

To use it as plain `vitrine` in every terminal: **App settings → Command line
tool → Install command line tool…**

- **macOS** creates the link `/usr/local/bin/vitrine` (it asks for your
  administrator password if that folder is not writable).
- **Windows** adds the install folder to your user PATH – open a new terminal
  afterwards.
- **deb/rpm:** nothing to do.
- **AppImage:** not possible from inside the image; download the CLI archive
  instead.

It never replaces a different program called `vitrine`. The same works from a
terminal with the full path: `vitrine install-cli` (`--status` shows the state,
`--remove` undoes it, `--yes` skips the question).

## Updates

The app looks for a new version **once a day** when it starts (*App settings →
Updates → Check for updates once a day*, on by default) and whenever you choose
**Check for updates…** (☰ menu, the macOS app menu, or the app settings).
It only asks GitHub whether a newer version exists – no personal data, no
statistics.

When an update is available you choose **Later**, **Release notes** or
**Install and restart**. Nothing is installed without your click; the download
is checked against its signature first, then the app restarts (your project is
saved anyway – every edit is).

- macOS app, Windows setup and AppImage update themselves.
- deb/rpm installs only show the notice – install the new package from the
  release page.
- Builds you compiled yourself never look for updates.

## Where Vitrine Studio keeps its data

Projects live wherever you put them. The app's own data – settings, recent
projects, imported device frames, your templates – lives here:

| System | Folder |
|---|---|
| macOS | `~/Library/Application Support/app.vitrinestudio/` |
| Windows | `%APPDATA%\vitrinestudio\` (`config\` and `data\`) |
| Linux | `~/.config/vitrinestudio/` (settings) and `~/.local/share/vitrinestudio/` (data) |

Store keys are not in these folders – they are in the system keychain (see
[Store upload](../store-upload/)). To use a separate app data folder (for
tests or CI), set `VITRINE_APP_DIR=<dir>` or pass `--app-dir` where a command
offers it.

## Build from source

You need [rustup](https://rustup.rs) (the pinned Rust toolchain installs
itself) and, for the desktop app, the
[Tauri 2 prerequisites](https://v2.tauri.app/start/prerequisites/) of your
system.

```sh
git clone https://github.com/vitrinestudio-app/vitrine.git
cd vitrine
cargo run -p vitrine-gui --release              # the desktop app
cargo install --path crates/cli                  # the `vitrine` command line tool
vitrine export --project tests/fixtures/example  # try it on the example project
```

`tests/fixtures/example` is *Fernwood*, a fictional garden planner with six
phone pages, iPad, Mac, web and banner targets in English, German, French and
Canadian French – a good playground.

## Licence

Vitrine Studio is free software under the GNU GPL v3 or later. The app ships
its licence texts and third-party notices (macOS:
`Vitrine Studio.app/Contents/Resources/licenses/`); the source code of a release
is its Git tag. The name and logo are not covered by the GPL.
