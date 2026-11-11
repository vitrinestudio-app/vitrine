# Windows code signing for Vitrine Studio (research, 2026-11-11)

Question from the owner (2026-11-11): free code signing for open source via
the **SignPath Foundation**? What does the project need, how does it fit
GitHub Actions and the Tauri NSIS installer, and what are the alternatives?
Facts collected from the sources listed below on 2026-11-11; anything not
confirmed is marked *(unverified)*.

## Short answer

- SignPath Foundation signs OSS releases for free, built on GitHub-hosted
  runners from the public repo, with a certificate **issued to "SignPath
  Foundation"** (that name is the publisher users see, not Vitrine Studio).
- Vitrine Studio meets the technical side (OSI licence GPL-3.0-or-later,
  public repo, builds on GitHub Actions, no malware, privacy-friendly), but
  three conditions need the owner's attention **before applying**:
  1. **"without commercial dual-licensing"**: SignPath requires an
     OSI licence "without commercial dual-licensing for all components".
     Our CLA (ADR 0022) lets the owner relicense, e.g. for a commercial
     edition. As long as no commercial licence is actually offered this is
     probably fine, but ask SignPath when applying and do not announce a
     dual licence while using their certificate.
  2. **Already released + reputation**: the project "must already be
     released in the form that should be signed", and executables need "a
     certain verifiable reputation". A first unsigned public release (and
     some users/stars/downloads) is likely needed before acceptance.
  3. **Code signing policy page** on the website (see below) and MFA for
     all team members on GitHub and SignPath.
- The release workflow is prepared: Windows builds stay **unsigned and are
  labelled so** until the repository variables `SIGNPATH_*` and the secret
  `SIGNPATH_API_TOKEN` exist; then the SignPath path runs automatically.

## SignPath Foundation conditions (signpath.org/terms)

- Licence: OSI-approved, no commercial dual-licensing, for all components.
  No proprietary components of the maintainers; system libraries and
  unmodified upstream OSS binaries are allowed (unsigned DLLs inside a
  signed installer are OK; sign as much as possible).
- Actively maintained, already released in the form to be signed, function
  described on the download page.
- Reputation for executables (checked once at the start); acceptance at
  SignPath's discretion.
- No malware/PUP, no security-circumvention features; system changes need
  warnings; an uninstaller must exist (NSIS has one).
- Privacy: if data goes to systems the user did not choose, a privacy
  policy shown at install time plus an opt-out. Vitrine Studio only talks to
  the stores the user configures, the gallery when opened, and GitHub for
  the update check (switchable in the app settings) – the standard
  statement below fits, plus a sentence about the update check.
- Team: the signing team develops the project and owns the repo. Roles:
  authors/committers, reviewers (all outside changes reviewed),
  approvers (every signing request approved manually in SignPath).
  **MFA** for everyone on GitHub and SignPath.
- Every signature attests "a valid, automated build resulting from the
  source code at the noted source code repository"; build scripts and CI
  config count as source.
- **Code signing policy** on the project home page, with the exact term
  "Code signing policy" on the home page and the download/release page
  (heading or link). It must contain "Free code signing provided by
  SignPath.io, certificate by SignPath Foundation", list committers/
  reviewers/approvers (link to people or the GitHub team), and privacy
  information: either a privacy policy link or "This program will not
  transfer any information to other networked systems unless specifically
  requested by the user or the person installing or operating it."
- Artifact configuration must enforce product name (= project name) and
  version.
- Violations can lead to revocation (also retroactive).

## Application

- Form: https://signpath.org/apply (HubSpot form; fields could not be read
  without a browser – expect repository, licence, release/download page,
  reputation, team).
- Turnaround: no official figure; reports say 1–2 weeks, then integration
  (one Tauri app reported "a few weeks" until signing worked) *(anecdotal)*.

## GitHub Actions integration

- Action `SignPath/github-action-submit-signing-request` – current tag
  **v3.0** (`v3`, commit 2026-09-10; v2.3 was the last v2). Inputs used:
  `api-token`, `organization-id`, `project-slug`, `signing-policy-slug`,
  `artifact-configuration-slug`, `github-artifact-id`,
  `wait-for-completion`, `wait-for-completion-timeout-in-seconds`,
  `output-artifact-directory` (also: `connector-url`, `parameters`,
  `skip-decompress`, timeouts). Outputs: `signing-request-id`,
  `signing-request-web-url`, `signed-artifact-download-url`.
- The unsigned files must be uploaded first with `actions/upload-artifact`
  (its `artifact-id` output is passed on). The upload is a ZIP, so the
  artifact configuration has a `<zip-file>` root.
