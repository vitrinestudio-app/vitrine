# Integrations

Status (2026-11-19): the GitHub Action is released (`v1.0.0`, `v1`; its
`Test` workflow passes against the 1.0.0 release; listed on the GitHub
Marketplace as "Vitrine Studio CLI", https://github.com/marketplace/actions/vitrine-studio-cli); the fastlane plugin is released (gem 1.0.0, https://rubygems.org/gems/fastlane-plugin-vitrine,
repo `vitrinestudio-app/fastlane-plugin-vitrine` tagged `v1.0.0`; rspec +
rubocop in its CI, run in a real lane against the 1.0.0 CLI); the GitLab components are a draft; the agent skill is online
(see the `README.md` integrations table). Each CI integration needs a
published Vitrine Studio release (1.0.0 or later) (CLI archives + `SHA256SUMS.txt` on GitHub
Releases, ADR 0026) and, for the container-based ones, the CLI image
`ghcr.io/vitrinestudio-app/vitrine-cli` (built and pushed by
`.github/workflows/release.yml` from the static musl archives). Research,
ranking and the owner decisions: `docs/research/2026-11-12-ci-integrations.md`.

| folder | what | install path | home (owner decision 2026-11-12) |
|---|---|---|---|
| `github-action/` | composite action: install (checksum, cache, PATH; Linux x64/arm64 glibc or static musl, macOS, Windows), validate with annotations, export | CLI archive from GitHub Releases | repo `vitrinestudio-app/vitrine-github-action` (folder content = repo root) → GitHub Marketplace |
| `gitlab/` | CI/CD components `validate` (Code Quality + JUnit) and `export` (artifact) – GitLab-only | container image | gitlab.com project `vitrinestudio/vitrine-gitlab-ci` → CI/CD Catalog |
| `fastlane-plugin/` | fastlane actions `vitrine_import`, `vitrine_validate`, `vitrine_export` | `vitrine` on PATH | repo `vitrinestudio-app/fastlane-plugin-vitrine` (folder content = repo root; owner decision 2026-11-19) → gem `fastlane-plugin-vitrine` on RubyGems |
| `skill/` | agent skill (open Agent Skills format) for making store screenshots of the user's own app over MCP; vendor-neutral | skill folder `skills/vitrine-store-screenshots/` copied into an agent's skills directory or uploaded as zip | own repo `vitrinestudio-app/vitrine-agent-skill` (install: `npx skills add vitrinestudio-app/vitrine-agent-skill`) |

Every folder is self-contained (no references to files outside it), so its
content can be copied to the root of its own repository. Licence: **MIT**
for all of them (`LICENSE` in each folder; ADR 0022 amendment) – they only
run the `vitrine` program or talk to its MCP server, which stays GPL-3.0-or-later.

For other CI systems (Jenkins, Buildkite, CircleCI, Azure Pipelines, …) the
generic integration is the container image or the release archives; the
website page `docs/automation/other-systems/` has snippets.

The CI integrations only call the `vitrine` CLI; the CLI enablers they rely on live
in this repository: `validate --format github|gitlab|junit [-o FILE]`,
`export --format json`, exit codes 0/1/2, store credentials from
`VITRINE_ASC_*` / `VITRINE_PLAY_*`, `VITRINE_APP_DIR`.

The skill (`skill/`) is not CI glue: it instructs an AI assistant that is
connected to Vitrine Studio's MCP server (`vitrine mcp`). Its README covers
install and requirements. Internal template-authoring notes that used to be
part of it: `docs/research/template-authoring-notes.md` (not shipped).
