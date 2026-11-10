//! Tests of `import_plan` / `import_apply` (task 3.3) on a fastlane-style
//! folder made from the example project's screenshots.

use std::path::Path;
use std::sync::Arc;

use serde_json::{Value, json};

use super::*;
use crate::dialogs::NoDialogs;
use crate::events::RecordingSink;
use crate::testutil::example_copy;

fn app() -> (tempfile::TempDir, PathBuf, AppState) {
    let (tmp, dir) = example_copy();
    let app = AppState::new(
        AppPaths::in_dir(&tmp.path().join("app")),
        Arc::new(RecordingSink::default()),
        Arc::new(NoDialogs),
    )
    .without_watcher();
    app.open_project(&dir).unwrap();
    (tmp, dir, app)
}

fn call(app: &AppState, name: &str, args: Value) -> Value {
    match dispatch(app, name, args) {
        Ok(Reply::Json(v)) => v,
        Ok(Reply::Image(_)) => panic!("{name} returned an image"),
        Err(e) => panic!("{name} failed: {e:?}"),
    }
}

/// `<tmp>/fastlane/{en-US,de-DE}/iPhone 17 Pro-0N_<key>.png` from the
/// project's ios-phone screenshots (de gets the en calendar, a new file)
/// plus a 16:10 file of an unknown device.
fn fastlane(tmp: &Path, project: &Path) -> PathBuf {
    let src = tmp.join("fastlane");
    let shots = project.join("screenshots");
    for (lang, locale) in [("en", "en-US"), ("de", "de-DE")] {
        let dir = src.join(locale);
        std::fs::create_dir_all(&dir).unwrap();
        for (i, key) in ["home", "planner", "calendar"].iter().enumerate() {
            let mut from = shots
                .join(lang)
                .join("ios-phone")
                .join(format!("{key}.png"));
            if !from.is_file() {
                from = shots.join("en/ios-phone").join(format!("{key}.png"));
            }
            std::fs::copy(
                &from,
                dir.join(format!("iPhone 17 Pro-{:02}_{key}.png", i + 1)),
            )
            .unwrap();
        }
    }
    std::fs::copy(
        shots.join("en/web/plots.png"),
        src.join("en-US/Mystery Gadget-09_gadget_shot.png"),
    )
    .unwrap();
    std::fs::write(src.join("screenshots.html"), "<html/>").unwrap();
    src
}

fn item<'a>(plan: &'a Value, source: &str) -> &'a Value {
    plan["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["source"] == source)
        .unwrap_or_else(|| panic!("no {source} in {plan:#}"))
}

#[test]
fn plan_detects_fastlane_and_applies_with_mapping() {
    let (tmp, dir, app) = app();
    let src = fastlane(tmp.path(), &dir);

    let plan = call(&app, "import_plan", json!({"dir": src}));
    assert_eq!(plan["format"], "fastlane");
    let de_cal = item(&plan, "de-DE/iPhone 17 Pro-03_calendar.png");
    assert_eq!(de_cal["dest"], "screenshots/de/ios-phone/calendar.png");
    assert_eq!(de_cal["action"], "new");
    assert_eq!(
        item(&plan, "en-US/iPhone 17 Pro-01_home.png")["action"],
        "unchanged"
    );
    assert_eq!(plan["skipped"][0]["source"], "screenshots.html");
    assert_eq!(plan["mapping"]["locales"]["de-DE"], "de");

    // Row corrections and an extra device mapping.
    let args = json!({
        "dir": src,
        "format": "fastlane",
        "mapping": {"devices": {"Mystery Gadget": "web"}},
        "overrides": {
            "en-US/Mystery Gadget-09_gadget_shot.png": {"key": "gadget"},
            "en-US/iPhone 17 Pro-02_planner.png": {"skip": true},
        },
    });
    let plan = call(&app, "import_plan", args.clone());
    let gadget = item(&plan, "en-US/Mystery Gadget-09_gadget_shot.png");
    assert_eq!(gadget["dest"], "screenshots/en/web/gadget.png");
    assert_eq!(gadget["overridden"], true);
    assert!(
        plan["skipped"]
            .as_array()
            .unwrap()
            .iter()
            .any(|s| s["reason"] == "skipped by you")
    );
    assert!(!dir.join("screenshots/en/web/gadget.png").exists());

    let mut apply_args = args;
    apply_args["save_mapping"] = json!(true);
    let out = call(&app, "import_apply", apply_args);
    let copied: Vec<&str> = out["report"]["copied"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(Value::as_str)
        .collect();
    assert!(copied.contains(&"screenshots/en/web/gadget.png"), "{out:#}");
    assert!(copied.contains(&"screenshots/de/ios-phone/calendar.png"));
    assert!(dir.join("screenshots/en/web/gadget.png").is_file());
    assert_eq!(out["mapping"]["applied"], true);

    // The mapping is in project.yaml and undoable in one step.
    let yaml = std::fs::read_to_string(dir.join("project.yaml")).unwrap();
    assert!(yaml.contains("Mystery Gadget: web"), "{yaml}");
    assert!(yaml.contains("09_gadget_shot: gadget"), "{yaml}");
    let session = app.session().unwrap();
    assert_eq!(
        session.with_project(|p| p.import.fastlane.devices["iPhone 17 Pro"].clone()),
        "ios-phone"
    );
    call(&app, "undo", Value::Null);
    assert!(session.with_project(|p| p.import.is_empty()));
}

#[test]
fn plan_files_and_errors() {
    let (tmp, dir, app) = app();
    let file = tmp.path().join("home_de.png");
    std::fs::copy(dir.join("screenshots/de/ios-phone/home.png"), &file).unwrap();
    let plan = call(&app, "import_plan", json!({"files": [file]}));
    assert_eq!(plan["format"], "flat");
    assert_eq!(
        plan["items"][0]["dest"],
        "screenshots/de/ios-phone/home.png"
    );
    assert_eq!(plan["items"][0]["action"], "unchanged");

    let e = dispatch(&app, "import_plan", json!({"dir": tmp.path().join("nope")})).unwrap_err();
    assert_eq!(e.code, ErrorCode::NotFound);
    let e = dispatch(&app, "import_plan", json!({})).unwrap_err();
    assert_eq!(e.code, ErrorCode::InvalidArgs);
    let e = dispatch(
        &app,
        "import_plan",
        json!({"dir": tmp.path(), "mapping": {"devices": {"x": "not valid"}}}),
    )
    .unwrap_err();
    assert_eq!(e.code, ErrorCode::InvalidArgs);
}