- Origin verification ("trusted build system" GitHub.com, added in the
  SignPath organisation and linked to the project): SignPath checks that the
  build ran in a GitHub workflow on **GitHub-hosted runners**, takes repo,
  branch/ref, commit and job URL from GitHub, and the signing policy can
  restrict branches (e.g. `main`, `release/*`). Whether tag refs
  (`refs/tags/v*`) are accepted as "branch" is *(unverified)* – check when
  setting up the policy.
- **NSIS installers cannot be deep-signed** (SignPath can open msi, cab,
  appx/msix, zip, nupkg, jar, directories – not NSIS). So the app exe and
  the CLI must be signed *before* the installer is built from them, and
  the installer in a second request (two manual approvals per release).

## What this means for the Tauri build (implemented in release.yml)

1. `cargo tauri build --no-bundle` (exe + sidecar CLI).
2. Patch the bundle-type marker `__TAURI_BUNDLE_TYPE_VAR_UNK` →
   `…_NSS` in the exe ourselves (Tauri normally does it while bundling,
   which would break an earlier signature; the updater reads it).
3. Upload exe + CLI → SignPath request 1 (`binaries.xml`).
4. `cargo tauri bundle --bundles nsis --no-binary-patching --no-sign` from
   the signed files.
5. Upload the setup → SignPath request 2 (`installer.xml`).
6. Re-create the updater signature of the signed setup:
   `cargo tauri signer sign --app-version <v> <setup.exe>` (Authenticode
   changes the file, so the `.sig` Tauri made earlier would be invalid).

Unsigned after this: the NSIS uninstaller and NSIS plugin DLLs (allowed by
the terms as upstream OSS). Tauri's `bundle.windows.signCommand` (string or
`{cmd, args}` with `%1`) cannot call SignPath per file, because SignPath
only signs artifacts uploaded by the workflow run.

## Alternatives

| Option | Cost | Notes |
|---|---|---|
| SignPath Foundation | free | publisher shown as "SignPath Foundation"; conditions above; manual approval per release |
| Azure Artifact Signing (formerly Trusted Signing) | Basic $9.99/month (5,000 signatures) | organisations in US, CA, EU, UK and more; individuals only US/CA; organisation validation (legal entity, business id, website, own-domain e-mail, Verified ID of the representative), 1–20 business days; no EV; Tauri supports it directly via `signCommand` (`artifact-signing-cli`), so no workflow split. Looking4Cache UG (haftungsbeschränkt) as a German company should qualify *(check)*. Publisher = the company name. |
| OV code-signing certificate | ~$150–300/year | key must live on an HSM/token since June 2023 (cloud HSM or USB token, hard in CI); publisher = company |
| EV certificate | $400+/year | since 2024 no instant SmartScreen reputation any more (same as OV) – not worth the extra cost |
| Microsoft Store (MSIX) | free | the Store signs; no SmartScreen warnings; separate packaging and store process |

SmartScreen reputation builds up per signing identity over downloads with
any of these; unsigned builds always warn ("More info" → "Run anyway").

**Recommendation:** ship the first public releases unsigned (clearly
labelled), apply to SignPath once there is a release and some users;
if the SignPath conditions (dual-licensing clause) are a problem or the
application is declined, use Azure Artifact Signing with the company
(~€10/month, simplest CI integration).

## Sources (accessed 2026-11-11)

- SignPath Foundation terms: https://signpath.org/terms
- Application: https://signpath.org/apply
- GitHub action: https://github.com/SignPath/github-action-submit-signing-request (action.yml, tags)
- Trusted build system GitHub: https://docs.signpath.io/trusted-build-systems/github
- Origin verification: https://docs.signpath.io/origin-verification
- Artifact configuration reference/examples: https://docs.signpath.io/artifact-configuration/reference , https://docs.signpath.io/artifact-configuration/examples
- Tauri Windows signing: https://v2.tauri.app/distribute/sign/windows/
- Tauri updater: https://v2.tauri.app/plugin/updater/
- Microsoft code signing options: https://learn.microsoft.com/windows/apps/package-and-deploy/code-signing-options
- Azure Artifact Signing: https://azure.microsoft.com/en-us/products/artifact-signing , https://learn.microsoft.com/en-us/azure/artifact-signing/quickstart
- Tauri + SignPath examples: https://github.com/NeuralNomadsAI/CodeNomad/pull/1257 , https://github.com/lkasdorf/SEPA-Validator/pull/37 , https://tabularis.dev/blog/signpath-code-signing
- Turnaround report: https://rubentalstra.github.io/Trial-Submission-Studio/development/windows-signing.html
