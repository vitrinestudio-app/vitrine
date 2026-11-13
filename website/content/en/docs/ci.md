---
title: CI/CD
linkTitle: CI/CD
description: "Validate, export and upload store screenshots in GitHub Actions, GitLab CI, Jenkins, Bitrise, Codemagic, Xcode Cloud and others with the vitrine CLI."
weight: 107
icon: check
---

The `vitrine` command line tool runs headless in any CI. Use it to check a
project on every pull request, to export the images as build artifacts, or
to upload them to the stores from a release job.

{{< callout status >}}
The CLI archives and the container image come with the first release. The
ready-made integrations (GitHub Action, GitLab components, fastlane plugin)
are **drafts** and not published yet; everything below that uses only the
CLI or the image works with the first release.
{{< /callout >}}

## Installing the CLI in CI

Every release has a standalone CLI archive per platform, plus
`SHA256SUMS.txt`:

| runner | file |
|---|---|
| Linux x86_64 (glibc 2.35+, e.g. Ubuntu 22.04 or Debian 12 and newer) | `vitrine-cli_linux_x86_64.tar.gz` |
| Linux aarch64 / arm64 (glibc 2.35+, e.g. `ubuntu-24.04-arm`) | `vitrine-cli_linux_aarch64.tar.gz` |
| any Linux, static build (Alpine, older glibc, minimal images) | `vitrine-cli_linux_x86_64-musl.tar.gz`, `vitrine-cli_linux_aarch64-musl.tar.gz` |
| macOS (Apple silicon and Intel) | `vitrine-cli_macos_universal.tar.gz` |
| Windows x64 | `vitrine-cli_windows_x64.zip` |

```sh
v=0.2.0
base=https://github.com/vitrinestudio-app/vitrine/releases/download/v$v
curl -sSfLO "$base/vitrine-cli_linux_x86_64.tar.gz"
curl -sSfL "$base/SHA256SUMS.txt" | grep ' vitrine-cli_linux_x86_64.tar.gz$' | sha256sum -c -
tar xzf vitrine-cli_linux_x86_64.tar.gz
export PATH="$PWD/vitrine-cli_$v:$PATH"
vitrine --version
```

The binary needs no GUI libraries, no fonts from the system and no
keychain. Fonts, hyphenation patterns, device data and the built-in frames
are inside it. The `-musl` builds are fully static (no libraries at all).

### Container image

For container-based CI (GitLab, Jenkins, CircleCI, Buildkite, Azure, …)
every release also publishes the image
**`ghcr.io/vitrinestudio-app/vitrine-cli`** (linux/amd64 and linux/arm64):

| tag | |
|---|---|
| `0.2.0` | exactly this release (recommended for reproducible builds) |
| `0.2` | the newest `0.2.x` |
| `latest` | the newest release |

It is Alpine with the static `vitrine` binary, `git`, `jq`, CA certificates
and a POSIX `sh` (no `bash`), runs as the non-root user `vitrine` (uid
10001) in `/work`, and has no entrypoint, so CI systems can run their
shell in it. The licences are in `/usr/share/doc/vitrine/licenses/`.

```sh
docker run --rm -u "$(id -u):$(id -g)" -v "$PWD:/work" \
  ghcr.io/vitrinestudio-app/vitrine-cli:0.2.0 vitrine --project store-screens validate
```

## What the CLI offers CI

