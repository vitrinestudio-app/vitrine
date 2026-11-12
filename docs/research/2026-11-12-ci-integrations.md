# CI/CD integrations for the `vitrine` CLI

Researched 2026-11-12 (owner request: which CI/CD systems matter for mobile
and desktop app teams, and how the CLI should integrate with them). All web
pages were read on 2026-11-12; claims that could not be checked on a
primary source are marked *(unverified)*.

## Summary and recommendation

- **What matters:** GitHub Actions leads everywhere, then GitLab CI and
  Jenkins. Mobile-only services (Bitrise, Codemagic, Xcode Cloud) are niche
  as the *main* CI but common as a second one, and all of them run
  **fastlane**. fastlane is the glue that mobile teams already have.
- **Build first (in this order):**
  1. **GitHub Action** `setup-vitrine` (composite; it installs the CLI
     archive from the release with a checksum check, caches it, validates
     with annotations and exports). It reaches the most users, costs little
     to maintain, and the Marketplace makes it easy to find.
  2. **Container image** `vitrine-cli` plus **GitLab CI/CD components**
     (`validate` with Code Quality + JUnit reports, `export`). The image
     also serves Jenkins, Buildkite, CircleCI and Azure without any
     per-system package.
  3. **fastlane plugin** `fastlane-plugin-vitrine` (`vitrine_validate`,
     `vitrine_export`). This one plugin covers Bitrise, Codemagic, Xcode
     Cloud and local lanes, the systems where a dedicated step would cost
     most per user.
- **Later, only on demand:** Bitrise step, CircleCI orb, Azure DevOps
  extension, Buildkite plugin. Each has its own publishing process and
  review, for a small audience. Until then, documented snippets around the
  CLI and the image are enough.
- **CLI enablers** (done in this repository; they help every system):
  - store credentials from environment variables;
  - `validate --format github|gitlab|junit [-o FILE]`;
  - stable exit codes 0/1/2;
  - `export --format json`;
  - a headless Linux binary and a Dockerfile.

## Market relevance (mobile CI)

| source | finding |
|---|---|
| Runway, *2025 State of Mobile Release Management* (300 senior mobile engineers, US/UK companies with 500–10,000 staff, Jan–Feb 2025) – <https://get.runway.team/hubfs/PDFs/2025StateOfMobileReleaseManagementReport.pdf>, <https://www.runway.team/report/pain-points-risk-of-status-quo-releases> | main CI/CD solution: **GitHub Actions 32 %**, **Jenkins 24 %**; both are also the most-used second tools. Per-tool numbers for the rest are in a chart image; a search snippet gives Bitrise 5 % main / 10 % secondary, Codemagic 2 % / 13 % *(unverified)* |
| JetBrains, *State of CI/CD 2026* (general, 598 respondents) – <https://www.jetbrains.com/pages/state-of-cicd2026/> | organisational use 2026 (2025): **GitHub Actions 47 % (41 %)**, **GitLab CI 32 % (34 %)**, **Jenkins 28 % (31 %)**, Azure DevOps Server 15 % (14 %), TeamCity 10 %, AWS CodePipeline/Build 7 %, Bitbucket Pipelines 6 %, Google Cloud Build 4 %; Bitrise, Buildkite and Travis appear without values |
| Stack Overflow Developer Survey 2025 – <https://survey.stackoverflow.co/2025/technology/> | no CI question in the visible results; GitLab (35.6 %) and Azure DevOps (16.6 %) appear only as collaboration tools |
| Bitrise Mobile Insights 2025 (vendor build data, >10 M builds) – via <https://hackernoon.com/bitrise-mobile-report-new-benchmarks-and-28percent-faster-build-times> | no vendor shares; 30 % of builds are cross-platform (React Native ≈ 26 %) *(secondary source)* |
| Expo, *Best CI/CD for mobile apps* (Oct 2026) – <https://expo.dev/blog/best-ci-cd-for-mobile-apps> | store submission on GitHub Actions and GitLab is "usually with Fastlane"; GitLab's macOS images ship fastlane |
| fastlane – <https://github.com/fastlane/fastlane>, <https://opencollective.com/fastlane/updates/monthly-releases> | 42.4k stars, releases 2.241.x (Oct 2026); the project moved to the Mobile Native Foundation (started late 2023, done early 2026). No survey measures fastlane's share; the evidence is indirect (it is preinstalled on Codemagic, GitLab's macOS images and Bitrise stacks) |

