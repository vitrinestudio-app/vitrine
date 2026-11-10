//! Frame package commands with a synthetic bezel package.

use std::sync::Arc;

use arrangia_core::frame_import::synthetic;
use serde_json::{Value, json};

use super::*;
use crate::dialogs::NoDialogs;
use crate::events::{PROJECT_CHANGED, RecordingSink};
use crate::testutil::example_copy;

fn call(app: &AppState, name: &str, args: Value) -> Value {
    match dispatch(app, name, args) {
        Ok(Reply::Json(v)) => v,
        Ok(Reply::Image(_)) => panic!("{name} returned an image"),
        Err(e) => panic!("{name} failed: {e:?}"),
    }
}

fn missing_frames(app: &AppState) -> usize {
    let v = call(
        app,
        "validate",
        json!({"targets": ["ios-iphone-69"], "langs": ["en"]}),
    );
    v.to_string().matches("imported_frame_missing").count()
}

#[test]
fn import_list_render_and_remove() {
    let (tmp, dir) = example_copy();
    // The copy uses imported Apple frames project-wide.
    let yaml = std::fs::read_to_string(dir.join("project.yaml")).unwrap();
    std::fs::write(
        dir.join("project.yaml"),
        yaml.replacen("styles:\n", "styles:\n  frame: imported:apple\n", 1),
    )
    .unwrap();
    let pkg = tmp.path().join("Bezels");
    synthetic::write_folder(&pkg, &synthetic::default_specs()).unwrap();

    let sink = Arc::new(RecordingSink::default());
    let app_dir = tmp.path().join("app");
    let app = AppState::new(
        AppPaths::in_dir(&app_dir),
        sink.clone(),
        Arc::new(NoDialogs),
    )
    .without_watcher();
    call(&app, "project_open", json!({"dir": dir}));
    let before = missing_frames(&app);
    assert!(before > 0);

    // Plan: nothing written yet.
    let plan = call(&app, "frames_import_plan", json!({"path": pkg}));
    assert_eq!(plan["applied"], false);
    assert_eq!(plan["psd"], 2);
    assert_eq!(plan["licenses"].as_array().unwrap().len(), 1);
    assert_eq!(plan["unmatched"].as_array().unwrap().len(), 1);
    let name = plan["unmatched"][0]["name"].as_str().unwrap().to_owned();
    assert!(!app_dir.join("data/frames").exists());

    // Apply, assigning the unknown file manually.
    let plan = call(
        &app,
        "frames_import_apply",
        json!({"path": pkg, "assignments": [
            {"name": name, "model": "iphone-notch", "color": "Space Gray", "orientation": "portrait"}
        ]}),
    );
    assert_eq!(plan["applied"], true);
    assert_eq!(plan["unmatched"], json!([]));
    let frames_dir = app_dir.join("data").join("frames").join("apple");
    assert!(frames_dir.join("index.yaml").is_file());
    assert!(frames_dir.join("user-mapping.yaml").is_file());
    // Never inside the project.
    assert!(!dir.join("frames").exists());
    assert!(
        sink.named(PROJECT_CHANGED)
            .iter()
            .any(|e| e["reason"] == "frames")
    );

    let list = call(&app, "frames_list", Value::Null);
    let apple = &list["sources"][0];
    assert_eq!(apple["source"], "apple");
    assert_eq!(apple["verified"], false);
    assert_eq!(apple["frames"].as_array().unwrap().len(), 8);
    assert!(
        apple["models"]
            .as_array()
            .unwrap()
            .contains(&json!("iphone-island"))
    );
    assert!(missing_frames(&app) < before);

    // The resolved device reports the chosen bezel: the example's iPhone
    // screenshots (1206×2622) are not the 1/4-size synthetic screens, so
    // the 1320×2868 target picks by aspect (Pro Max, the higher priority)
    // and lists both models for the inspector.
    let info = call(
        &app,
        "element_info",
        json!({"target": "ios-iphone-69", "lang": "en", "id": "phone-plan"}),
    );
    let imported = &info["element"]["props"]["imported"];
    assert_eq!(imported["bezel"], "iPhone 16 Pro Max", "{info}");
    assert_eq!(
        imported["bezels"],
        json!(["iPhone 16 Pro", "iPhone 16 Pro Max"])
    );

    // Rendering uses the frames (no asset issues).
    let file = apple["frames"][0]["file"].as_str().unwrap();
    match dispatch(
        &app,
        "frames_thumbnail",
        json!({"file": file, "max_edge": 64}),
    )
    .unwrap()
    {
        Reply::Image(i) => assert!(i.width <= 64 && i.height <= 64),
        Reply::Json(v) => panic!("{v}"),
    }
    assert!(dispatch(&app, "frames_thumbnail", json!({"file": "../x.png"})).is_err());

    let n = call(&app, "frames_remove", json!({"model": "iphone-notch"}));
    assert_eq!(n, 1);
    let n = call(&app, "frames_remove", json!({"bezel": "iPhone 16 Pro"}));
    assert_eq!(n, 2);
    let n = call(&app, "frames_remove", json!({}));
    assert_eq!(n, 5);
    assert_eq!(missing_frames(&app), before);
}

#[test]
fn dmg_licence_error_has_details() {
    let e = package_error(arrangia_core::frame_import::attach_error(
        std::path::Path::new("/tmp/Bezel-iPhone-18.dmg"),
        "hdiutil: attach canceled",
    ));
    let v = serde_json::to_value(&e).unwrap();
    assert_eq!(v["details"]["kind"], "dmg_license");
    assert!(
        v["details"]["volume"]
            .as_str()
            .unwrap()
            .ends_with("Bezel-iPhone-18")
    );
    assert!(v["message"].as_str().unwrap().contains("licence agreement"));
}
