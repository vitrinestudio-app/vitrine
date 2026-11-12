//! Template commands: save from the open project, list, preview, insert
//! (one undo step), export/import, new project from a stored template.

use std::sync::Arc;

use serde_json::{Value, json};

use super::*;
use crate::dialogs::NoDialogs;
use crate::events::RecordingSink;
use crate::testutil::example_copy;

fn call(app: &AppState, name: &str, args: Value) -> Value {
    match dispatch(app, name, args) {
        Ok(Reply::Json(v)) => v,
        Ok(Reply::Image(_)) => panic!("{name} returned an image"),
        Err(e) => panic!("{name} failed: {e:?}"),
    }
}

#[test]
fn save_insert_undo_export_import_and_new_project() {
    let (tmp, dir) = example_copy();
    let app = AppState::new(
        AppPaths::in_dir(&tmp.path().join("app")),
        Arc::new(RecordingSink::default()),
        Arc::new(NoDialogs),
    )
    .without_watcher();
    let app = &app;
    call(app, "project_open", json!({"dir": dir}));

    let saved = call(
        app,
        "template_save",
        json!({"name": "Mac set", "canvases": ["mac"],
               "tags": {"style": ["minimal"], "color": ["light"], "use": ["productivity"]}}),
    );
    assert_eq!(saved["template"]["id"], "mac-set");
    assert_eq!(saved["template"]["tags"]["style"], json!(["minimal"]));
    assert_eq!(saved["template"]["devices"], json!(["mac"]));
    let bad = dispatch(
        app,
        "template_save",
        json!({"name": "Bad", "canvases": ["mac"], "tags": {"style": ["fancy"]}}),
    )
    .unwrap_err();
    assert!(bad.message.contains("allowed: minimal"), "{bad:?}");
    let tags = call(app, "template_tags", json!({}));
    assert_eq!(tags["facets"].as_array().unwrap().len(), 4);
    let with_samples = call(
        app,
        "template_save",
        json!({"name": "Phone samples", "canvases": ["phone"], "sample_screenshots": true}),
    );
    assert!(with_samples["template"]["samples"].as_u64().unwrap() > 0);
    let samples = with_samples["report"]["samples"].as_array().unwrap();
    assert_ne!(samples.len(), 0);
    // Re-encoded as JPEG (quality 60), smaller than the project's PNGs.
    assert!(
        samples.iter().all(|s| s.as_str().unwrap().ends_with("jpg")),
        "{samples:?}"
    );
    assert!(
        with_samples["report"]["samples_bytes"].as_u64().unwrap()
            < with_samples["report"]["samples_bytes_original"]
                .as_u64()
                .unwrap()
    );
    assert!(saved["template"]["canvases"][0]["preview"].is_string());
    let list = call(app, "template_list", json!({}));
    assert!(
        list.as_array()
            .unwrap()
            .iter()
            .any(|t| t["id"] == "mac-set")
    );
    match dispatch(app, "template_preview", json!({"id": "minimal"})) {
        Ok(Reply::Image(img)) => assert!(img.width > 0 && img.data.starts_with(b"\x89PNG")),
        other => panic!("{other:?}"),
    }

    // Insert into the same project: renamed, one undo step.
    let before = app.session().unwrap().snapshot().canvases.len();
    let plan = call(app, "template_insert_plan", json!({"template": "mac-set"}));
    assert_eq!(plan["canvases"][0]["to"], "mac-2");
    let r = call(app, "template_insert", json!({"template": "mac-set"}));
    assert_eq!(r["report"]["canvases"][0]["to"], "mac-2");
    assert_eq!(app.session().unwrap().snapshot().canvases.len(), before + 1);
    call(app, "undo", json!({}));
    assert_eq!(app.session().unwrap().snapshot().canvases.len(), before);

    // Export, remove, import again.
    let file = tmp.path().join("mac-set");
    let out = call(
        app,
        "template_export",
        json!({"id": "mac-set", "path": file}),
    );
    let path = out["path"].as_str().unwrap().to_owned();
    assert!(path.ends_with(".vitrine-template"));
    call(app, "template_remove", json!({"id": "mac-set"}));
    let t = call(app, "template_import", json!({"path": path}));
    assert_eq!(t["id"], "mac-set");
    assert!(dispatch(app, "template_import", json!({"path": path})).is_err());

    // New project from the stored template.
    let new_dir = tmp.path().join("fresh");
    let s = call(
        app,
        "project_new",
        json!({"dir": new_dir, "template": "mac-set", "langs": ["de"]}),
    );
    assert_eq!(s["canvases"][0]["id"], "mac");
    assert_eq!(s["languages"]["default"], "de");
    assert!(dispatch(app, "template_remove", json!({"id": "phone"})).is_err());
}