The surveys agree on the order for the main CI: GitHub Actions > Jenkins /
GitLab CI > the rest. Mobile-specific services are "second CI" tools.
Xcode Cloud and CircleCI show up in neither survey.

## Per system

Maintenance cost is the work after the first version: (low) follows our
releases automatically; (medium) a second repo or registry with its own
release step; (high) its own publishing and review process, a runtime to
keep up to date (Node handlers etc.), or vendor certification.

| system | relevance | idiomatic integration | install path | caching | report formats it reads | our plan | maintenance |
|---|---|---|---|---|---|---|---|
| **GitHub Actions** | #1 | action repo with `action.yml` at the root. JavaScript and composite actions run on Linux, macOS and Windows; Docker actions are Linux only ([docs](https://docs.github.com/en/actions/concepts/workflows-and-actions/custom-actions)). Marketplace: public repo, one action per repo, unique name, 2FA ([docs](https://docs.github.com/en/actions/how-tos/create-and-publish-actions/publish-in-github-marketplace)) | CLI archive from GitHub Releases + `SHA256SUMS.txt` | `actions/cache` keyed by version + asset, in `RUNNER_TOOL_CACHE` | workflow commands `::error file=,line=,title=::`. At most 10 errors, 10 warnings and 10 notices per step and 50 per job ([toolkit docs](https://raw.githubusercontent.com/actions/toolkit/HEAD/docs/problem-matchers.md)); SARIF via code scanning | **composite action** (draft `integrations/github-action/`), repo `setup-vitrine` | low: bash only, no Node runtime to bump; asset names are stable (ADR 0026) |
| **GitLab CI** | #2 (general), common in-house | CI/CD components: `templates/<name>.yml` with `spec: inputs`, README required, catalog project, published by a `release` job on a semver tag ([docs](https://docs.gitlab.com/ci/components/)) | container image | the image is cached by the runner | `artifacts:reports:junit` (all tiers), `codequality` (MR widget on all tiers, diff annotations need Ultimate; [docs](https://docs.gitlab.com/ci/testing/code_quality/)), `sarif` (Ultimate only, security tab) | **components `validate` + `export`** (draft `integrations/gitlab/`) | medium: second project on gitlab.com, release tags in step with ours |
| **Jenkins** | #2–3 | plugins are Java and heavy; the usual way is a Docker agent (`agent { docker { image … } }`) plus the JUnit plugin | container image (or the Linux archive) | Docker layer cache | JUnit (`junit` step); Warnings NG reads SARIF, JUnit and its native format ([formats](https://github.com/jenkinsci/warnings-ng-plugin/blob/main/SUPPORTED-FORMATS.md)) | **docs snippet only** (image + `validate --format junit`) | none |
| **Bitrise** | mobile-specific, ~5–10 % *(unverified)* | step in the StepLib: `step.yml` with `source_code_url`/`support_url`, one PR per version, must be maintained ([docs](https://docs.bitrise.io/en/steps-and-workflows/developing-your-own-bitrise-step/sharing-steps-with-all-bitrise-users.html)) | archive (macOS/Linux stacks) | Bitrise cache steps | JUnit in `$BITRISE_TEST_RESULT_DIR/<sub>/` with `test-info.json`, then "Deploy to Bitrise.io" ([docs](https://docs.bitrise.io/en/bitrise-ci/testing/deploying-and-viewing-test-results)) | **later**. Until then: fastlane plugin, or a script step that downloads the archive | high: StepLib PR per release |
| **Codemagic** | mobile-specific, mostly Flutter | no plugin marketplace; `codemagic.yaml` scripts, YAML anchors, variable groups; fastlane preinstalled ([docs](https://docs.codemagic.io/yaml-basic-configuration/yaml-getting-started)) | archive in a script step (macOS M2 / Linux) | `cache:` paths in `codemagic.yaml` | JUnit via `test_report` *(unverified)* | **docs snippet**; credentials map from its ASC variables `APP_STORE_CONNECT_PRIVATE_KEY`, `_KEY_IDENTIFIER`, `_ISSUER_ID` ([docs](https://docs.codemagic.io/yaml-publishing/app-store-connect/)) | none |
| **Xcode Cloud** | Apple-only, no public share | `ci_scripts/ci_post_clone.sh`, `ci_pre_xcodebuild.sh`, `ci_post_xcodebuild.sh` ([docs](https://developer.apple.com/documentation/xcode/writing-custom-build-scripts)); Homebrew available, no `sudo` ([docs](https://developer.apple.com/documentation/xcode/making-dependencies-available-to-xcode-cloud)); no Docker (macOS only) | archive downloaded in `ci_post_clone.sh` (or Homebrew tap later) | none between builds | none for third-party tools | **docs snippet** (validate/export after the build; upload better in another CI) | none |
| **CircleCI** | low in both surveys | orbs: immutable semver releases in a namespace; tiers certified / partner / community ([concepts](https://circleci.com/docs/orbs/use/orb-concepts/)) | archive or image | `save_cache` | JUnit via `store_test_results` *(unverified this session)* | **later** (orb); snippet with the image meanwhile | medium |
| **Azure Pipelines** | 15 % (Azure DevOps Server) | extension with `task.json` (Node 20 handler or PowerShell), `vss-extension.json`, `tfx extension publish`, verified publisher for public listing ([docs](https://learn.microsoft.com/en-us/azure/devops/extend/develop/add-build-task)) | archive (Windows/macOS/Linux agents) or container job | `Cache@2` | `PublishTestResults@2` with JUnit | **docs snippet**; extension only on demand | high: Node handler updates, publisher verification |
| **Buildkite** | small, larger companies | plugin repo `*-buildkite-plugin` with `plugin.yml` + `hooks/`, listed via the `buildkite-plugin` topic ([docs](https://buildkite.com/docs/pipelines/integrations/plugins/writing)) | image (docker plugin) | agent-local | annotations (`buildkite-agent annotate`), Test Engine *(unverified)* | **docs snippet** | low–medium |

Not researched: Appcircle, Travis CI, Expo EAS (EAS Build runs custom
`npx`/shell steps; `vitrine` would be an archive download there too).

## What CI users need from us – and where it stands

| need | status |
|---|---|
| install without the app, verifiable | CLI archives per OS + `SHA256SUMS.txt` on every release (ADR 0026). Gaps: **no Linux arm64** and **no musl/static** build (Alpine); owner decision |
| headless, no GUI libraries | checked 2026-11-12: a Linux build of `vitrine` needs only `libc`, `libm`, `libpthread`, `libdl`, `librt`, `libgcc_s` and `ld-linux` (readelf of a cross build; that build needs `GLIBC_2.28`, the
release built on Ubuntu 22.04 may need up to its glibc 2.35, so: glibc
distributions from Ubuntu 22.04 / Debian 12 on, not Alpine). Fonts, hyphenation, catalogs and parametric frames are embedded; the keychain crate talks D-Bus in pure Rust (zbus) and only runs when used; SQLite is bundled; TLS uses rustls |
| container image | `docker/cli/Dockerfile` (multi-stage, `debian:bookworm-slim`, non-root uid 10001, `ca-certificates jq git`, no ENTRYPOINT so GitLab/Jenkins can run shells, `VITRINE_APP_DIR` set). Not built here (no Docker on the dev machine). For releases, build the image from the published Linux archive, so the image is the same binary and carries the third-party notices |
| machine-readable results | `validate --format json`, `export --format json` (lists every file with path relative to `out_dir`, target, language, size and whether it was skipped as unchanged, so a whole `out_dir` can be uploaded as an artifact), `upload plan --format json` |
| native CI reports | **new:** `validate --format github` (annotations, errors first, plus the text report), `--format gitlab` (Code Quality; error → `critical`, warning → `minor`, info → `info`; SHA-256 fingerprints), `--format junit` (suite per target, failure per error, warnings as `<skipped>` or failures with `--strict`), `-o FILE` (report to file, text to stdout). Findings point at the `project.yaml` line of their element or target |
| stable exit codes | 0 ok · 1 project problems (validation errors, `--strict` warnings, export failures, blocked plan, failed upload) · 2 could not run (usage, project not loadable, credentials invalid) |
| credentials without a keychain | **new:** `VITRINE_ASC_KEY_ID`, `VITRINE_ASC_ISSUER_ID`, `VITRINE_ASC_KEY` (text or base64) or `VITRINE_ASC_KEY_PATH`; `VITRINE_PLAY_SERVICE_ACCOUNT` (JSON or base64) or `VITRINE_PLAY_SERVICE_ACCOUNT_PATH`. Only `vitrine upload` reads them; they win over the project's app-wide account and need no keychain; all-or-nothing per store; never written to disk or logged (ADR 0016 amendment) |
| explicit upload opt-in | unchanged and enough: `upload plan` → token; `upload execute --confirm <token>` needs `--yes` without a terminal (exit 2 otherwise). Human approval belongs to the CI system (GitHub environments with reviewers, GitLab manual/protected jobs) (ADR 0018 amendment) |
| frames in CI | `VITRINE_APP_DIR` (CLI, all commands) points at an app data folder with frame packs/templates; `--app-dir` exists for `upload` and `mcp`. **Apple's Product Bezels must not be shipped** (their licence forbids redistribution, `docs/research/2026-11-11-vendor-frames.md`): CI uses parametric frames or the project's own frame packs (`frames/` in the project, ADR 0024). Projects using `imported:apple` render with the parametric fallback and the info `imported_frame_missing`, so validate in CI with `--strict` only if that info is acceptable |

## How fastlane fits

There are three levels, and each builds on the one before:

1. **Done (2026-11-10): the layout export.** `vitrine export --layout
   fastlane --out fastlane` writes deliver's `screenshots/<locale>/` and
   supply's `metadata/android/<locale>/images/…`, and the import reads
   snapshot/screengrab output. That works today on every CI with one `sh`
   line in the lane and no plugin.
2. **Draft: `fastlane-plugin-vitrine`.** It adds `vitrine_validate` and
   `vitrine_export`. It only calls the CLI and sets
   `DELIVER_SCREENSHOTS_PATH` / `SUPPLY_METADATA_PATH` plus the lane
   context. It is worth publishing because fastlane's plugin list is where
   frameit users look for a replacement, and it reaches Bitrise, Codemagic
   and Xcode Cloud users without a step for each. The cost is a gem release
   per CLI interface change (rare, because it uses only stable flags and
   the JSON reports).
3. **Optional later: `vitrine_upload`.** It would map fastlane's
   `app_store_connect_api_key` / `SUPPLY_JSON_KEY_DATA` to our
   `VITRINE_*` variables and run plan + execute. Not drafted, because
   deliver/supply already upload in fastlane setups. Vitrine's own upload is
   only better for Duo/creative assets and incremental sets.

fastlane's own variable names (checked in the source):

- `APP_STORE_CONNECT_API_KEY_KEY_ID`, `_ISSUER_ID`, `_KEY`, `_KEY_FILEPATH`
  and `_IS_KEY_CONTENT_BASE64`
  ([app_store_connect_api_key.rb](https://raw.githubusercontent.com/fastlane/fastlane/master/fastlane/lib/fastlane/actions/app_store_connect_api_key.rb))
- `SUPPLY_JSON_KEY` and `SUPPLY_JSON_KEY_DATA`
  ([supply/options.rb](https://raw.githubusercontent.com/fastlane/fastlane/master/supply/lib/supply/options.rb))

We deliberately do **not** read these names implicitly (ADR 0016
amendment). The docs show the mapping in one line per variable.

## Snippets for systems without a package

The docs page `website/content/*/docs/ci.md` carries the snippets for
Jenkins, Bitrise, Codemagic, Xcode Cloud, CircleCI, Azure Pipelines and
Buildkite. They use either the release archive (download, `sha256sum -c`,
extract) or the container image.

## Owner decisions

1. **Where and under what names to publish:**
   - GitHub Action repo `vitrinestudio-app/setup-vitrine` (Marketplace name
     "Setup Vitrine Studio CLI");
   - image `ghcr.io/vitrinestudio-app/vitrine-cli` (tags `<version>`,
     `latest`; built in `release.yml` from the Linux archive);
   - gitlab.com project `vitrinestudio/vitrine-ci` for the components;
   - gem `fastlane-plugin-vitrine` (own repo, licence MIT vs.
     GPL-3.0-or-later).
2. **CI upload policy:** keep plan + `execute --confirm <token> --yes`
   (implemented, recommended), or also add a one-step `vitrine upload run
   --yes`. That step is convenient but skips the reviewable plan.
   Recommendation: no `run` for now.
3. **More CLI builds:** a Linux arm64 archive (`ubuntu-24.04-arm` runners
   are free for public repos) and/or a static musl build for Alpine-based
   CI images.
4. **Later integrations** (Bitrise step, CircleCI orb, Azure extension):
   only when users ask for them.
5. **fastlane variable names:** confirm that we don't read
   `APP_STORE_CONNECT_API_KEY_*` implicitly.
