//! Frame package commands with a synthetic bezel package.

use std::sync::Arc;

use serde_json::{Value, json};
use vitrine_core::frame_import::synthetic;

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
    let app = &app;
    call(app, "project_open", json!({"dir": dir}));
    let before = missing_frames(app);
    assert!(before > 0);

    // Plan: nothing written yet.
    let plan = call(app, "frames_import_plan", json!({"path": pkg}));
    assert_eq!(plan["applied"], false);
    assert_eq!(plan["psd"], 2);
    assert_eq!(plan["licenses"].as_array().unwrap().len(), 1);
    assert_eq!(plan["unmatched"].as_array().unwrap().len(), 1);
    let name = plan["unmatched"][0]["name"].as_str().unwrap().to_owned();
    assert!(!app_dir.join("data/frames").exists());

    // Apply, assigning the unknown file manually.
    let plan = call(
        app,
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

    let list = call(app, "frames_list", Value::Null);
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
    assert!(missing_frames(app) < before);

    // The resolved device reports the chosen bezel: the example's iPhone
    // screenshots (1206×2622) are not the 1/4-size synthetic screens, so
    // the 1320×2868 target picks by aspect (Pro Max, the higher priority)
    // and lists both models for the inspector.
    let info = call(
        app,
        "element_info",
        json!({"target": "ios-iphone-69", "lang": "en", "id": "phone-plan"}),
    );
    let imported = &info["element"]["props"]["imported"];
    assert_eq!(imported["bezel"], "iPhone 16 Pro Max", "{info}");
    assert_eq!(
        imported["bezels"],
        json!(["iPhone 16 Pro", "iPhone 16 Pro Max"])
    );

    colour_swatches_and_remapping(app, apple, imported);

    // Rendering uses the frames (no asset issues).
    let file = apple["frames"][0]["file"].as_str().unwrap();
    match dispatch(
        app,
        "frames_thumbnail",
        json!({"file": file, "max_edge": 64}),
    )
    .unwrap()
    {
        Reply::Image(i) => assert!(i.width <= 64 && i.height <= 64),
        Reply::Json(v) => panic!("{v}"),
    }
    assert!(dispatch(app, "frames_thumbnail", json!({"file": "../x.png"})).is_err());

    let n = call(app, "frames_remove", json!({"model": "iphone-notch"}));
    assert_eq!(n, 1);
    let n = call(app, "frames_remove", json!({"bezel": "iPhone 16 Pro"}));
    assert_eq!(n, 2);
    let n = call(app, "frames_remove", json!({}));
    assert_eq!(n, 5);
    assert_eq!(missing_frames(app), before);
}

/// Swatches of installed colours, and colours following frame switches
/// (project default and per device), each in one undo step.
fn colour_swatches_and_remapping(app: &AppState, apple: &Value, imported: &Value) {
    // Measured swatches per installed colour, also on the resolved device.
    let colors = &apple["colors"]["iphone-island"];
    assert_eq!(colors[0]["id"], "black-titanium");
    assert_eq!(colors[0]["swatch"], "#2C2C30");
    assert_eq!(colors[1]["swatch"], "#BFA48F");
    assert_eq!(imported["colors"], *colors);

    // Switching the project default remaps inheriting device colours in
    // one undo step: to parametric keeps palette colours …
    let beds = |app: &AppState| {
        call(
            app,
            "element_get",
            json!({"target": "ios-iphone-69", "id": "phone-beds"}),
        )["base"]["color"]
            .clone()
    };
    call(app, "styles_patch", json!({"frame": "detailed"}));
    assert_eq!(beds(app), "natural-titanium");
    // … to imported picks the closest installed colour.
    let r = call(app, "styles_patch", json!({"frame": "imported:apple"}));
    assert!(r["applied"].as_bool().unwrap());
    assert_eq!(beds(app), "desert-titanium");
    call(app, "undo", Value::Null);
    assert_eq!(beds(app), "natural-titanium");
    call(app, "redo", Value::Null);
    // An imported-only colour on a device, then the device goes parametric:
    // the colour follows in the same patch.
    let r = call(
        app,
        "element_patch",
        json!({"target": "ios-iphone-69", "id": "phone-beds", "patch": {"frame": "stylized"}}),
    );
    assert!(r["applied"].as_bool().unwrap());
    assert_eq!(beds(app), "natural-titanium");
    call(
        app,
        "element_patch",
        json!({"target": "ios-iphone-69", "id": "phone-beds", "patch": {"frame": null}}),
    );
    assert_eq!(beds(app), "desert-titanium");
    // Back to parametric project-wide: colours first, then the styles.
    call(app, "styles_patch", json!({"frame": "stylized"}));
    assert_eq!(beds(app), "natural-titanium");
    call(app, "styles_patch", json!({"frame": "imported:apple"}));
}

