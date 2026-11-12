# CI integrations – DRAFTS

Nothing in this folder is published yet. Each integration needs a published
Vitrine Studio release first (CLI archives + `SHA256SUMS.txt` on GitHub
Releases, ADR 0026) and, for the container-based ones, the CLI image
`ghcr.io/vitrinestudio-app/vitrine-cli` (built and pushed by
`.github/workflows/release.yml` from the static musl archives). Research,
ranking and the owner decisions: `docs/research/2026-11-12-ci-integrations.md`.

| folder | what | install path | home (owner decision 2026-11-12) |
|---|---|---|---|
| `github-action/` | composite action: install (checksum, cache, PATH; Linux x64/arm64 glibc or static musl, macOS, Windows), validate with annotations, export | CLI archive from GitHub Releases | repo `vitrinestudio-app/vitrine-github-action` (folder content = repo root) → GitHub Marketplace |
| `gitlab/` | CI/CD components `validate` (Code Quality + JUnit) and `export` (artifact) – GitLab-only | container image | gitlab.com project `vitrinestudio/vitrine-gitlab-ci` → CI/CD Catalog |
| `fastlane-plugin/` | fastlane actions `vitrine_validate`, `vitrine_export` | `vitrine` on PATH | gem `fastlane-plugin-vitrine` on RubyGems (repo open) |

Every folder is self-contained (no references to files outside it), so its
content can be copied to the root of its own repository. Licence: **MIT**
for all three (`LICENSE` in each folder; ADR 0022 amendment) – they only
run the `vitrine` program, which stays GPL-3.0-or-later.

For other CI systems (Jenkins, Buildkite, CircleCI, Azure Pipelines, …) the
generic integration is the container image or the release archives; the
website page `docs/ci` has snippets.

All of them only call the `vitrine` CLI; the CLI enablers they rely on live
in this repository: `validate --format github|gitlab|junit [-o FILE]`,
`export --format json`, exit codes 0/1/2, store credentials from
`VITRINE_ASC_*` / `VITRINE_PLAY_*`, `VITRINE_APP_DIR`.
