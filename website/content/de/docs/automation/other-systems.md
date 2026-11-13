---
title: Andere CI-Systeme
linkTitle: Andere Systeme
description: "Jenkins, Xcode Cloud, Bitrise, Codemagic, CircleCI, Azure Pipelines und Buildkite: wo das vitrine-Werkzeug hingehört und wo seine Berichte erscheinen."
weight: 40
icon: terminal
---

Jedes CI-System kann das Kommandozeilenwerkzeug `vitrine` ausführen:
entweder im Container-Image oder aus dem Archiv für deinen Runner (beides
unter [Automatisierung](../../automation/#installing-the-cli-in-ci)). Die Schritte sind
überall gleich – `vitrine validate`, `vitrine export` und für Releases
`vitrine upload` ([Hochladen aus der CI](../../automation/#uploading-from-ci)). Unterschiede
gibt es nur dabei, wo die Berichte landen. Läuft in deiner Pipeline fastlane,
kannst du Vitrine Studio auch aus der Lane aufrufen: [fastlane](../fastlane/).

## Jenkins {#jenkins}

Führe die Schritte im Container-Image aus oder installiere das Archiv auf dem
Agent. Den JUnit-Bericht veröffentlichst du mit dem `junit`-Schritt:

```groovy
pipeline {
  agent { docker { image 'ghcr.io/vitrinestudio-app/vitrine-cli:1.0.0' } }
  stages {
    stage('Store screenshots') {
      steps {
        sh 'vitrine --project store-screens validate --format junit -o vitrine-junit.xml'
        sh 'vitrine --project store-screens export --out export --no-gallery'
      }
    }
  }
  post {
    always {
      junit 'vitrine-junit.xml'
      archiveArtifacts artifacts: 'export/**'
    }
  }
}
```

## Xcode Cloud {#xcode-cloud}

Installiere das macOS-Archiv in `ci_scripts/ci_post_clone.sh` – ohne `sudo`,
in einen Ordner im Workspace – und prüfe oder exportiere dann in
`ci_scripts/ci_post_xcodebuild.sh`.

## Bitrise {#bitrise}

Füge einen *Script*-Schritt hinzu, der das Archiv installiert (macOS- oder
Linux-Stack), und rufe danach `vitrine` auf. Für den Testbericht schreibst du
die JUnit-Datei mit einer `test-info.json` nach
`$BITRISE_TEST_RESULT_DIR/vitrine/` und fügst *Deploy to Bitrise.io* hinzu.

## Codemagic {#codemagic}

Füge einen `scripts:`-Eintrag hinzu, der das Archiv installiert. Für Uploads
gibst du die App-Store-Connect-Variablen von Codemagic unter unseren Namen an
`vitrine` weiter:

```yaml
scripts:
  - name: Upload store screenshots
    script: |
      export VITRINE_ASC_KEY="$APP_STORE_CONNECT_PRIVATE_KEY"
      export VITRINE_ASC_KEY_ID="$APP_STORE_CONNECT_KEY_IDENTIFIER"
      export VITRINE_ASC_ISSUER_ID="$APP_STORE_CONNECT_ISSUER_ID"
      vitrine --project store-screens upload plan --store apple --remote --format json > plan.json
      vitrine --project store-screens upload execute --confirm "$(jq -r .confirm_token plan.json)" --yes
```

## CircleCI, Azure Pipelines, Buildkite {#circleci-azure-pipelines-buildkite}

Nimm das Container-Image oder das Archiv. Den JUnit-Bericht veröffentlichst
du mit `store_test_results` (CircleCI), `PublishTestResults@2` im Format
JUnit (Azure) oder als Buildkite-Annotation.