#[test]
fn dmg_licence_error_has_details() {
    let e = package_error(vitrine_core::frame_import::attach_error(
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

/// Own frame packs (ADR 0024): import app-wide and into the project, list,
/// use with `frame: pack:<id>`, thumbnail, remove.
#[test]
fn frame_packs_import_use_and_remove() {
    let (tmp, dir) = example_copy();
    let zip = synthetic::write_pack_zip(&tmp.path().join("clay.zip"), 4).unwrap();
    let app_dir = tmp.path().join("app");
    let app = AppState::new(
        AppPaths::in_dir(&app_dir),
        Arc::new(RecordingSink::default()),
        Arc::new(NoDialogs),
    )
    .without_watcher();
    let app = &app;
    call(app, "project_open", json!({"dir": dir}));
    let pack_missing = |app: &AppState| {
        call(
            app,
            "element_patch",
            json!({"target": "ios-iphone-69", "id": "phone-beds",
                   "patch": {"frame": "pack:clay-phone", "color": "ink"}}),
        );
        let v = call(
            app,
            "validate",
            json!({"targets": ["ios-iphone-69"], "langs": ["en"]}),
        );
        v.to_string().contains("imported_frame_missing")
    };
    // Not installed: parametric fallback with an info.
    assert!(pack_missing(app));

    let dry = call(
        app,
        "frames_pack_import",
        json!({"path": zip, "dry_run": true}),
    );
    assert_eq!(dry["applied"], false);
    assert_eq!(dry["id"], "clay-phone");
    assert_eq!(
        dry["pack"]["frames"].as_array().unwrap().len(),
        10,
        "{dry:#}"
    );
    assert!(!app_dir.join("data").join("frames").join("packs").exists());
    let done = call(app, "frames_pack_import", json!({"path": zip}));
    assert_eq!(done["applied"], true);
    assert!(!pack_missing(app), "installed app-wide");

    let list = call(app, "frames_list", Value::Null);
    let packs = list["packs"].as_array().unwrap();
    assert_eq!(packs.len(), 1);
    assert_eq!(packs[0]["origin"], "app");
    assert_eq!(packs[0]["frame"], "pack:clay-phone");
    let file = packs[0]["frames"][0]["file"].as_str().unwrap().to_owned();
    assert!(matches!(
        dispatch(
            app,
            "frames_thumbnail",
            json!({"source": "pack:clay-phone", "file": file})
        ),
        Ok(Reply::Image(_))
    ));
    let bad_source = format!("pack:{}", "..");
    assert!(
        dispatch(
            app,
            "frames_thumbnail",
            json!({"source": bad_source, "file": file})
        )
        .is_err()
    );

    // Into the project too (travels with it); it hides the app pack.
    call(
        app,
        "frames_pack_import",
        json!({"path": zip, "target": "project"}),
    );
    assert!(
        dir.join("frames")
            .join("clay-phone")
            .join("frame.yaml")
            .is_file()
    );
    let list = call(app, "frames_list", Value::Null);
    assert_eq!(list["packs"][0]["origin"], "project");
    assert_eq!(list["packs"][0]["shadows_app_pack"], true);

    // Remove both: the fallback is back.
    assert_eq!(
        call(
            app,
            "frames_pack_remove",
            json!({"id": "clay-phone", "target": "project"})
        ),
        true
    );
    assert_eq!(
        call(app, "frames_pack_remove", json!({"id": "clay-phone"})),
        true
    );
    assert!(pack_missing(app));
    assert_eq!(
        call(app, "frames_pack_remove", json!({"id": "clay-phone"})),
        false
    );
}
