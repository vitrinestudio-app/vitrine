# Vitrine Studio – GitLab CI/CD components

GitLab CI/CD components for [Vitrine Studio](https://vitrinestudio.app)
projects (App Store / Play Store screenshots).

> **Status: draft.** Home: the gitlab.com project
> [`vitrinestudio/vitrine-gitlab-ci`](https://gitlab.com/vitrinestudio/vitrine-gitlab-ci),
> marked as a CI/CD Catalog project and released with a semver tag. The
> source is maintained in
> [`integrations/gitlab/`](https://github.com/vitrinestudio-app/vitrine/tree/main/integrations/gitlab)
> of the main repository and copied there.

## What this project contains

Only two component files, nothing else:

| file | component | job | what it does |
|---|---|---|---|
| `templates/validate.yml` | `validate` | `vitrine-validate` | `vitrine validate` → Code Quality report (merge request widget) + JUnit report (pipeline *Tests* tab); errors fail the job |
| `templates/export.yml` | `export` | `vitrine-export` | `vitrine export` → artifact *Store screenshots* (also `layout: fastlane`) |

Both jobs run the container image
[`ghcr.io/vitrinestudio-app/vitrine-cli`](https://github.com/vitrinestudio-app/vitrine/pkgs/container/vitrine-cli)
(Alpine, static `vitrine` binary, amd64 + arm64, built by the Vitrine Studio
release workflow). CI/CD components are a GitLab feature; on every other CI
system (Jenkins, Buildkite, CircleCI, Azure Pipelines, …) use that image or
the release archives directly – see
[CI/CD](https://vitrinestudio.app/docs/ci/).

## Use

```yaml
include:
  - component: $CI_SERVER_FQDN/vitrinestudio/vitrine-gitlab-ci/validate@1
    inputs:
      project: store-screens
      strict: true
  - component: $CI_SERVER_FQDN/vitrinestudio/vitrine-gitlab-ci/export@1
    inputs:
      project: store-screens
      layout: fastlane
      out: fastlane
```

Inputs of both: `stage`, `image` (default
`ghcr.io/vitrinestudio-app/vitrine-cli:<the matching release>`; pin your own
tag here), `project`, `job_name`; `validate` also `strict`, `preview`;
`export` also `layout`, `out`.

Without the catalog (self-managed GitLab without access to gitlab.com), copy
the templates into your repository and include them as local components or
use the job directly:

```yaml
vitrine-validate:
  image: ghcr.io/vitrinestudio-app/vitrine-cli:0.1.0
  script:
    - vitrine --project store-screens validate --format gitlab -o gl-code-quality-report.json
  artifacts:
    when: always
    reports:
      codequality: gl-code-quality-report.json
```

## Uploading from GitLab

Store the keys as masked CI/CD variables (file variables work too:
`VITRINE_ASC_KEY_PATH: $ASC_KEY_FILE`), and keep the job manual or on a
protected branch. An upload is always plan → `execute --confirm <token> --yes`:

```yaml
upload-screenshots:
  image: ghcr.io/vitrinestudio-app/vitrine-cli:0.1.0
  rules:
    - if: $CI_COMMIT_BRANCH == $CI_DEFAULT_BRANCH
      when: manual
  variables:
    VITRINE_ASC_KEY_ID: $ASC_KEY_ID
    VITRINE_ASC_ISSUER_ID: $ASC_ISSUER_ID
    VITRINE_ASC_KEY_PATH: $ASC_KEY_FILE      # a "File" variable
  script:
    - cd store-screens
    - vitrine upload plan --remote --format json > plan.json
    - vitrine upload execute --confirm "$(jq -r .confirm_token plan.json)" --yes
```

## Notes

- The image's shell is POSIX `sh` (Alpine; no `bash`), with `git`, `jq` and
  CA certificates.
- Code Quality shows in the merge request widget on every tier; inline diff
  annotations need GitLab Ultimate. JUnit reports work on every tier.
- The image runs as a non-root user (uid 10001). GitLab's Docker executor
  checks out with a permissive umask by default, so the job can write the
  reports and exports; with a restrictive runner set `image: docker: user:`.
- Findings point at the `project.yaml` line of the element or target,
  relative to the repository root (the job's working directory).
- Release of this project: a semver tag (`1.0.0`) plus a `release` job
  publishes the components to the CI/CD Catalog; keep the default `image`
  tag in step with the Vitrine Studio release it was tested with.

## Licence

MIT (see `LICENSE`). The components only run the `vitrine` program from the
container image; Vitrine Studio itself is licensed under the GNU GPL v3 or
later, and the image carries its licence files under
`/usr/share/doc/vitrine/licenses/`.
