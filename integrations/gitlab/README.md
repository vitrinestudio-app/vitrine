# Vitrine Studio – GitLab CI/CD components (DRAFT)

> **Draft, not published.** Needs the CLI container image
> (`docker/cli/Dockerfile`) published to a registry first – proposed
> `ghcr.io/vitrinestudio-app/vitrine-cli:<version>` – and, for the GitLab
> CI/CD Catalog, its own project on gitlab.com (proposed
> `gitlab.com/vitrinestudio/vitrine-ci`) marked as a catalog project and
> released with a semver tag (owner decisions). The templates follow the
> component layout (`templates/<name>.yml`, `spec: inputs`).

Two components:

| component | job | what it does |
|---|---|---|
| `validate` | `vitrine-validate` | `vitrine validate` → Code Quality report (merge request widget) + JUnit report (pipeline *Tests* tab); errors fail the job |
| `export` | `vitrine-export` | `vitrine export` → artifact *Store screenshots* (also `layout: fastlane`) |

## Use (after publication)

```yaml
include:
  - component: $CI_SERVER_FQDN/vitrinestudio/vitrine-ci/validate@1
    inputs:
      project: store-screens
      strict: true
  - component: $CI_SERVER_FQDN/vitrinestudio/vitrine-ci/export@1
    inputs:
      project: store-screens
      layout: fastlane
      out: fastlane
```

Without the catalog (self-managed GitLab, or before publication), copy the
templates into your repository and include them as local components or use
the job directly:

```yaml
vitrine-validate:
  image: ghcr.io/vitrinestudio-app/vitrine-cli:0.2.0
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
protected branch:

```yaml
upload-screenshots:
  image: ghcr.io/vitrinestudio-app/vitrine-cli:0.2.0
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

- Code Quality shows in the merge request widget on every tier; inline diff
  annotations need GitLab Ultimate. JUnit reports work on every tier.
- The image runs as a non-root user (uid 10001). GitLab's Docker executor
  checks out with a permissive umask by default, so the job can write the
  reports and exports; with a restrictive runner set `image: docker: user:`.
- Findings point at the `project.yaml` line of the element or target,
  relative to the repository root (the job's working directory).
