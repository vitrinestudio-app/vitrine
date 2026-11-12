# setup-vitrine (GitHub Action) – DRAFT

> **Draft, not published.** This action works only once Vitrine Studio has a
> published GitHub release with the CLI archives (`vitrine-cli_*.tar.gz|zip`
> and `SHA256SUMS.txt`, ADR 0026). For the Marketplace it has to move to its
> own public repository with `action.yml` at the root – proposed name
> `vitrinestudio-app/setup-vitrine` (owner decision). The examples below use
> that name.

A composite action (runs on Linux x64, macOS arm64/x64 and Windows runners)
that

1. downloads the `vitrine` CLI of the requested release from GitHub
   Releases, **verifies its SHA-256** against the release's
   `SHA256SUMS.txt`, caches it (`actions/cache`) and puts it on `PATH`;
2. optionally runs `vitrine validate --format github` – errors and warnings
   become annotations on the run and on the pull request's `project.yaml`
   lines;
3. optionally runs `vitrine export` and reports the export folder;
4. optionally runs any other `vitrine` command (`args`).

## Inputs

| input | default | |
|---|---|---|
| `version` | `latest` | release version (`0.2.0` or `v0.2.0`); pin it for reproducible builds |
| `repository` | `vitrinestudio-app/vitrine` | where the releases are |
| `cache` | `true` | cache the downloaded CLI |
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
      - uses: vitrinestudio-app/setup-vitrine@v1
        with:
          version: 0.2.0
          project: store-screens
          validate: true
          strict: true
```

GitHub shows at most 10 error and 10 warning annotations per step; the
full report is in the step log.

### Export and keep the images as an artifact

```yaml
      - uses: vitrinestudio-app/setup-vitrine@v1
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
      - uses: vitrinestudio-app/setup-vitrine@v1
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
      - uses: vitrinestudio-app/setup-vitrine@v1
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
- Linux arm64 runners: no CLI build yet (owner decision); use an x64 runner.
- Security: the action passes inputs through environment variables, never
  into the script text. Pin `actions/cache` (and this action) to a commit
  SHA when publishing.
- Local test of the scripts: they were run against a fake release
  (`file://` URLs) on macOS, including a checksum mismatch.
