# 0026 – Packaging, signing and updates

- Status: accepted
- Date: 2026-11-11

## Context

Phase 6.5: Vitrine Studio needs installers for macOS, Windows and Linux,
the `vitrine` CLI inside them (Phase 4 TODO: "Add to Claude Desktop" needs
a stable CLI path), signing, third-party notices (ADR 0022) and an update
path. Owner decisions 2026-11-11: build as much as possible on GitHub
Actions; Apple Developer account available (Developer ID + notarization);
Windows: check free OSS signing via SignPath Foundation; Tauri updater yes
(signed manifests, the user confirms, can be switched off, no background
install).

## Decision

### Build: GitHub Actions + `cargo tauri build` directly

`.github/workflows/release.yml`: tag `v*` → draft GitHub release;
`workflow_dispatch` → the same builds as workflow artifacts (optionally
with the signing secrets, input `sign`). Jobs: `prepare` (tag must equal
the workspace version; licence files with cargo-about), `build` matrix,
`publish` (latest.json, `SHA256SUMS.txt`, notes, `gh release create
--draft`). The owner reviews and publishes the draft by hand.

We call the pinned Tauri CLI (2.12.4, prebuilt binary, SHA-256 checked)
ourselves instead of `tauri-apps/tauri-action` (v1.0.0): the Windows
SignPath path needs build → sign → bundle → sign → re-sign the updater
artifact, which tauri-action cannot interleave; owning `latest.json`
(written by `scripts/release/assemble.py`) keeps one code path for signed
and unsigned builds and stable, space-free asset names. Scripts in
`scripts/release/` also run locally.

### Bundles

- **macOS: one universal `.app`/`.dmg`** (aarch64 + x86_64, `--target
  universal-apple-darwin`): one download, one updater artifact for both
  architectures (`darwin-aarch64` and `darwin-x86_64` point at it). Cost:
  two compilations in CI. Minimum macOS 11.0.
- **Windows: NSIS `-setup.exe` only**, per-user install (`currentUser`, no
  admin, the updater runs it in passive mode). No MSI: one installer type,
  one updater key; MSI can be added later for managed deployments.
- **Linux: AppImage (self-updating), deb, rpm** on Ubuntu 22.04 (oldest
  glibc with WebKitGTK 4.1). deb/rpm installs do not self-update: the app
  only shows the notice with a link to the release page.
- Asset names without version (`VitrineStudio_macos_universal.dmg`,
  `VitrineStudio_windows_x64-setup.exe`, `VitrineStudio_linux_x86_64.AppImage`,
  `…_linux_amd64.deb`, `…_linux_x86_64.rpm`, `vitrine-cli_<platform>.tar.gz|zip`)
  so the website can link `releases/latest/download/<name>`.
- Release builds strip symbols (`CARGO_PROFILE_RELEASE_STRIP=true` in the
  workflow only).

### The CLI ships with the app

- Tauri sidecar (`bundle.externalBin: ["binaries/vitrine"]`), built by
  `scripts/release/build-cli-sidecar.sh` (universal: both archs + `lipo`).
  Locations: macOS `Vitrine Studio.app/Contents/MacOS/vitrine`, Windows
  `<install dir>\vitrine.exe`, deb/rpm `/usr/bin/vitrine`, AppImage inside
  the image. `claude_desktop::cli_path()` (next to the GUI binary) finds it
  without changes; in an AppImage a `vitrine` on `PATH` wins (the mount
  path changes every start). `vitrine mcp` resolves symlinks before looking
  for the GUI/app bundle.
- Bundle-only settings (`externalBin`, licence resources) live in
  `crates/gui/tauri.bundle.conf.json`, merged with `--config` at release
  time, so `cargo build`/`cargo test` never need the sidecar file.
- **"Install command line tool"** (`vitrine_app::cli_install`, app commands
  `cli_install_status|cli_install`, CLI `vitrine install-cli
  [--remove|--status] [--yes]`): macOS symlink `/usr/local/bin/vitrine`
  (password prompt via `osascript … with administrator privileges` only
  when the folder is not writable); Windows: install folder in the user
  `PATH` (`HKCU\Environment`, REG_EXPAND_SZ kept, via PowerShell, change
  broadcast) – the NSIS installer offers it on first install (hook
  `crates/gui/windows/installer-hooks.nsh` runs `vitrine install-cli
  --yes`; uninstall removes it); Linux deb/rpm: nothing to do; AppImage:
  not possible (standalone archive). Never replaces a foreign file.
  `VITRINE_CLI_LINK_DIR` / `AppState::with_cli_link_dir` redirect the
  symlink (tests, dev bridge).
- Standalone CLI archives per OS are attached to the release (macOS: the
  signed CLI copied out of the notarized app, so Gatekeeper's online check
  finds its ticket).

