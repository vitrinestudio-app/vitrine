# Vitrine Studio CLI – GitHub Action

Install the [`vitrine`](https://vitrinestudio.app) command line tool in a
GitHub Actions workflow, validate a Vitrine Studio project with annotations
and export the App Store / Play Store screenshots.

> **Status: draft.** The action needs a published Vitrine Studio release
> with the CLI archives (`vitrine-cli_*.tar.gz|zip` and `SHA256SUMS.txt` on
> [GitHub Releases](https://github.com/vitrinestudio-app/vitrine/releases)).
> Its home is the repository
> [`vitrinestudio-app/vitrine-github-action`](https://github.com/vitrinestudio-app/vitrine-github-action)
> (GitHub Marketplace); the source is maintained in
> [`integrations/github-action/`](https://github.com/vitrinestudio-app/vitrine/tree/main/integrations/github-action)
> of the main repository and copied there.

A composite action (bash only, no Node runtime) for Linux x64 and arm64
(also in Alpine containers), macOS (arm64/x64) and Windows runners. It

1. downloads the `vitrine` CLI of the requested release from GitHub
   Releases, **verifies its SHA-256** against the release's
   `SHA256SUMS.txt`, caches it (`actions/cache`) and puts it on `PATH`.
   Linux picks the build that fits the runner: `vitrine-cli_linux_x86_64` /
   `_aarch64` (glibc 2.35+, e.g. the Ubuntu runners) or the static
   `…-musl` build (Alpine and other non-glibc containers, older glibc);
   input `libc` overrides it;
2. optionally runs `vitrine validate --format github` – errors and warnings
   become annotations on the run and on the pull request's `project.yaml`
   lines;
3. optionally runs `vitrine export` and reports the export folder;
4. optionally runs any other `vitrine` command (`args`).

## Inputs

| input | default | |
|---|---|---|
| `version` | `latest` | release version (`0.1.0` or `v0.1.0`); pin it for reproducible builds |
| `repository` | `vitrinestudio-app/vitrine` | where the releases are |
| `cache` | `true` | cache the downloaded CLI |
| `libc` | `auto` | Linux: `auto`, `gnu` or `musl` (static build) |
| `project` | `.` | project folder (with `project.yaml`) |
| `validate` | `false` | validate with annotations; errors fail the step |
| `strict` | `false` | with `validate`: warnings fail too |
| `preview` | `false` | with `validate`: missing screenshots are warnings |
| `export` | `false` | export all targets × languages |
| `layout` | `standard` | with `export`: `standard` or `fastlane` |
| `out` | – | with `export`: output folder |
| `args` | – | another `vitrine` command, split at spaces |

Outputs: `version`, `bin-dir`, `export-dir`, `export-report` (the
`export --format json` report: every file with target, language, size).

## Examples

### Validate on every pull request

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
      - uses: vitrinestudio-app/vitrine-github-action@v1
        with:
          version: 0.1.0
          project: store-screens
          validate: true
          strict: true
```

GitHub shows at most 10 error and 10 warning annotations per step; the
full report is in the step log.

### Export and keep the images as an artifact

```yaml
      - uses: vitrinestudio-app/vitrine-github-action@v1
        id: vitrine
        with:
          project: store-screens
          export: true
      - uses: actions/upload-artifact@v4
        with:
          name: store-screenshots
          path: ${{ steps.vitrine.outputs.export-dir }}
```

### fastlane: export into fastlane's folders, then deliver / supply

```yaml
  screenshots:
    runs-on: macos-latest
    steps:
      - uses: actions/checkout@v5
      - uses: vitrinestudio-app/vitrine-github-action@v1
        with:
          project: store-screens
          export: true
          layout: fastlane
          out: fastlane          # → fastlane/screenshots/<locale>/, fastlane/metadata/android/…
      - run: bundle exec fastlane deliver --skip_binary_upload --skip_metadata --overwrite_screenshots
        env:
          APP_STORE_CONNECT_API_KEY_KEY_ID: ${{ secrets.ASC_KEY_ID }}
          APP_STORE_CONNECT_API_KEY_ISSUER_ID: ${{ secrets.ASC_ISSUER_ID }}
          APP_STORE_CONNECT_API_KEY_KEY: ${{ secrets.ASC_KEY_P8 }}
```

### Upload with Vitrine Studio itself

Credentials come from environment variables (no keychain in CI). An upload
always needs the plan's confirm token and `--yes`; protect the job with an
environment that needs a reviewer if you want a human in the loop.

```yaml
  upload:
    runs-on: ubuntu-latest
    environment: store            # e.g. with required reviewers
    steps:
      - uses: actions/checkout@v5
      - uses: vitrinestudio-app/vitrine-github-action@v1
      - name: Upload screenshots
        working-directory: store-screens
        env:
          VITRINE_ASC_KEY_ID: ${{ secrets.ASC_KEY_ID }}
          VITRINE_ASC_ISSUER_ID: ${{ secrets.ASC_ISSUER_ID }}
          VITRINE_ASC_KEY: ${{ secrets.ASC_KEY_P8 }}
          VITRINE_PLAY_SERVICE_ACCOUNT: ${{ secrets.PLAY_SERVICE_ACCOUNT_JSON }}
        run: |
          vitrine upload plan --remote --format json > plan.json   # exit 1 = blocked
          vitrine upload execute --confirm "$(jq -r .confirm_token plan.json)" --yes
```

## Notes

- Apple's Product Bezels can't be used in CI: their licence doesn't allow
  redistributing them, so they are not in any image or cache of ours.
  Projects that use `imported:apple` frames fall back to the parametric
  frame (info `imported_frame_missing`). Use built-in frames or a frame pack
  inside the project (`frames/`) for CI renders.
- Linux arm64 runners (`ubuntu-24.04-arm`) get the aarch64 build.
- Alpine (or other musl) containers: the action's steps use `bash`, `curl`,
  `tar` and (for `export`) `jq`; install them first, e.g.
  `apk add --no-cache bash curl tar jq`. The static musl build is chosen
  automatically. A container job can also use the image
  `ghcr.io/vitrinestudio-app/vitrine-cli` instead of this action.
- Security: the action passes inputs through environment variables, never
  into the script text. Pin `actions/cache` (and this action) to a commit
  SHA when publishing.
- Tests: `.github/workflows/test.yml` runs the action on Linux x64/arm64,
  macOS, Windows and in an Alpine container against a published release.

## Licence

MIT (see `LICENSE`). The action only downloads and calls the `vitrine`
program; Vitrine Studio itself is licensed under the GNU GPL v3 or later,
and its release archives carry their own licence files.
