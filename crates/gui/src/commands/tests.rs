use std::sync::Arc;

use serde_json::{Value, json};

use super::*;
use crate::dialogs::{FixedDialogs, NoDialogs};
use crate::events::RecordingSink;
use crate::preview::PixelFormat;
use crate::testutil::example_copy;

struct Fixture {
    _tmp: tempfile::TempDir,
    dir: PathBuf,
    app: AppState,
    sink: Arc<RecordingSink>,
}

fn fixture() -> Fixture {
    let (tmp, dir) = example_copy();
    let sink = Arc::new(RecordingSink::default());
    let app = AppState::new(
        AppPaths::in_dir(&tmp.path().join("app")),
        sink.clone(),
        Arc::new(NoDialogs),
    )
    .without_watcher();
    Fixture {
        _tmp: tmp,
        dir,
        app,
        sink,
    }
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

fn open(f: &Fixture) -> Value {
    call(&f.app, "project_open", json!({"dir": f.dir}))
}

#[test]
fn needs_a_project_and_known_commands() {
    let f = fixture();
    let e = dispatch(&f.app, "project_summary", Value::Null).unwrap_err();
    assert_eq!(e.code, ErrorCode::NoProject);
    let e = dispatch(&f.app, "nope", Value::Null).unwrap_err();
    assert_eq!(e.code, ErrorCode::UnknownCommand);
    let e = dispatch(&f.app, "project_open", json!({"folder": "x"})).unwrap_err();
    assert_eq!(e.code, ErrorCode::InvalidArgs);
    assert_eq!(call(&f.app, "app_info", Value::Null)["name"], "Arrangia");
}

#[test]
fn open_summarizes_and_records_recent() {
    let f = fixture();
    let s = open(&f);
    assert_eq!(s["name"], "Fernwood");
    assert_eq!(s["languages"]["default"], "en");
    let phone = &s["strips"][0];
    assert_eq!(phone["id"], "phone");
    assert_eq!(phone["page"]["count"], 6);
    assert_eq!(phone["page"]["ids"][0], "plan");
    let title = phone["elements"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["id"] == "title-plan")
        .unwrap();
    assert_eq!(title["kind"], "text");
    assert!(title["label"].as_str().unwrap().len() > 3);
    let targets = s["targets"].as_array().unwrap();
    assert!(
        targets
            .iter()
            .any(|t| t["id"] == "ios-ipad-13-landscape" && t["group"] == 2)
    );

    let recent = call(&f.app, "recent_projects", Value::Null);
    assert_eq!(recent[0]["name"], "Fernwood");
    assert_eq!(recent[0]["exists"], true);

    let model = call(&f.app, "project_get", Value::Null);
    assert_eq!(model["project"]["name"], "Fernwood");
    assert!(model["strings"]["de"].is_object());

    call(&f.app, "project_close", Value::Null);
    assert!(dispatch(&f.app, "project_summary", Value::Null).is_err());
}

#[test]
fn new_project_from_template() {
    let f = fixture();
    call(
        &f.app,
        "settings_set",
        json!({"settings": {"default_project_language": "de", "default_frame_style": "stylized"}}),
    );
    let dir = f.dir.parent().unwrap().join("new.arrangia");
    let s = call(
        &f.app,
        "project_new",
        json!({"dir": dir, "template": "minimal"}),
    );
    assert_eq!(s["name"], "new");
    assert_eq!(s["languages"]["default"], "de");
    assert_eq!(s["styles"]["frame"], "stylized");
    assert!(dir.join("i18n/de.yaml").is_file());
    let e = dispatch(&f.app, "project_new", json!({"dir": dir})).unwrap_err();
    assert_eq!(e.code, ErrorCode::Io);
}

#[test]
fn apply_undo_redo_via_commands() {
    let f = fixture();
    let rev = open(&f)["revision"].as_u64().unwrap();
    let r = call(
        &f.app,
        "apply_ops",
        json!({"ops": [{"op": "set_name", "name": "Grove"}], "label": "Rename"}),
    );
    assert_eq!(r["revision"], rev + 1);
    assert_eq!(r["changes"]["name"], true);
    assert_eq!(
        call(&f.app, "history_state", Value::Null)["undo"][0],
        "Rename"
    );
    let u = call(&f.app, "undo", Value::Null);
    assert_eq!(u["history"]["can_redo"], true);
    call(&f.app, "redo", Value::Null);
    assert_eq!(
        call(&f.app, "project_summary", Value::Null)["name"],
        "Grove"
    );
    let e = dispatch(
        &f.app,
        "apply_ops",
        json!({"ops": [{"op": "remove_target", "id": "nope"}]}),
    )
    .unwrap_err();
    assert_eq!(e.code, ErrorCode::OpRejected);
}

#[test]
#[allow(clippy::many_single_char_names)]
fn render_commands_cache_and_invalidate() {
    let f = fixture();
    open(&f);
    let args = json!({"target": "ios-iphone-69", "image": 0, "max_edge": 400});
    let a = image(&f.app, "render_image", args.clone());
    assert_eq!(a.format, PixelFormat::Png);
    assert_eq!(a.height, 400);
    assert!(a.data.starts_with(b"\x89PNG"));
    let b = image(&f.app, "render_image", args.clone());
    assert!(Arc::ptr_eq(&a, &b), "second render is a cache hit");
    let stats = call(&f.app, "preview_stats", Value::Null);
    assert_eq!(stats["hits"], 1);

    // Raw RGBA, interactive = half size.
    let raw = image(
        &f.app,
        "render_image",
        json!({"target": "ios-iphone-69", "max_edge": 400, "interactive": true, "format": "rgba"}),
    );
    assert_eq!(raw.height, 200);
    assert_eq!(raw.data.len(), (raw.width * raw.height * 4) as usize);

    // An op changing a text on page 1 → new scene hash → re-rendered.
    call(
        &f.app,
        "apply_ops",
        json!({"ops": [{"op": "set_string", "lang": "en", "key": "plan.title", "value": "Changed"}]}),
    );
    let c = image(&f.app, "render_image", args.clone());
    assert!(!Arc::ptr_eq(&a, &c));
    assert_ne!(a.data, c.data);
    // A German preview is not affected by an English text change.
    let de = json!({"target": "ios-iphone-69", "lang": "de", "max_edge": 400});
    let d1 = image(&f.app, "render_image", de.clone());
    call(
        &f.app,
        "apply_ops",
        json!({"ops": [{"op": "set_string", "lang": "en", "key": "plan.title", "value": "Again"}]}),
    );
    assert!(Arc::ptr_eq(&d1, &image(&f.app, "render_image", de)));

    // External screenshot change → entries using it are dropped.
    let session = f.app.session().unwrap();
    let before = session.preview().stats().entries;
    let shot = f.dir.join("screenshots/en/ios-phone/home.png");
    std::fs::copy(f.dir.join("screenshots/en/ios-phone/planner.png"), &shot).unwrap();
    session.handle_fs_changes(&[shot]);
    assert!(session.preview().stats().entries < before);
    let ev = f.sink.named(crate::events::PROJECT_CHANGED);
    assert_eq!(ev.last().unwrap()["reason"], "assets");

    let canvas = image(
        &f.app,
        "render_canvas",
        json!({"target": "ios-iphone-69", "scale": 0.05}),
    );
    assert!(canvas.width > canvas.height);
    assert!((canvas.scale - 0.05).abs() < 1e-4, "{}", canvas.scale);
    let gallery = image(
        &f.app,
        "render_gallery",
        json!({"target": "ios-iphone-69", "height": 200}),
    );
    assert!(gallery.width > gallery.height);
    let e = dispatch(&f.app, "render_gallery", json!({"target": "blog-header"})).unwrap_err();
    assert_eq!(e.code, ErrorCode::InvalidArgs);
    let e = dispatch(&f.app, "render_image", json!({"image": 99})).unwrap_err();
    assert_eq!(e.code, ErrorCode::NotFound);
}

#[test]
fn validate_strings_catalog() {
    let f = fixture();
    open(&f);
    let v = call(
        &f.app,
        "validate",
        json!({"targets": ["ios-iphone-69"], "preview": true}),
    );
    assert!(v["findings"].is_array());
    assert_eq!(v["errors"], 0, "{v}");
    let s = call(&f.app, "strings_get", json!({"lang": "de"}));
    assert!(s["strings"]["de"]["plan.title"].is_string());
    assert!(s["strings"].get("en").is_none());
    let missing = call(&f.app, "strings_missing", Value::Null);
    assert!(
        missing
            .as_array()
            .unwrap()
            .iter()
            .all(|m| m["lang"] == "de" || m["lang"] == "en")
    );
    assert!(
        call(&f.app, "catalog_devices", Value::Null)
            .as_array()
            .unwrap()
            .len()
            > 5
    );
    assert!(
        call(&f.app, "catalog_targets", Value::Null)
            .as_array()
            .unwrap()
            .len()
            > 5
    );
    assert_ne!(call(&f.app, "catalog_galleries", Value::Null), json!([]));
    let fonts = call(&f.app, "catalog_fonts", Value::Null);
    assert!(
        fonts["bundled"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f == "Inter")
    );
}

#[test]
fn export_emits_progress() {
    let f = fixture();
    open(&f);
    let out = f.dir.join("out");
    let r = call(
        &f.app,
        "export",
        json!({"targets": ["blog-header"], "langs": ["en"], "out_dir": out}),
    );
    assert_eq!(r["cancelled"], false);
    assert_eq!(r["files"].as_array().unwrap().len(), 1);
    let progress = f.sink.named(crate::events::EXPORT_PROGRESS);
    assert_eq!(progress.len(), 1);
    assert_eq!(progress[0]["total"], 1);
    assert_eq!(f.sink.named(crate::events::EXPORT_FINISHED).len(), 1);
    assert_eq!(
        call(&f.app, "export_cancel", Value::Null)["cancelled"],
        false
    );
}

#[test]
fn asset_import_names_and_variants() {
    let f = fixture();
    open(&f);
    let src_dir = f.dir.parent().unwrap().join("incoming");
    std::fs::create_dir_all(&src_dir).unwrap();
    let src = src_dir.join("Home Screen (1).PNG");
    std::fs::copy(f.dir.join("screenshots/en/ios-phone/home.png"), &src).unwrap();
    let r = call(
        &f.app,
        "assets_import",
        json!({"paths": [src], "kind": "screenshot", "lang": "de"}),
    );
    assert_eq!(r[0]["path"], "screenshots/de/ios-phone/home-screen-1.png");
    assert_eq!(r[0]["variant"], "ios-phone");
    assert_eq!(r[0]["detected"], true);
    // Same name again: suffixed; with a key and `replace`: replaced.
    let again = call(
        &f.app,
        "assets_import",
        json!({"paths": [src], "kind": "screenshot", "lang": "de"}),
    );
    assert_eq!(
        again[0]["path"],
        "screenshots/de/ios-phone/home-screen-1-2.png"
    );
    let keyed = call(
        &f.app,
        "assets_import",
        json!({"paths": [src], "kind": "screenshot", "lang": "de", "key": "home", "replace": true}),
    );
    assert_eq!(keyed[0]["path"], "screenshots/de/ios-phone/home.png");
    let e = dispatch(
        &f.app,
        "assets_import",
        json!({"paths": [src], "kind": "font"}),
    )
    .unwrap_err();
    assert_eq!(e.code, ErrorCode::InvalidArgs);

    let list = call(&f.app, "assets_list", Value::Null);
    let shot = list["screenshots"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["path"] == "screenshots/de/ios-phone/home-screen-1.png")
        .unwrap()
        .clone();
    assert_eq!(shot["key"], "home-screen-1");
    assert_eq!(shot["variant"], "ios-phone");
    assert!(shot["width"].as_u64().unwrap() > 0);
    assert!(
        list["assets"]
            .as_array()
            .unwrap()
            .iter()
            .any(|a| a["path"] == "assets/logo.svg")
    );

    let removed = call(
        &f.app,
        "assets_remove",
        json!({"paths": ["screenshots/de/ios-phone/home-screen-1-2.png"]}),
    );
    assert_eq!(removed.as_array().unwrap().len(), 1);
    for bad in ["../x.png", "project.yaml", "screenshots"] {
        assert!(dispatch(&f.app, "assets_remove", json!({"paths": [bad]})).is_err());
    }
}

#[test]
fn settings_and_dialogs() {
    let (tmp, dir) = example_copy();
    let app = AppState::new(
        AppPaths::in_dir(&tmp.path().join("app")),
        Arc::new(RecordingSink::default()),
        Arc::new(FixedDialogs {
            folder: Some(dir.clone()),
            files: vec![dir.join("assets/logo.svg")],
        }),
    )
    .without_watcher();
    let s = call(&app, "settings_get", Value::Null);
    assert_eq!(s["settings"]["default_project_language"], "en");
    let s = call(
        &app,
        "settings_set",
        json!({"settings": {"ui_language": "de"}}),
    );
    assert_eq!(s["ui_language"], "de");
    assert!(
        dispatch(
            &app,
            "settings_set",
            json!({"settings": {"ui_language": "fr"}})
        )
        .is_err()
    );
    let opened = call(&app, "dialog_open_project", Value::Null);
    assert_eq!(opened["name"], "Fernwood");
    let files = call(&app, "dialog_pick_files", json!({"kind": "asset"}));
    assert_eq!(files.as_array().unwrap().len(), 1);
}
