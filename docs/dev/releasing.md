# Releasing Vitrine Studio

How releases are built and what the owner sets up once. Design and reasons:
ADR 0026. Workflow: `.github/workflows/release.yml`. Windows signing
research: `docs/research/2026-11-11-windows-signing.md`.

## Overview

- Push a tag `vX.Y.Z` → GitHub Actions builds macOS (universal .dmg + .app),
  Windows (NSIS setup) and Linux (AppImage, deb, rpm), the standalone CLI
  archives (macOS, Windows, Linux x86_64 + aarch64 with glibc, Linux
  x86_64 + aarch64 static musl) and the licence files, signs what the
  secrets allow and creates a **draft** release with `SHA256SUMS.txt`,
  `THIRD-PARTY-NOTICES.html` and `latest.json` (updater manifest). You
  publish the draft by hand.
- The same tag run pushes the CLI container image
  `ghcr.io/vitrinestudio-app/vitrine-cli` (`X.Y.Z`, `X.Y`, `latest`;
  amd64 + arm64, from the musl archives). It is pushed **before** you
  publish the draft: if you discard a draft, also delete that image
  version (GitHub → organisation → Packages → vitrine-cli → versions).
- Actions → Release → "Run workflow" (`workflow_dispatch`) builds the same
  files as workflow artifacts only (no release; the image is built and
  smoke-tested, not pushed); tick `sign` to use the signing secrets (test
  notarization before the first tag).
- Builds without secrets succeed unsigned and say so in the release notes.

## One-time setup (owner)

Secrets and variables go into the **environment `release`**, not into
repository or organization secrets (the `build` job declares
`environment: release`):

1. GitHub → repository → Settings → Environments → New environment
   `release`.
2. Deployment branches and tags: "Selected branches and tags" → add the
   branch `main` and the tag pattern `v*`. Other branches, forks and pull
   requests then never get the signing keys.
3. Optional: "Required reviewers" (yourself) – every release/test build
   waits for your click before the secrets are used.
4. Add all secrets and variables below under this environment
   ("Environment secrets" / "Environment variables").

Never commit keys; never paste them into issues or logs.

### 1. macOS: Developer ID certificate

1. On the Mac with Xcode or Keychain Access: developer.apple.com →
   Certificates → "+" → **Developer ID Application** (needs the Account
   Holder role). Create the CSR with Keychain Access (Certificate Assistant
   → Request a Certificate from a Certificate Authority, saved to disk),
   upload it, download the `.cer`, double-click to add it to the login
   keychain.
2. Keychain Access → My Certificates → right-click "Developer ID
   Application: Looking4Cache UG (…) (TEAMID)" → Export → `.p12` with a
   strong password.
3. `base64 -i DeveloperID.p12 | pbcopy` → secret **`APPLE_CERTIFICATE`**.
4. The export password → secret **`APPLE_CERTIFICATE_PASSWORD`**.
5. The identity name exactly as in Keychain Access, e.g.
   `Developer ID Application: Looking4Cache UG (haftungsbeschränkt) (ABCDE12345)`
   → secret **`APPLE_SIGNING_IDENTITY`** (`security find-identity -v -p codesigning`
   lists it).
6. Delete the `.p12` file afterwards.

### 2. macOS: notarization key (App Store Connect API)

1. App Store Connect → Users and Access → Integrations → App Store Connect
   API → Team Keys → "+" → name "Vitrine notarization", access
   **Developer** → download `AuthKey_<KEYID>.p8` (only once!).
2. Secrets: **`APPLE_API_ISSUER`** (Issuer ID shown above the key list),
   **`APPLE_API_KEY`** (the Key ID), **`APPLE_API_KEY_P8`** (the whole
   content of the `.p8` file, including the BEGIN/END lines).
3. Keep the `.p8` in a password manager, delete the download.

(This is not the store-upload key users configure in the app; a separate key
with the Developer role is enough for notarization.)

### 3. Updater signing key (Tauri)

1. Generate on your machine (not in CI):
   ```sh
   cargo tauri signer generate -w ~/.tauri/vitrine-updater.key
   ```
   Choose a password. It prints the **public key** and writes
   `vitrine-updater.key` (private) + `.key.pub`.
2. Put the public key (the single base64 line from `.key.pub`) into
   `crates/gui/tauri.conf.json` → `plugins.updater.pubkey`, replacing the
   `REPLACE_WITH_TAURI_UPDATER_PUBLIC_KEY …` placeholder, and commit it.
   While the placeholder is there the app never offers updates.
