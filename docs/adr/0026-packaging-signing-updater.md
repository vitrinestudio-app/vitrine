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

## Amendment 2026-11-12: more CLI builds and the container image

Owner decisions (STATUS open question g): Linux arm64 and a static musl
build of the CLI; the CLI container image is built and pushed by
`release.yml`.

### CLI-only archives (job `cli`)

| archive | target | built on | libc |
|---|---|---|---|
| `vitrine-cli_linux_aarch64.tar.gz` | `aarch64-unknown-linux-gnu` | GitHub `ubuntu-22.04-arm` runner (native; free for public repos) | glibc, same Ubuntu 22.04 baseline as `vitrine-cli_linux_x86_64` (glibc 2.35+) |
| `vitrine-cli_linux_x86_64-musl.tar.gz` | `x86_64-unknown-linux-musl` | `ubuntu-24.04`, `cargo zigbuild` | static (musl), any Linux, Alpine |
| `vitrine-cli_linux_aarch64-musl.tar.gz` | `aarch64-unknown-linux-musl` | `ubuntu-24.04-arm`, `cargo zigbuild` | static (musl) |

- No GUI for Linux arm64 (no AppImage/deb/rpm; `latest.json` unchanged,
  GUI only).
- musl via **cargo-zigbuild** (0.24.1) with **zig** (0.17.1) as C compiler
  and linker, both prebuilt and SHA-256 pinned in the workflow, instead of
  `cross` (needs Docker) or Ubuntu's `musl-tools` (a musl-gcc setup that
  cannot be tried on the Mac). The C parts (aws-lc-sys for rustls, bundled
  SQLite) build unchanged; no code or dependency changes were needed. The
  same tool chain builds both musl targets on a Mac (verified 2026-11-12).
- Native runners per architecture, so `scripts/release/build-cli.sh` runs
  the binary: `--version`, a new project, `validate --preview`, one
  rendered page; it also checks the linkage with `readelf` (musl: no
  dynamic section, no interpreter; glibc: NEEDED list and newest GLIBC
  symbol version).
- Names follow the existing `vitrine-cli_<os>_<arch>` scheme; the libc is
  a suffix of the arch (`x86_64-musl`), so the version-free names stay
  stable for `releases/latest/download/…`. All archives have the same
  layout (`vitrine-cli_<version>/{vitrine, licenses/}`) and are in
  `SHA256SUMS.txt`. `about.toml` / `deny.toml` list the three targets.
- musl's allocator is slower than glibc's under many threads; rendering
  in the musl build may be slower. Not measured yet (needs a Linux host);
  if it matters, a faster allocator for musl builds only is the fix.

### Container image `ghcr.io/vitrinestudio-app/vitrine-cli`

- Built from the **static musl archives** of the same run (identical
  binary to the download), `docker/cli/Dockerfile` target `release`:
  Alpine 3.24, `ca-certificates jq git`, non-root user `vitrine` (uid
  10001), `/work`, `VITRINE_APP_DIR`, licence files under
  `/usr/share/doc/vitrine/licenses/`, no ENTRYPOINT (CI runs shells).
  Alpine instead of distroless/static: GitLab, Jenkins and other CI
  systems need a shell in the image, and upload scripts need `jq`.
- linux/amd64 + linux/arm64, each built natively on its runner (no QEMU),
  smoke-tested (`docker run --network none`: version, new project,
  validate, render, licence file present), pushed by digest; job
  `image-manifest` joins the digests into one multi-arch index with tags
  `<version>`, `<major>.<minor>` and `latest` (pre-releases: only
  `<version>`).
- Only on tag pushes, with `GITHUB_TOKEN` (`packages: write` on the image
  jobs only; plain `docker buildx`, no third-party actions). Manual runs
  build and smoke-test without pushing. OCI labels: title, description,
  source (links the package to the repository), url, documentation,
  licenses `GPL-3.0-or-later`, version, revision; the same as index
  annotations.
- The image is pushed when the tag workflow runs, i.e. **before** the
  owner publishes the draft release. A discarded draft means deleting that
  image version on GitHub (Packages) as well.
- The default target of the Dockerfile still builds from source (Debian,
  glibc) for local images without a release.
