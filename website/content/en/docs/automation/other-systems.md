---
title: Other CI systems
linkTitle: Other systems
description: "Jenkins, Xcode Cloud, Bitrise, Codemagic, CircleCI, Azure Pipelines and Buildkite: where the vitrine CLI goes and where its reports show up."
weight: 40
icon: terminal
---

Every CI system can run the `vitrine` CLI: either in the container image or
from the archive for your runner (both on [Automation](../../automation/#installing-the-cli-in-ci)).
The steps are the same everywhere – `vitrine validate`, `vitrine export`,
and for releases `vitrine upload` ([Uploading from CI](../../automation/#uploading-from-ci)).
What differs is where the reports go. If your pipeline runs fastlane, you
can also call Vitrine Studio from the lane: [fastlane](../fastlane/).

## Jenkins

Run the steps in the container image, or install the archive on the agent.
Publish the JUnit report with the `junit` step:

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

## Xcode Cloud

Install the macOS archive in `ci_scripts/ci_post_clone.sh` – without `sudo`,
into a folder of the workspace – then validate or export in
`ci_scripts/ci_post_xcodebuild.sh`.

## Bitrise

Add a *Script* step that installs the archive (macOS or Linux stack), then
call `vitrine`. For the test report, write the JUnit file to
`$BITRISE_TEST_RESULT_DIR/vitrine/` with a `test-info.json` and add
*Deploy to Bitrise.io*.

## Codemagic

Add a `scripts:` entry that installs the archive. For uploads, hand
Codemagic's App Store Connect variables to `vitrine` under our names:

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

## CircleCI, Azure Pipelines, Buildkite

Use the container image or the archive. Publish the JUnit report with
`store_test_results` (CircleCI), `PublishTestResults@2` with the format JUnit
(Azure) or as a Buildkite annotation.