3. Secrets: **`TAURI_SIGNING_PRIVATE_KEY`** (content of
   `vitrine-updater.key`), **`TAURI_SIGNING_PRIVATE_KEY_PASSWORD`**.
4. Back the private key up (password manager + offline copy). **If it is
   lost, existing installations can never update again** (users would have
   to reinstall a build with a new key).

### 4. Windows: unsigned for now (SignPath deferred)

**Owner decision 2026-11-13:** no Windows code-signing certificate for
now. Windows releases ship unsigned; nothing has to be set up for them:

- Leave `SIGNPATH_API_TOKEN` and the `SIGNPATH_*` variables unset. The
  workflow then builds the Windows setup on the normal path, still signs
  the updater artifact (`…-setup.exe.sig`, entry `windows-x86_64` in
  `latest.json`) and writes "Windows: unsigned (SmartScreen warns on first
  start)" into the release notes.
- Users see SmartScreen's "Windows protected your PC" / "Unknown
  publisher" on the downloaded setup and continue with **More info → Run
  anyway** (website: Install & update → "Unknown developer" warnings).
- In-app updates keep working: the Tauri updater verifies its own
  signature (the updater key, §3), not an Authenticode signature. A later
  switch to signed builds needs no change in installed apps.

The SignPath path below stays in the workflow, dormant, for when the
decision is revisited. Conditions and alternatives: the research note
above. Before applying:

- Publish a first (unsigned) release; SignPath wants software that is
  already released and has some reputation.
- Add a **"Code signing policy"** section on the website (home page and the
  download page must show that term, as heading or link) with: "Free code
  signing provided by SignPath.io, certificate by SignPath Foundation",
  the committers/reviewers/approvers (link to the GitHub org members or a
  team), and privacy info (link to the privacy policy, or the SignPath
  standard sentence plus a note on the update check and store uploads).
- Enable MFA for every team member on GitHub (and later SignPath).
- Mind the clause "without commercial dual-licensing" (ADR 0022 keeps the
  option through the CLA): ask SignPath when applying.

Then:

1. Apply at https://signpath.org/apply (project: Vitrine Studio, repo
   github.com/vitrinestudio-app/vitrine, licence GPL-3.0-or-later, download
   page).
2. After acceptance, in SignPath: add the predefined trusted build system
   **GitHub.com** to the organisation and link it to the project; create the
   artifact configurations from `.signpath/artifact-configurations/`
   (`binaries.xml`, `installer.xml` – drafts, check them in SignPath's
   editor); create a signing policy (release signing, manual approval;
   allowed refs: tags `v*` – check that SignPath accepts tag refs, otherwise
   build releases from a `release/*` branch); create a CI user / API token
   with submitter rights.
3. GitHub: secret **`SIGNPATH_API_TOKEN`**; variables
   **`SIGNPATH_ORGANIZATION_ID`**, **`SIGNPATH_PROJECT_SLUG`**,
   **`SIGNPATH_POLICY_SLUG`**, **`SIGNPATH_ARTIFACT_CONFIG_BINARIES`**,
   **`SIGNPATH_ARTIFACT_CONFIG_INSTALLER`**. Setting
   `SIGNPATH_ORGANIZATION_ID` switches the Windows job to the SignPath path.
4. Each release then waits for two approvals in SignPath (binaries, then
   installer); the job waits up to an hour per request.
5. Open item for SignPath's product-name/version rule: the CLI
   (`vitrine.exe`) has no Windows version resource yet (add one with a
   build script, e.g. `winresource`, then enforce `product-name` /
   `product-version` in `binaries.xml` for it too).

Alternative without SignPath: Azure Artifact Signing (~$10/month,
company validation); Tauri calls it via `bundle.windows.signCommand`.

### 5. Container image (after the first tag run)

The first push creates the package `vitrine-cli` in the organisation. Once:

1. GitHub → organisation → Packages → `vitrine-cli` → Package settings →
   **Change visibility → Public** (new packages start private; CI users
   pull without login).
2. Check that the package shows "linked to vitrinestudio-app/vitrine" (the
   `org.opencontainers.image.source` label does that) and that "Manage
   Actions access" lists the repository with **Write** (needed for later
   pushes from the workflow's `GITHUB_TOKEN`; it is set automatically when
   the workflow created the package).
3. Try it: `docker run --rm ghcr.io/vitrinestudio-app/vitrine-cli:X.Y.Z vitrine --version`
   (also on an arm64 machine).

### 6. First test run

1. Actions → Release → Run workflow (branch `main`), tick `sign`.
2. Download the artifacts `vitrine-studio-macos|windows|linux` and
   `release-preview` (latest.json, checksums, notes).
