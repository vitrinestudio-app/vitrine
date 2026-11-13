---
title: GitLab CI
linkTitle: GitLab CI
description: "Show screenshot findings as Code Quality and test reports in merge requests, keep the images as artifacts and upload them from a manual job."
weight: 20
icon: git
---

The container image has everything a GitLab job needs, so there is no
install step: `vitrine`, `git`, `jq` and a POSIX `sh`. How to install the CLI
on other runners and what it offers CI in general: [Automation](../../automation/).

## Checking and exporting in merge requests

```yaml
vitrine-validate:
  image: ghcr.io/vitrinestudio-app/vitrine-cli:0.1.0
  script:
    - vitrine --project store-screens validate --format gitlab -o gl-code-quality-report.json
  artifacts:
    when: always
    reports:
      codequality: gl-code-quality-report.json

vitrine-export:
  image: ghcr.io/vitrinestudio-app/vitrine-cli:0.1.0
  script:
    - vitrine --project store-screens export --out vitrine-export --no-gallery
  artifacts:
    paths: [vitrine-export/]
```

- Code Quality findings show in the merge request. With `-o` the report goes
  to the file and the readable report to the job log.
- For the pipeline's *Tests* tab, use `--format junit -o vitrine-junit.xml`
  with `reports: junit: vitrine-junit.xml`.
- Without the image (e.g. on a macOS runner), install the archive in
  `before_script` – see [Installing the CLI in CI](../../automation/#installing-the-cli-in-ci).

## The planned CI/CD components

{{< callout status >}}
The components are **drafts** and not published yet. Until then, use the
jobs above.
{{< /callout >}}

The CI/CD Catalog components in the gitlab.com project
[`vitrinestudio/vitrine-gitlab-ci`](https://gitlab.com/vitrinestudio/vitrine-gitlab-ci)
will wrap these two jobs (`validate` with Code Quality + JUnit, `export`) and
run the same image:

```yaml
include:
  - component: $CI_SERVER_FQDN/vitrinestudio/vitrine-gitlab-ci/validate@1
    inputs:
      project: store-screens
```

## Uploading from a manual job

Store the keys as masked CI/CD variables, ideally limited to protected
branches or tags (all variables: [Uploading from CI](../../automation/#uploading-from-ci)).
A *File* variable named `VITRINE_ASC_KEY_PATH` holding the `.p8` text works
directly: GitLab puts the text in a file and the variable holds its path. A
manual job means a person starts every upload:

```yaml
vitrine-upload:
  image: ghcr.io/vitrinestudio-app/vitrine-cli:0.1.0
  rules:
    - if: $CI_COMMIT_TAG
      when: manual
  script:
    - vitrine --project store-screens upload plan --store apple --remote --format json > plan.json
    - vitrine --project store-screens upload execute --confirm "$(jq -r .confirm_token plan.json)" --yes
  artifacts:
    when: always
    paths: [plan.json]
```

The plan step exits with 1 when the plan is blocked, so nothing is uploaded.
For Google Play, set `VITRINE_PLAY_SERVICE_ACCOUNT` (or a *File* variable
`VITRINE_PLAY_SERVICE_ACCOUNT_PATH`) and use `--store google`.

## Device frames

The runner has no Apple bezels of its own. Store the ones your project uses
in the repository (private repositories only), or use the built-in frames –
see [Device frames in CI](../../automation/#device-frames-in-ci).
