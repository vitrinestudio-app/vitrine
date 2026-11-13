---
title: GitLab CI
linkTitle: GitLab CI
description: "Befunde zu den Screenshots als Code-Quality- und Testberichte im Merge Request zeigen, die Bilder als Artefakte aufbewahren und aus einem manuellen Job hochladen."
weight: 20
icon: git
---

Das Container-Image bringt alles mit, was ein GitLab-Job braucht, ein
Installationsschritt entfällt: `vitrine`, `git`, `jq` und eine POSIX-`sh`.
Wie du das Kommandozeilenwerkzeug auf anderen Runnern installierst und was es
für CI allgemein bietet: [Automatisierung](../../automation/).

## Prüfen und exportieren im Merge Request {#checking-and-exporting-in-merge-requests}

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

- Code-Quality-Befunde erscheinen im Merge Request. Mit `-o` geht der Bericht
  in die Datei und der lesbare Bericht ins Job-Log.
- Für den Reiter *Tests* der Pipeline nimmst du `--format junit -o
  vitrine-junit.xml` mit `reports: junit: vitrine-junit.xml`.
- Ohne das Image (z. B. auf einem macOS-Runner) installierst du das Archiv
  in `before_script` – siehe
  [Installation in der CI](../../automation/#installing-the-cli-in-ci).

## Die geplanten CI/CD-Komponenten {#the-planned-cicd-components}

{{< callout status >}}
Die Komponenten sind **Entwürfe** und noch nicht veröffentlicht. Bis dahin
nimmst du die Jobs oben.
{{< /callout >}}

Die CI/CD-Catalog-Komponenten im gitlab.com-Projekt
[`vitrinestudio/vitrine-gitlab-ci`](https://gitlab.com/vitrinestudio/vitrine-gitlab-ci)
fassen diese beiden Jobs zusammen (`validate` mit Code Quality + JUnit,
`export`) und nutzen dasselbe Image:

```yaml
include:
  - component: $CI_SERVER_FQDN/vitrinestudio/vitrine-gitlab-ci/validate@1
    inputs:
      project: store-screens
```

## Hochladen aus einem manuellen Job {#uploading-from-a-manual-job}

Leg die Schlüssel als maskierte CI/CD-Variablen ab, am besten auf geschützte
Branches oder Tags beschränkt (alle Variablen:
[Hochladen aus der CI](../../automation/#uploading-from-ci)). Eine Variable vom Typ *File*
namens `VITRINE_ASC_KEY_PATH` mit dem Text der `.p8`-Datei funktioniert
direkt: GitLab legt den Text in einer Datei ab, und die Variable enthält
ihren Pfad. Ein manueller Job bedeutet, dass ein Mensch jeden Upload startet:

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

Ist der Plan blockiert, endet der Plan-Schritt mit Exit-Code 1, und es wird
nichts hochgeladen. Für Google Play setzt du `VITRINE_PLAY_SERVICE_ACCOUNT`
(oder eine *File*-Variable `VITRINE_PLAY_SERVICE_ACCOUNT_PATH`) und nimmst
`--store google`.

## Geräterahmen {#device-frames}

Der Runner hat keine eigenen Apple-Bezels. Speichere die, die dein Projekt
verwendet, im Repository (nur private Repositories), oder nimm die
eingebauten Rahmen – siehe [Geräterahmen in der CI](../../automation/#device-frames-in-ci).