| | |
|---|---|
| **Exit codes** | **0** ok · **1** project problems (validation errors, warnings with `--strict`, export failures, blocked upload plan, failed upload) · **2** could not run (wrong arguments, project not found, invalid credentials) |
| **Annotations and reports** | `vitrine validate --format github` (GitHub annotations), `--format gitlab` (GitLab Code Quality), `--format junit` (JUnit XML for GitLab, Jenkins, Azure, CircleCI, Bitrise), `--format json`. With `-o FILE` the report goes to the file and the readable report to the log. Findings point at the line in `project.yaml` that defines the element or target |
| **Export list** | `vitrine export --format json` lists every file (path in the export folder, target, language, size, unchanged or written), for artifact uploads |
| **Credentials** | environment variables instead of the keychain (see [Uploading from CI](#uploading-from-ci)) |
| **App data** | `VITRINE_APP_DIR` points at an app data folder with frame packs and templates |

GitHub shows at most 10 error and 10 warning annotations per step. Errors
come first; the full report is always in the log.

## GitHub Actions

```yaml
name: Store screenshots
on:
  pull_request:
    paths: ["store-screens/**"]
jobs:
  validate:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v5
      - name: Install vitrine
        run: |
          v=0.2.0
          base=https://github.com/vitrinestudio-app/vitrine/releases/download/v$v
          curl -sSfLO "$base/vitrine-cli_linux_x86_64.tar.gz"
          curl -sSfL "$base/SHA256SUMS.txt" | grep ' vitrine-cli_linux_x86_64.tar.gz$' | sha256sum -c -
          tar xzf vitrine-cli_linux_x86_64.tar.gz
          echo "$PWD/vitrine-cli_$v" >> "$GITHUB_PATH"
      - run: vitrine --project store-screens validate --format github --strict
      - run: vitrine --project store-screens export --out export
      - uses: actions/upload-artifact@v4
        with:
          name: store-screenshots
          path: export
```

The planned action
[`vitrinestudio-app/vitrine-github-action`](https://github.com/vitrinestudio-app/vitrine-github-action)
will do the install step (checksum, caching; it picks the arm64 or static
build where needed) and the validate/export steps in one `uses:`:

```yaml
      - uses: vitrinestudio-app/vitrine-github-action@v1
        with:
          version: 0.2.0
          project: store-screens
          validate: true
          export: true
```

## GitLab CI

```yaml
vitrine-validate:
  image: ghcr.io/vitrinestudio-app/vitrine-cli:0.2.0
  script:
    - vitrine --project store-screens validate --format gitlab -o gl-code-quality-report.json
  artifacts:
    when: always
    reports:
      codequality: gl-code-quality-report.json

vitrine-export:
  image: ghcr.io/vitrinestudio-app/vitrine-cli:0.2.0
  script:
    - vitrine --project store-screens export --out vitrine-export --no-gallery
  artifacts:
    paths: [vitrine-export/]
```

Code Quality findings show in the merge request. Use `--format junit -o
vitrine-junit.xml` with `reports: junit:` for the pipeline's *Tests* tab.
Without the image, install the archive in `before_script` as shown above.

The planned CI/CD Catalog components in the gitlab.com project
[`vitrinestudio/vitrine-gitlab-ci`](https://gitlab.com/vitrinestudio/vitrine-gitlab-ci)
wrap these two jobs (`validate` with Code Quality + JUnit, `export`) and run
the same image:

```yaml
include:
  - component: $CI_SERVER_FQDN/vitrinestudio/vitrine-gitlab-ci/validate@1
    inputs:
      project: store-screens
```

## fastlane (Bitrise, Codemagic, CircleCI, local)

Most mobile pipelines run fastlane. Export straight into fastlane's
folders, and deliver/supply upload them unchanged (see
[Using with fastlane](../fastlane/)):

```ruby
lane :screenshots do
  sh("vitrine", "--project", "../store-screens", "validate", "--strict")
  sh("vitrine", "--project", "../store-screens", "export", "--layout", "fastlane", "--out", ".")
  upload_to_app_store(skip_binary_upload: true, skip_metadata: true, overwrite_screenshots: true)
end
```

(`sh` runs inside `fastlane/`, so `--out .` writes `fastlane/screenshots/`
and `fastlane/metadata/android/`.)

## Other systems

- **Jenkins:** run the steps in the container image
  (`agent { docker { image 'ghcr.io/vitrinestudio-app/vitrine-cli:0.2.0' } }`),
  or install the archive on the agent. Publish `vitrine validate --format
  junit -o vitrine-junit.xml` with the `junit` step.
- **Bitrise:** add a *Script* step that installs the archive (macOS or
  Linux stack), then call `vitrine`. For the test report, write the JUnit
  file to `$BITRISE_TEST_RESULT_DIR/vitrine/` with a `test-info.json` and
  add *Deploy to Bitrise.io*. Or use fastlane, as above.
- **Codemagic:** a `scripts:` entry that installs the archive. Map
  Codemagic's App Store Connect variables to ours:
  `VITRINE_ASC_KEY: $APP_STORE_CONNECT_PRIVATE_KEY`,
  `VITRINE_ASC_KEY_ID: $APP_STORE_CONNECT_KEY_IDENTIFIER`,
  `VITRINE_ASC_ISSUER_ID: $APP_STORE_CONNECT_ISSUER_ID`.
- **Xcode Cloud:** install the macOS archive in `ci_scripts/ci_post_clone.sh`
  (no `sudo`; put it in a folder of the workspace), then validate or export
  in `ci_post_xcodebuild.sh`.
- **CircleCI, Azure Pipelines, Buildkite:** use the container image or
  the archive. Use `store_test_results`, `PublishTestResults@2` (format
  JUnit) or Buildkite annotations for the JUnit report.

## Uploading from CI

On a CI runner there is no keychain and no app settings, so `vitrine upload`
also reads the store keys from environment variables. Set them from your
CI's secret store:

| store | variables |
|---|---|
| App Store Connect | `VITRINE_ASC_KEY_ID`, `VITRINE_ASC_ISSUER_ID`, and `VITRINE_ASC_KEY` (the `.p8` text, or base64 of it) **or** `VITRINE_ASC_KEY_PATH` (path of the `.p8` file) |
| Google Play | `VITRINE_PLAY_SERVICE_ACCOUNT` (the JSON, or base64 of it) **or** `VITRINE_PLAY_SERVICE_ACCOUNT_PATH` |

- When a store's variables are set, they are used **instead of** the
  account chosen in the project settings. The command prints which key it
  uses (key id or service account e-mail, never the key).
- Set all of them or none: if only some of a store's variables are set, the
  command stops (exit 2), so a misconfigured job never falls back to another account.
- Vitrine Studio never writes the keys to disk and never logs them. Only
  `vitrine upload` reads these variables; the app and the AI assistant
  connection (MCP) ignore them.

An upload still needs the plan's confirm token, and without a terminal also
`--yes`. Both steps run in the same job:

```sh
vitrine upload plan --remote --format json > plan.json        # exit 1 = blocked
vitrine upload execute --confirm "$(jq -r .confirm_token plan.json)" --yes
```

Keep the `plan.json` as an artifact: it records what was uploaded. If you
want a person to approve each upload, put that check in front of the job in
your CI: GitHub *environments* with required reviewers, or GitLab manual
jobs on protected branches.

## Device frames in CI

Apple's Product Bezels are imported on your own Mac, and their licence
doesn't allow passing them on – so the CI machine has none of its own. You
have two options:

- **Store the used bezels in the project** (Project settings → Frames, or
  `vitrine frames embed`). They are copied to `frames/imported/` and CI
  renders exactly the same images as your Mac. Only for **private**
  repositories – see [Apple bezels in CI](../devices-frames/#apple-bezels-in-ci).
- Otherwise `imported:apple` frames fall back to the built-in frame in CI,
  and validation reports `imported_frame_missing` (or
  `imported_frame_not_stored` locally). The built-in frames or a
  [frame pack](../devices-frames/) inside the project give identical
  results everywhere too.
