---
title: GitHub Actions
linkTitle: GitHub Actions
description: "Check store screenshots on every pull request with annotations, keep the images as build artifacts and upload them from a release workflow."
weight: 10
icon: github
---

Two workflows cover most projects: one checks and exports the screenshots on
every pull request, the other uploads them to the stores when you release.
How to install the CLI and what it offers CI in general:
[Automation](../../automation/).

## Checking and exporting on pull requests

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
          v=0.1.0
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

- `--format github` turns every finding into an annotation in the pull
  request, at the line in `project.yaml` that defines the element or
  target. GitHub shows at most 10 error and 10
  warning annotations per step. Errors come first; the full report is always
  in the log.
- `--strict` fails the job on warnings too (exit code 1).
- On an arm64 runner (`ubuntu-24.04-arm`) take
  `vitrine-cli_linux_aarch64.tar.gz`, on a macOS runner
  `vitrine-cli_macos_universal.tar.gz` – see
  [the archive list](../../automation/#installing-the-cli-in-ci).

## The planned action

{{< callout status >}}
The action is a **draft** and not published yet. Until then, use the install
step above.
{{< /callout >}}

[`vitrinestudio-app/vitrine-github-action`](https://github.com/vitrinestudio-app/vitrine-github-action)
will do the install step (checksum, caching; it picks the arm64 or static
build where needed) and the validate/export steps in one `uses:`:

```yaml
      - uses: vitrinestudio-app/vitrine-github-action@v1
        with:
          version: 0.1.0
          project: store-screens
          validate: true
          export: true
```

## Uploading from a release workflow

Store the App Store Connect key as repository or environment secrets and
pass them to `vitrine upload` as environment variables (all variables:
[Uploading from CI](../../automation/#uploading-from-ci)). An *environment* with required
reviewers makes a person approve every run before the job starts:

```yaml
name: Upload store screenshots
on: workflow_dispatch
jobs:
  upload:
    runs-on: ubuntu-latest
    environment: app-store          # required reviewers approve each run
    steps:
      - uses: actions/checkout@v5
      - name: Install vitrine
        run: |
          v=0.1.0
          base=https://github.com/vitrinestudio-app/vitrine/releases/download/v$v
          curl -sSfLO "$base/vitrine-cli_linux_x86_64.tar.gz"
          curl -sSfL "$base/SHA256SUMS.txt" | grep ' vitrine-cli_linux_x86_64.tar.gz$' | sha256sum -c -
          tar xzf vitrine-cli_linux_x86_64.tar.gz
          echo "$PWD/vitrine-cli_$v" >> "$GITHUB_PATH"
      - name: Upload
        env:
          VITRINE_ASC_KEY_ID: ${{ secrets.ASC_KEY_ID }}
          VITRINE_ASC_ISSUER_ID: ${{ secrets.ASC_ISSUER_ID }}
          VITRINE_ASC_KEY: ${{ secrets.ASC_KEY }}
        run: |
          vitrine --project store-screens upload plan --store apple --remote --format json > plan.json
          vitrine --project store-screens upload execute --confirm "$(jq -r .confirm_token plan.json)" --yes
      - uses: actions/upload-artifact@v4
        if: always()
        with:
          name: upload-plan
          path: plan.json
```

The plan step exits with 1 when the plan is blocked, so nothing is uploaded.
For Google Play, set `VITRINE_PLAY_SERVICE_ACCOUNT` and use
`--store google`.

## Device frames

The runner has no Apple bezels of its own. Store the ones your project uses
in the repository (private repositories only), or use the built-in frames –
see [Device frames in CI](../../automation/#device-frames-in-ci).