3. Check: the DMG opens without a Gatekeeper warning
   (`spctl -a -vvv -t install "Vitrine Studio.app"` → "Notarized Developer
   ID"; `xcrun stapler validate "Vitrine Studio.app"`), the CLI
   `"/Applications/Vitrine Studio.app/Contents/MacOS/vitrine" --version`,
   App settings → Command line tool → Install, App settings → Updates shows
   the version (no updates yet: no release), Windows setup installs per
   user and offers the PATH entry, the AppImage starts.
4. The artifacts `vitrine-studio-linux-aarch64`, `…-linux-x86_64-musl`,
   `…-linux-aarch64-musl` hold the extra CLI archives; the `cli` job log
   shows the linkage check ("static: no NEEDED entries") and the smoke
   test, the `image` jobs the container smoke test.

## Releasing a version

1. Make sure `main` is green (CI incl. the `licenses` job).
2. Bump the version in `Cargo.toml` → `[workspace.package] version`
   (single source: crates, Tauri bundle, `vitrine --version`, About/start
   screen, updater). Run `cargo build` so `Cargo.lock` follows; commit
   "Release vX.Y.Z".
3. Tag and push the tag:
   ```sh
   git tag -a vX.Y.Z -m "Vitrine Studio X.Y.Z"
   git push origin vX.Y.Z
   ```
   The workflow fails early when the tag and the version differ.
4. Wait for the workflow (~45–60 min, macOS is the slowest). Approve the
   SignPath requests if Windows signing is on (not the case for now, §4).
5. Review the **draft release**: write the highlights at the top of the
   notes, check the signing lines, download one installer per OS.
6. Publish. From now on `releases/latest/download/latest.json` points to
   it and installed apps (with the real updater key) offer the update.
7. Website: once the first release is published, set
   `downloads_available = true` in `website/hugo.toml` and fill
   `[params.downloads]` with the stable links
   `https://github.com/vitrinestudio-app/vitrine/releases/latest/download/VitrineStudio_macos_universal.dmg`,
   `…/VitrineStudio_windows_x64-setup.exe`,
   `…/VitrineStudio_linux_x86_64.AppImage` (deb/rpm/CLI: link the release
   page).

A re-run of the workflow for the same tag replaces the files of the draft
and keeps edited notes. Never re-use a tag for different code; bump instead.

## CI integrations (separate repositories, MIT)

Sources live in `integrations/` (each folder self-contained); copy a folder's
content to the root of its repository when it changed:

- `integrations/github-action/` → `vitrinestudio-app/vitrine-github-action`;
  tag `v1.x.y` and move the `v1` tag; Marketplace listing from the release
  page. Its `Test` workflow runs against the latest Vitrine Studio release.
- `integrations/gitlab/` → gitlab.com `vitrinestudio/vitrine-gitlab-ci`
  (CI/CD Catalog project); bump the default `image` tag
  (`ghcr.io/vitrinestudio-app/vitrine-cli:X.Y`) when a new minor release
  changes the CLI, then tag `X.Y.Z` there.
- `integrations/fastlane-plugin/` → gem `fastlane-plugin-vitrine`.

## Local CLI builds for Linux (from the Mac)

```sh
rustup target add x86_64-unknown-linux-musl aarch64-unknown-linux-musl
brew install zig && cargo install cargo-zigbuild   # once
scripts/release/build-cli.sh x86_64-unknown-linux-musl   # → target/x86_64-unknown-linux-musl/release/vitrine
scripts/release/build-cli.sh aarch64-unknown-linux-musl
```

The smoke test only runs on a Linux host of the same architecture; with a
`readelf` on `PATH` (e.g. the messense cross toolchain's
`x86_64-unknown-linux-gnu-readelf` linked as `readelf`) the static check
runs on the Mac too. Container image from a release archive:

```sh
mkdir -p dist/image && tar xzf vitrine-cli_linux_x86_64-musl.tar.gz -C dist/image --strip-components=1
docker build --target release -f docker/cli/Dockerfile -t vitrine-cli .
```

## Local bundle (macOS, unsigned)

```sh
scripts/release/third-party-notices.sh crates/gui/bundle/licenses   # needs cargo-about 0.9.3
scripts/release/build-cli-sidecar.sh aarch64-apple-darwin            # or universal-apple-darwin
cd crates/gui && cargo tauri build --target aarch64-apple-darwin --bundles app --config tauri.bundle.conf.json
```

`crates/gui/binaries/` and `crates/gui/bundle/` are generated (ignored by
Git). Add `--config tauri.updater.conf.json` only with
`TAURI_SIGNING_PRIVATE_KEY` set.
