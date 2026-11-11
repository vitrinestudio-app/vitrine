# Releasing Vitrine Studio

How releases are built and what the owner sets up once. Design and reasons:
ADR 0026. Workflow: `.github/workflows/release.yml`. Windows signing
research: `docs/research/2026-11-11-windows-signing.md`.

## Overview

- Push a tag `vX.Y.Z` → GitHub Actions builds macOS (universal .dmg + .app),
  Windows (NSIS setup) and Linux (AppImage, deb, rpm), the standalone CLI
  archives and the licence files, signs what the secrets allow and creates a
  **draft** release with `SHA256SUMS.txt`, `THIRD-PARTY-NOTICES.html` and
  `latest.json` (updater manifest). You publish the draft by hand.
- Actions → Release → "Run workflow" (`workflow_dispatch`) builds the same
  files as workflow artifacts only (no release); tick `sign` to use the
  signing secrets (test notarization before the first tag).
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

### 4. Windows: SignPath Foundation (when accepted)

Until this is set up, Windows builds are unsigned (SmartScreen: "More info"
→ "Run anyway") and labelled so. Conditions and alternatives: the research
note above. Before applying:

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

### 5. First test run

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
   SignPath requests if Windows signing is on.
5. Review the **draft release**: write the highlights at the top of the
   notes, check the signing lines, download one installer per OS.
6. Publish. From now on `releases/latest/download/latest.json` points to
   it and installed apps (with the real updater key) offer the update.
7. Website: once the first signed release is out, set
   `downloads_available = true` in `website/hugo.toml` and fill
   `[params.downloads]` with the stable links
   `https://github.com/vitrinestudio-app/vitrine/releases/latest/download/VitrineStudio_macos_universal.dmg`,
   `…/VitrineStudio_windows_x64-setup.exe`,
   `…/VitrineStudio_linux_x86_64.AppImage` (deb/rpm/CLI: link the release
   page).

A re-run of the workflow for the same tag replaces the files of the draft
and keeps edited notes. Never re-use a tag for different code; bump instead.

## Local bundle (macOS, unsigned)

```sh
scripts/release/third-party-notices.sh crates/gui/bundle/licenses   # needs cargo-about 0.9.3
scripts/release/build-cli-sidecar.sh aarch64-apple-darwin            # or universal-apple-darwin
cd crates/gui && cargo tauri build --target aarch64-apple-darwin --bundles app --config tauri.bundle.conf.json
```

`crates/gui/binaries/` and `crates/gui/bundle/` are generated (ignored by
Git). Add `--config tauri.updater.conf.json` only with
`TAURI_SIGNING_PRIVATE_KEY` set.
