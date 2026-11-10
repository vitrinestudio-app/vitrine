//! `arrangia upload plan|execute|log` without network: the local plan, the
//! token checks and the log. Uploads themselves are tested end to end in
//! `crates/app/tests/upload.rs` (wiremock); the CLI process cannot see a
//! key imported elsewhere (`ARRANGIA_KEYRING=mock` is per process).

use std::path::Path;
use std::process::{Command, Output, Stdio};

use serde_json::Value;

const PROJECT: &str = r##"format: arrangia/1
revision: 0
name: "CLI upload"
languages:
  default: en
  available: [en, de]
strips:
- id: main
  page: {aspect: 1320:2868, count: 2, ids: [one, two]}
  backgrounds:
  - "#EEF1F6"
  elements:
  - id: t1
    kind: text
    page: 0
    text: one.title
    box: {top: 160, left: 80, right: 80}
  - id: t2
    kind: text
    page: 1
    text: two.title
    box: {top: 160, left: 80, right: 80}
targets:
- id: play
  strip: main
  size: [1080, 2160]
  gallery: play-phone
  store: {google: {image_type: phoneScreenshots}}
store:
  google: {account: google-play, package_name: com.example.fernwood}
"##;

const SETTINGS: &str = "store_accounts:
- id: google-play
  kind: google
  label: Play
  google: {client_email: up@demo.iam.gserviceaccount.com}
";

fn run(dir: &Path, app: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_arrangia"))
        .arg("--project")
        .arg(dir)
        .arg("upload")
        .arg("--app-dir")
        .arg(app)
        .args(args)
        .env("ARRANGIA_KEYRING", "mock")
        .env_remove("ARRANGIA_APP_DIR")
        .stdin(Stdio::null())
        .output()
        .unwrap()
}

fn text(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}

#[test]
fn plan_execute_checks_and_log() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("project");
    let app = tmp.path().join("app");
    std::fs::create_dir_all(dir.join("i18n")).unwrap();
    std::fs::create_dir_all(app.join("config")).unwrap();
    std::fs::write(dir.join("project.yaml"), PROJECT).unwrap();
    std::fs::write(dir.join("i18n/en.yaml"), "one.title: One\ntwo.title: Two\n").unwrap();
    std::fs::write(
        dir.join("i18n/de.yaml"),
        "one.title: Eins\ntwo.title: Zwei\n",
    )
    .unwrap();
    std::fs::write(app.join("config/settings.yaml"), SETTINGS).unwrap();

    // Text plan.
    let out = run(&dir, &app, &["plan"]);
    let stdout = text(&out.stdout);
    assert_eq!(
        out.status.code(),
        Some(0),
        "{stdout}\n{}",
        text(&out.stderr)
    );
    assert!(stdout.contains("Google Play"), "{stdout}");
    assert!(stdout.contains("de-DE"), "{stdout}");
    assert!(
        stdout.contains("arrangia upload execute --confirm upl_"),
        "{stdout}"
    );

    // JSON plan.
    let out = run(&dir, &app, &["plan", "--lang", "en", "--format", "json"]);
    assert_eq!(out.status.code(), Some(0));
    let plan: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(plan["totals"]["upload_sets"], 1);
    assert_eq!(plan["stores"][0]["sets"][0]["locale"], "en-US");
    let token = plan["confirm_token"].as_str().unwrap().to_owned();

    // Not a terminal and no --yes: refused before anything happens.
    let out = run(&dir, &app, &["execute", "--confirm", &token]);
    assert_eq!(out.status.code(), Some(2));
    assert!(text(&out.stderr).contains("--yes"), "{}", text(&out.stderr));
    // Unknown token.
    let out = run(&dir, &app, &["execute", "--confirm", "upl_nope", "--yes"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(text(&out.stderr).contains("unknown confirm token"));
    // The key is not in this process' (mock) keychain: refused, no upload.
    let out = run(&dir, &app, &["execute", "--confirm", &token, "--yes"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(
        text(&out.stderr).contains("keychain") || text(&out.stderr).contains("no "),
        "{}",
        text(&out.stderr)
    );

    // A blocked plan (unknown target filter → error; store without slot → blocked).
    let out = run(&dir, &app, &["plan", "--store", "apple"]);
    assert_eq!(out.status.code(), Some(1), "{}", text(&out.stdout));
    assert!(text(&out.stdout).contains("not mapped to apple"));

    let out = run(&dir, &app, &["log"]);
    assert_eq!(out.status.code(), Some(0));
    assert!(text(&out.stdout).contains("No uploads yet"));
}

#[test]
fn refdata_needs_an_apple_account_and_its_key() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("empty");
    let app = tmp.path().join("app");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::create_dir_all(app.join("config")).unwrap();
    std::fs::write(app.join("config/settings.yaml"), SETTINGS).unwrap();
    // No project, only a Google account: nothing to read with.
    let out = run(&dir, &app, &["refdata"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(
        text(&out.stderr).contains("no App Store Connect account"),
        "{}",
        text(&out.stderr)
    );
    // An Apple account whose key is not in this process' keychain: refused
    // before any network call.
    std::fs::write(
        app.join("config/settings.yaml"),
        format!(
            "{SETTINGS}- id: apple-team\n  kind: apple\n  label: Team\n  apple: {{issuer_id: 57246542-96fe-1a63-e053-0824d011072a, key_id: ABCDE12345}}\n"
        ),
    )
    .unwrap();
    let out = run(&dir, &app, &["refdata", "--format", "json"]);
    assert_eq!(out.status.code(), Some(2), "{}", text(&out.stdout));
    assert!(
        text(&out.stderr).contains("keychain") || text(&out.stderr).contains("key"),
        "{}",
        text(&out.stderr)
    );
    let out = Command::new(env!("CARGO_BIN_EXE_arrangia"))
        .args(["upload", "refdata", "--help"])
        .output()
        .unwrap();
    assert!(
        text(&out.stdout).contains("asset types"),
        "{}",
        text(&out.stdout)
    );
}
