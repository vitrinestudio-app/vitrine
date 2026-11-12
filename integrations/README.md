# CI integrations – DRAFTS

Nothing in this folder is published. Each integration needs a published
Vitrine Studio release first (CLI archives + `SHA256SUMS.txt` on GitHub
Releases, ADR 0026) and, for the container-based ones, the CLI image built
from `docker/cli/Dockerfile`. Research, ranking and the owner decisions:
`docs/research/2026-11-12-ci-integrations.md`.

| folder | what | install path | planned home (owner decision) |
|---|---|---|---|
| `github-action/` | composite action: install (checksum, cache, PATH), validate with annotations, export | CLI archive from GitHub Releases | own repo `vitrinestudio-app/setup-vitrine` → GitHub Marketplace |
| `gitlab/` | CI/CD components `validate` (Code Quality + JUnit) and `export` (artifact) | container image | gitlab.com project `vitrinestudio/vitrine-ci` → CI/CD Catalog |
| `fastlane-plugin/` | fastlane actions `vitrine_validate`, `vitrine_export` | `vitrine` on PATH | gem `fastlane-plugin-vitrine` on RubyGems |

All of them only call the `vitrine` CLI; the CLI enablers they rely on live
in this repository: `validate --format github|gitlab|junit [-o FILE]`,
`export --format json`, exit codes 0/1/2, store credentials from
`VITRINE_ASC_*` / `VITRINE_PLAY_*`, `VITRINE_APP_DIR`.
