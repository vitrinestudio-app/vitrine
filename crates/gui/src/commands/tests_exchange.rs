//! Tests of the translation exchange commands (task 3.4): `strings_export`,
//! `strings_import_plan`, `strings_import_apply`, `dialog_save_file`.

use std::sync::Arc;

use serde_json::{Value, json};

use super::*;
use crate::dialogs::FixedDialogs;
use crate::events::RecordingSink;
use crate::testutil::example_copy;

fn app() -> (tempfile::TempDir, PathBuf, AppState) {
    let (tmp, dir) = example_copy();
    let save = tmp.path().join("out/strings.xlf");
    let app = AppState::new(
        AppPaths::in_dir(&tmp.path().join("app")),
        Arc::new(RecordingSink::default()),
        Arc::new(FixedDialogs {
            save: Some(save),
            ..FixedDialogs::default()
        }),
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

#[test]
fn export_writes_files_and_round_trips() {
    let (tmp, _dir, app) = app();
    let path = call(
        &app,
        "dialog_save_file",
        json!({"title": "Export", "default_name": "Fernwood.xlf", "filters": [{"name": "XLIFF", "extensions": ["xlf"]}]}),
    );
    assert_eq!(path, json!(tmp.path().join("out/strings.xlf")));
    let r = call(
        &app,
        "strings_export",
        json!({"format": "xliff20", "path": path, "options": {"context": true}}),
    );
    let files = r["files"].as_array().unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0]["lang"], "de");
    assert_eq!(files[0]["keys"], 26);
    let written = PathBuf::from(files[0]["path"].as_str().unwrap());
    assert!(
        std::fs::read_to_string(&written)
            .unwrap()
            .contains("trgLang=\"de\"")
    );

    let csv = tmp.path().join("out/t.csv");
    call(
        &app,
        "strings_export",
        json!({"format": "csv", "path": csv, "options": {"delimiter": "semicolon", "bom": true}}),
    );
    let plan = call(
        &app,
        "strings_import_plan",
        json!({"paths": [written, csv]}),
    );
    assert_eq!(plan["changes"], json!([]));
    assert_eq!(plan["files"][0]["format"], "xliff20");
    assert_eq!(plan["files"][1]["format"], "csv");
    let e = dispatch(&app, "strings_import_apply", json!({"plan": plan})).unwrap_err();
    assert_eq!(e.code, ErrorCode::InvalidArgs);

    let e = dispatch(
        &app,
        "strings_export",
        json!({"format": "csv", "langs": ["xx"], "path": csv}),
    )
    .unwrap_err();
    assert_eq!(e.code, ErrorCode::NotFound);
}

#[test]
fn import_plan_apply_is_one_undo_step() {
    let (tmp, dir, app) = app();
    let csv = tmp.path().join("de.csv");
    std::fs::write(
        &csv,
        "key,de,en\nplan.sub,Beete und Aufgaben,\nbeds.title,\"Plane jedes einzelne Beet ganz bequem **per Drag & Drop** auf deinem Tablet\",\n",
    )
    .unwrap();
    let plan = call(&app, "strings_import_plan", json!({"paths": [csv]}));
    assert_eq!(plan["changes"].as_array().unwrap().len(), 2);
    assert_eq!(plan["langs"][0]["lang"], "en");
    assert_eq!(plan["langs"][0]["skipped"].as_array().unwrap().len(), 2);
    assert_eq!(
        plan["changes"][0]["old"],
        "Jedes Beet **per Drag & Drop** planen"
    );
    let layout = plan["layout"].as_array().unwrap();
    assert!(
        layout
            .iter()
            .any(|f| f["code"] == "text_overflow" && f["key"] == "beds.title" && f["new"] == true),
        "{layout:?}"
    );

    let r = call(&app, "strings_import_apply", json!({"plan": plan}));
    assert_eq!(r["applied"], true);
    assert_eq!(
        r["history"]["undo"].as_array().unwrap().last().unwrap(),
        "Import translations (2 strings)"
    );
    let de = std::fs::read_to_string(dir.join("i18n/de.yaml")).unwrap();
    assert!(de.contains("plan.sub: Beete und Aufgaben"));

    // Applying the same plan again conflicts (old values changed).
    let e = dispatch(&app, "strings_import_apply", json!({"plan": plan})).unwrap_err();
    assert_eq!(e.code, ErrorCode::Conflict);

    call(&app, "undo", Value::Null);
    let t = call(&app, "strings_get", json!({"lang": "de"}));
    assert_eq!(
        t["strings"]["de"]["plan.sub"],
        "Beete, Pflanzen und Aufgaben an einem ruhigen Ort."
    );
    assert_eq!(
        t["strings"]["de"]["beds.title"],
        "Jedes Beet **per Drag & Drop** planen"
    );
}

#[test]
fn import_plan_reports_bad_files() {
    let (tmp, _dir, app) = app();
    let bad = tmp.path().join("bad.xlf");
    std::fs::write(
        &bad,
        "<xliff version=\"2.0\" srcLang=\"en\" trgLang=\"fr\"><file id=\"f\"/></xliff>",
    )
    .unwrap();
    let plan = call(
        &app,
        "strings_import_plan",
        json!({"paths": [bad], "layout": false}),
    );
    assert!(plan["files"][0]["error"].as_str().unwrap().contains("`fr`"));
    let e = dispatch(
        &app,
        "strings_import_plan",
        json!({"paths": [tmp.path().join("nope.csv")]}),
    )
    .unwrap_err();
    assert_eq!(e.code, ErrorCode::Io);
}