### macOS signing + notarization

Tauri's bundler signs inside-out (sidecar CLI, then the app) with the
**hardened runtime** and `crates/gui/macos/entitlements.plist`, which is
deliberately empty: the app is not sandboxed (projects are arbitrary
folders, the CLI shares them, the MCP proxy starts the GUI), so the
sandbox entitlements (network client, user-selected files) would have no
effect; `keyring` uses the legacy keychain (no access-group entitlement);
WKWebView needs no JIT exception. Certificate: Developer ID Application
from `APPLE_CERTIFICATE` (base64 .p12) + `APPLE_CERTIFICATE_PASSWORD`
(+ `APPLE_SIGNING_IDENTITY`); notarization with an App Store Connect API
key (`APPLE_API_ISSUER`, `APPLE_API_KEY` = key id, `APPLE_API_KEY_P8` =
key content, written to a temp file for Tauri's `APPLE_API_KEY_PATH`);
Tauri staples the app. Without secrets the build is unsigned (ad-hoc) and
labelled so.

### Windows signing: SignPath Foundation, prepared, off until accepted

Research and conditions: `docs/research/2026-11-11-windows-signing.md`.
The workflow switches to the SignPath path when the variable
`SIGNPATH_ORGANIZATION_ID` and the secret `SIGNPATH_API_TOKEN` exist (and
the run signs): two signing requests (binaries before NSIS packaging, then
the installer; SignPath cannot open NSIS), the bundle-type marker patched
by us, the updater signature re-created after Authenticode. Artifact
configuration drafts: `.signpath/artifact-configurations/`. Until then
Windows builds are unsigned and labelled. Alternative if SignPath does not
work out: Azure Artifact Signing with the company (Tauri `signCommand`).

### Updater

- `tauri-plugin-updater` 2.14.1, called from Rust only
  (`crates/gui/src/updater.rs`; the web view keeps `core:default`).
  Endpoint `https://github.com/vitrinestudio-app/vitrine/releases/latest/download/latest.json`
  (drafts and pre-releases are not "latest"); `requireSignedVersion: true`
  (the version is part of the minisign trusted comment, so a forged
  manifest cannot downgrade); Windows `installMode: passive`.
- Public key in `tauri.conf.json` → `plugins.updater.pubkey`. Until the
  owner generates the key it is a **placeholder**, and the app offers no
  updates at all (`Reason::NotConfigured`). `createUpdaterArtifacts` is
  only enabled by `tauri.updater.conf.json` when `TAURI_SIGNING_PRIVATE_KEY`
  is set (Tauri fails the build otherwise).
- UI: "Check for updates…" (in-app menu + macOS app menu + app settings),
  automatic check once a day at start (setting `update_check`, default on;
  `update_checked_at`), dialog Later / Release notes / Install and restart
  (download progress, then restart). Offered only for installed bundles
  (.app, NSIS/MSI, AppImage); deb/rpm get the notice + download link;
  debug/dev builds nothing. No telemetry beyond one GET.

### Licence compliance

- `deny.toml` (allow-list from ADR 0022) + CI job `licenses` (`cargo deny
  check licenses`, prebuilt 0.20.2, SHA-256 checked).
- `about.toml` + `scripts/release/about.hbs` → `THIRD-PARTY-NOTICES.html`
  (cargo-about 0.9.3, `--fail` on unknown licences), plus `LICENSE.txt`, the
  fonts' `OFL.txt` files and `hyphenation/LICENSES.md`
  (`scripts/release/third-party-notices.sh`) bundled as resources under
  `licenses/` (macOS `Contents/Resources/licenses/`), in every CLI archive,
  and the notices attached to the release. Release notes name the tag as
  the corresponding source.

### Version

The workspace version (`Cargo.toml` `[workspace.package] version`, now
0.1.0) is the single source: crates inherit it, Tauri takes it from the GUI
crate (no `version` in `tauri.conf.json`), `vitrine --version`, `app_info`
(start screen) and the updater use it; the release job fails when the tag
is not `v<version>`.

## Consequences

- Releasing = bump version, commit, tag `vX.Y.Z`, push the tag, review the
  draft, publish (`docs/dev/releasing.md`).
- Until the owner adds the secrets: macOS builds are not notarized
  (Gatekeeper warning), Windows builds unsigned (SmartScreen warning), and
  the updater stays off (placeholder key). All clearly labelled.
- The updater key must never be lost: without it no update can reach
  existing installs (a new key needs a manual reinstall).
- Bundles are large (~100 MB unpacked per architecture; the CLI and GUI
  each embed the fonts and catalogs); the universal app doubles that.
- Not verified locally: notarization, DMG creation on CI, NSIS hooks, the
  SignPath path, AppImage/deb/rpm – first `workflow_dispatch` run checks
  them.
