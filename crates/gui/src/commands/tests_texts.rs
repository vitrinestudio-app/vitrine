//! Tests of the text and asset commands of tasks 2.7/2.8: `element_info`,
//! `markup_check`, `render_preview_string`, `strings_get.used_by`,
//! `assets_probe`, `assets_upload`, `asset_thumbnail`, `assets_list` usage.

use std::sync::Arc;

use serde_json::{Value, json};

use super::*;
use crate::dialogs::NoDialogs;
use crate::events::RecordingSink;
use crate::testutil::example_copy;

fn app() -> (tempfile::TempDir, AppState) {
    let (tmp, dir) = example_copy();
    let app = AppState::new(
        AppPaths::in_dir(&tmp.path().join("app")),
        Arc::new(RecordingSink::default()),
        Arc::new(NoDialogs),
    )
    .without_watcher();
    app.open_project(&dir).unwrap();
    (tmp, app)
}

fn call(app: &AppState, name: &str, args: Value) -> Value {
    match dispatch(app, name, args) {
        Ok(Reply::Json(v)) => v,
        Ok(Reply::Image(_)) => panic!("{name} returned an image"),
        Err(e) => panic!("{name} failed: {e:?}"),
    }
}

fn image(app: &AppState, name: &str, args: Value) -> Arc<PreviewImage> {
    match dispatch(app, name, args) {
        Ok(Reply::Image(i)) => i,
        Ok(Reply::Json(v)) => panic!("{name} returned JSON {v}"),
        Err(e) => panic!("{name} failed: {e:?}"),
    }
}

#[test]
fn element_info_and_markup_check() {
    let (_tmp, app) = app();
    let info = call(
        &app,
        "element_info",
        json!({"target": "ios-iphone-69", "lang": "de", "id": "title-companions"}),
    );
    assert_eq!(info["element"]["props"]["kind"], "text");
    assert_eq!(info["element"]["props"]["key"], "companions.title");
    assert!(
        info["own_text"]
            .as_str()
            .unwrap()
            .contains("[accent]Pflanzen")
    );
    let device = call(
        &app,
        "element_info",
        json!({"target": "ios-iphone-69", "id": "phone-plan"}),
    );
    assert_eq!(device["element"]["props"]["screenshot"]["key"], "home");

    let check = call(
        &app,
        "markup_check",
        json!({"text": "a **b [accent]c[/] d"}),
    );
    assert_eq!(check["diagnostics"][0]["marker"], "**");
    assert_eq!(check["spans"], json!(["accent"]));

    let table = call(&app, "strings_get", json!({}));
    assert_eq!(
        table["used_by"]["companions.title"],
        json!(["title-companions"])
    );
}

#[test]
fn preview_string_renders_without_saving() {
    let (_tmp, app) = app();
    let view = json!({"target": "ios-iphone-69", "lang": "de", "image": 2, "width": 120});
    let base = image(&app, "render_image", view.clone());
    let mut args = view;
    args["key"] = json!("companions.title");
    args["value"] = json!("Ganz anderer Text mit einem Donau\\-dampfschiff");
    args["interactive"] = json!(false);
    let preview = image(&app, "render_preview_string", args);
    assert_eq!((preview.width, preview.height), (base.width, base.height));
    assert_ne!(preview.data, base.data);
    // Nothing was saved.
    let table = call(&app, "strings_get", json!({"lang": "de"}));
    assert!(
        !table["strings"]["de"]["companions.title"]
            .as_str()
            .unwrap()
            .contains("Donau")
    );
    assert_eq!(call(&app, "history_state", json!({}))["can_undo"], false);
}

#[test]
fn probe_upload_thumbnail_and_usage() {
    let (_tmp, app) = app();
    let probed = call(
        &app,
        "assets_probe",
        json!({"sizes": [[1320, 2868], [17, 23]]}),
    );
    assert_eq!(probed[0]["variant"], "ios-phone");
    assert_eq!(probed[1]["candidates"], json!([]));

    let list = call(&app, "assets_list", json!({}));
    assert_eq!(list["screenshot_keys"]["home"], json!(["phone-plan"]));
    assert!(
        list["variants"]
            .as_array()
            .unwrap()
            .contains(&json!("ios-phone"))
    );
    assert!(list["asset_usage"]["assets/logo.svg"].is_array());

    // A 1×1 PNG as base64.
    let png = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==";
    let out = call(
        &app,
        "assets_upload",
        json!({"name": "Tiny Dot.png", "data": png, "kind": "asset"}),
    );
    assert_eq!(out[0]["path"], "assets/tiny-dot.png");
    assert_eq!(out[0]["source"], "Tiny Dot.png");

    let thumb = image(
        &app,
        "asset_thumbnail",
        json!({"path": "screenshots/en/ios-phone/home.png", "max_edge": 64}),
    );
    assert_eq!(thumb.height, 64);
    assert!(thumb.width < 64);
    let svg = image(&app, "asset_thumbnail", json!({"path": "assets/logo.svg"}));
    assert!(svg.width.max(svg.height) <= THUMBNAIL_EDGE);
    assert!(dispatch(&app, "asset_thumbnail", json!({"path": "../x.png"})).is_err());
}
