//! End-to-end tests of `vitrine mcp` (Phase 4.4): the binary runs as a
//! subprocess and is driven over stdio with newline-delimited JSON-RPC,
//! like MCP clients (Claude Code, Claude Desktop, …) do.

// One long scripted session reads best as one test; JSON values are
// passed by value like `json!` literals.
#![allow(
    clippy::too_many_lines,
    clippy::many_single_char_names,
    clippy::needless_pass_by_value
)]

mod common;

use std::path::{Path, PathBuf};

use base64::Engine;
use common::{Client, path_str, png_size};
use serde_json::{Value, json};

#[test]
fn mcp_session_creates_designs_translates_and_exports() {
    let tmp = tempfile::tempdir().unwrap();
    let app_dir = tmp.path().join("appdata");
    let mut c = Client::start(
        &["mcp", "--headless", "--app-dir", path_str(&app_dir)],
        &app_dir,
        tmp.path(),
    );

    // Tool list with schemas.
    let tools = c.request("tools/list", json!({}))["tools"].clone();
    let names: Vec<&str> = tools
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    for want in [
        "project_open",
        "project_create",
        "project_summary",
        "view_get",
        "selection_get",
        "catalog_devices",
        "catalog_targets",
        "canvas_add",
        "canvas_duplicate",
        "canvas_remove",
        "target_add",
        "banner_add",
        "element_add",
        "element_update",
        "element_move",
        "apply_ops",
        "strings_set",
        "strings_missing",
        "styles_update",
        "screenshots_import",
        "render_preview",
        "layout_report",
        "validate",
        "export",
        "export_status",
        "history_undo",
    ] {
        assert!(names.contains(&want), "tool {want} missing: {names:?}");
    }
    let update = tools
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["name"] == "element_update")
        .unwrap();
    let props = &update["inputSchema"]["properties"];
    assert!(props["patch"].is_object(), "{update}");
    assert!(props["scope"].is_object(), "{update}");
    assert!(update["description"].as_str().unwrap().contains("anchor_y"));
    let preview = tools
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["name"] == "render_preview")
        .unwrap();
    assert_eq!(preview["annotations"]["readOnlyHint"], true);

    // No project yet: a readable tool error.
    let e = c.call_err("project_summary", json!({}));
    assert_eq!(e["code"], "no_project");

    // Create a project.
    let dir = tmp.path().join("demo");
    let s = c.call(
        "project_create",
        json!({"dir": path_str(&dir), "name": "Demo", "template": "minimal", "langs": ["en", "de"]}),
    );
    assert_eq!(s["name"], "Demo");
    assert_eq!(s["languages"]["available"], json!(["en", "de"]));
    let target = s["targets"][0]["id"].as_str().unwrap().to_owned();
    assert!(dir.join("project.yaml").is_file());
    // Not into a non-empty folder.
    let e = c.call_err("project_create", json!({"dir": path_str(&dir)}));
    assert_eq!(e["code"], "io");

    // Add a device and a text.
    let r = c.call(
        "element_add",
        json!({"kind": "device", "id": "phone-2", "fields": {"rotation": -6, "width": 380, "at": {"x": 760, "y": 1500}}}),
    );
    assert_eq!(r["created"], json!(["phone-2"]));
    assert!(
        r["step"].as_str().unwrap().starts_with("Vitrine Test: "),
        "{r}"
    );
    let r = c.call(
        "element_add",
        json!({
            "kind": "text", "id": "promo", "key": "promo.title", "text": "Plan **faster**",
            "fields": {"at": {"x": 500, "y": 560}, "width": 840, "font_size": 64},
        }),
    );
    assert_eq!(r["created"], json!(["promo"]));
    let r = c.call("element_move", json!({"id": "promo", "x": 500, "y": 600}));
    assert_eq!(r["applied"], true);
    let info = c.call("element_get", json!({"id": "promo"}));
    assert_eq!(info["effective"]["at"]["y"], 600);

    // Strings in two languages (one step each; unchanged values are no-ops).
    let r = c.call(
        "strings_set",
        json!({"lang": "en", "values": {"promo.title": "Plan **faster**"}}),
    );
    assert_eq!(r["applied"], false);
    let r = c.call(
        "strings_set",
        json!({"lang": "de", "values": {"promo.title": "Schneller **planen**", "home.title": "Deine App auf einen Blick"}}),
    );
    assert_eq!(r["step"], "Vitrine Test: Set 2 de strings");
    let missing = c.call("strings_missing", json!({"lang": "de"}));
    assert_eq!(missing["missing"], json!([]));

    // Preview: an image content block with a real PNG.
    let raw = c.call_raw(
        "render_preview",
        json!({"target": target, "lang": "de", "max_edge": 800}),
    );
    assert_eq!(raw["isError"], false, "{raw}");
    let img = &raw["content"][1];
    assert_eq!(img["type"], "image");
    assert_eq!(img["mimeType"], "image/png");
    let png = base64::engine::general_purpose::STANDARD
        .decode(img["data"].as_str().unwrap())
        .unwrap();
    let (w, h) = png_size(&png);
    assert_eq!(h, 800);
    assert!(w > 300 && w < 400, "{w}x{h}");
    let info = &raw["structuredContent"];
    assert_eq!(info["width"], w);
    assert_eq!(info["height"], h);
    assert_eq!(info["images"], 1);
    if let Ok(out) = std::env::var("VITRINE_MCP_PREVIEW_OUT") {
        std::fs::write(out, &png).unwrap();
    }
    let gallery = c.call_raw("render_preview", json!({"view": "canvas"}));
    assert_eq!(gallery["content"][1]["type"], "image");

    // Layout report: page-relative boxes and text metrics.
    let rep = c.call("layout_report", json!({"lang": "de"}));
    let promo = rep["elements"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["id"] == "promo")
        .unwrap();
    assert_eq!(promo["page"], 0);
    assert!(promo["text"]["font_size"].as_f64().unwrap() > 0.0);
    assert_eq!(promo["text"]["overflow"], false);

    // Validate and export.
    let v = c.call("validate", json!({"preview": true}));
    assert_eq!(v["errors"], 0, "{v}");
    let raw = c.request(
        "tools/call",
        json!({"name": "export", "arguments": {"langs": ["en", "de"]}, "_meta": {"progressToken": "exp-1"}}),
    );
    assert_eq!(raw["isError"], false, "{raw}");
    let x = raw["structuredContent"].clone();
    let progress: Vec<&Value> = c
        .notifications
        .iter()
        .filter(|n| n["method"] == "notifications/progress")
        .collect();
    assert!(!progress.is_empty(), "no progress notifications");
    assert!(
        progress
            .iter()
            .all(|n| n["params"]["progressToken"] == "exp-1")
    );
    assert!(
        progress.last().unwrap()["params"]["total"]
            .as_f64()
            .unwrap()
            >= 2.0
    );
    let files = x["files"].as_array().unwrap();
    assert!(files.len() >= 2, "{x}");
    let out = PathBuf::from(x["out_dir"].as_str().unwrap());
    for f in files {
        let p = out.join(f["path"].as_str().unwrap());
        assert!(p.is_file(), "{} missing", p.display());
    }

    // A background export: the call returns at once, export_status waits
    // for the same result.
    let started = c.call(
        "export",
        json!({"langs": ["en"], "force": true, "wait_seconds": 0}),
    );
    let job = started["job"].as_str().unwrap().to_owned();
    assert!(
        started["status"] == "running" || started["status"] == "finished",
        "{started}"
    );
    let done = c.call("export_status", json!({"job": job, "wait_seconds": 120}));
    assert_eq!(done["status"], "finished", "{done}");
    assert!(done["files"].as_array().unwrap().len() >= 2, "{done}");
    // Only the gallery previews.
    let g = c.call(
        "export",
        json!({"langs": ["en"], "pages": false, "force": true}),
    );
    let gfiles = g["files"].as_array().unwrap();
    assert!(!gfiles.is_empty(), "{g}");
    assert!(
        gfiles
            .iter()
            .all(|f| f["path"].as_str().unwrap().ends_with("_gallery.png")),
        "{g}"
    );
    // The page files of the earlier export are kept.
    for f in files {
        assert!(out.join(f["path"].as_str().unwrap()).is_file());
    }

    // Gallery on the store's dark page, as JPEG, also written to a file.
    let raw = c.request(
        "tools/call",
        json!({"name": "render_preview", "arguments": {"view": "gallery", "background": "dark", "format": "jpeg", "out_file": "preview/gallery.jpg"}}),
    );
    assert_eq!(raw["isError"], false, "{raw}");
    assert_eq!(raw["content"][1]["mimeType"], "image/jpeg", "{raw}");
    let file = PathBuf::from(raw["structuredContent"]["file"].as_str().unwrap());
    assert!(std::fs::read(&file).unwrap().starts_with(&[0xFF, 0xD8]));

    // History: the assistant's steps carry its display name; undo works.
    let h = c.call("history_state", json!({}));
    let undo = h["undo"].as_array().unwrap();
    assert_ne!(undo.len(), 0);
    assert!(
        undo.iter()
            .all(|l| l.as_str().unwrap().starts_with("Vitrine Test: ")),
        "{h}"
    );
    let r = c.call("history_undo", json!({}));
    assert_eq!(r["step"], "Vitrine Test: Set 2 de strings");
    let de = c.call(
        "strings_get",
        json!({"lang": "de", "keys": ["promo.title"]}),
    );
    assert!(de["strings"]["de"].get("promo.title").is_none(), "{de}");
    c.call("history_redo", json!({}));

    // Errors the model can recover from.
    let e = c.call_err("element_get", json!({"id": "nope"}));
    assert_eq!(e["code"], "not_found");
    let e = c.call_err("element_move", json!({"id": "promo", "x": "left"}));
    assert_eq!(e["code"], "invalid_args");
    assert!(e["message"].as_str().unwrap().contains("element_move"));
    let e = c.call_err(
        "element_update",
        json!({"id": "promo", "scope": "lang", "patch": {}}),
    );
    assert_eq!(e["code"], "invalid_args");

    // Resources.
    let list = c.request("resources/list", json!({}));
    let uris: Vec<&str> = list["resources"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["uri"].as_str().unwrap())
        .collect();
    assert!(uris.contains(&"vitrine://project/i18n/de.yaml"), "{uris:?}");
    let r = c.request(
        "resources/read",
        json!({"uri": "vitrine://project/project.yaml"}),
    );
    assert!(
        r["contents"][0]["text"]
            .as_str()
            .unwrap()
            .contains("id: promo")
    );
}

#[test]
fn mcp_reloads_external_edits_and_reports_broken_files() {
    let tmp = tempfile::tempdir().unwrap();
    let app_dir = tmp.path().join("appdata");
    let dir = tmp.path().join("example");
    copy_dir(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/example"),
        &dir,
    );
    let mut c = Client::start(
        &[
            "mcp",
            "--headless",
            "--project",
            path_str(&dir),
            "--app-dir",
            path_str(&app_dir),
        ],
        &app_dir,
        tmp.path(),
    );
    let s = c.call("project_summary", json!({}));
    assert_eq!(s["name"], "Fernwood");
    let rev = s["revision"].as_u64().unwrap();
    c.call(
        "element_update",
        json!({"id": "title-plan", "patch": {"align": "left"}}),
    );
    assert_eq!(c.call("history_state", json!({}))["can_undo"], true);

    // External edit (another editor, git checkout …): seen by the next call.
    let file = dir.join("project.yaml");
    let text = std::fs::read_to_string(&file).unwrap();
    let rev_now = rev + 1;
    let edited = text
        .replace("name: Fernwood", "name: Fernwood Edited")
        .replace(
            &format!("revision: {rev_now}"),
            &format!("revision: {}", rev_now + 1),
        );
    assert_ne!(edited, text);
    std::fs::write(&file, &edited).unwrap();
    let s = c.call("project_summary", json!({}));
    assert_eq!(s["name"], "Fernwood Edited");
    assert_eq!(s["revision"], rev_now + 1);
    // The reload cleared the undo history (its steps no longer apply).
    assert_eq!(c.call("history_state", json!({}))["can_undo"], false);

    // A project.yaml that does not load: a clear error, nothing written.
    let broken = format!(
        "{}\nnot_a_field: 1\n",
        edited.replace(
            &format!("revision: {}", rev_now + 1),
            &format!("revision: {}", rev_now + 5)
        )
    );
    std::fs::write(&file, &broken).unwrap();
    let e = c.call_err(
        "element_update",
        json!({"id": "title-plan", "patch": {"align": "right"}}),
    );
    assert_eq!(e["code"], "load", "{e}");
    assert!(e["message"].as_str().unwrap().contains("does not load"));
    assert_eq!(std::fs::read_to_string(&file).unwrap(), broken);

    // Fixed on disk: work goes on with the file's content.
    std::fs::write(&file, &edited).unwrap();
    let r = c.call(
        "element_update",
        json!({"id": "title-plan", "patch": {"align": "right"}}),
    );
    assert_eq!(r["applied"], true);
    let saved = std::fs::read_to_string(&file).unwrap();
    assert!(saved.contains("name: Fernwood Edited"));
    assert!(
        saved.contains(&format!("revision: {}", rev_now + 2)),
        "{saved}"
    );

    // A broken project does not block switching to another one.
    std::fs::write(&file, &broken).unwrap();
    let other = tmp.path().join("other");
    let s = c.call(
        "project_create",
        json!({"dir": path_str(&other), "template": "minimal"}),
    );
    assert_eq!(s["name"], "other");
}

/// Feedback of a real session (2026-11-10): resetting overrides, several
/// language adjustments in one step, edits across canvases, docs without
/// resources, a mapping-list asset import.
#[test]
fn mcp_overrides_batches_docs_and_asset_mapping() {
    let tmp = tempfile::tempdir().unwrap();
    let app_dir = tmp.path().join("appdata");
    let dir = tmp.path().join("example");
    copy_dir(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/example"),
        &dir,
    );
    let mut c = Client::start(
        &[
            "mcp",
            "--headless",
            "--project",
            path_str(&dir),
            "--app-dir",
            path_str(&app_dir),
        ],
        &app_dir,
        tmp.path(),
    );
    let file = dir.join("project.yaml");

    // The reported case: a free text box moved for English only, then the
    // English position reset with `at: null`.
    c.call(
        "element_add",
        json!({"kind": "text", "page": 0, "id": "promo", "key": "promo.title", "text": "Hi",
               "fields": {"at": {"x": 500, "y": 600}, "width": 800}}),
    );
    c.call(
        "element_update",
        json!({"id": "promo", "scope": "lang", "lang": "en", "patch": {"at": {"x": 520, "y": 640}}}),
    );
    let e = c.call("element_get", json!({"id": "promo", "lang": "en"}));
    assert_eq!(e["lang_override"]["at"]["x"], 520, "{e}");
    let r = c.call(
        "element_update",
        json!({"id": "promo", "scope": "lang", "lang": "en", "patch": {"at": null}}),
    );
    assert_eq!(r["applied"], true);
    let e = c.call("element_get", json!({"id": "promo", "lang": "en"}));
    assert!(e.get("lang_override").is_none(), "{e}");
    assert_eq!(e["effective"]["at"], json!({"x": 500, "y": 600}));
    let yaml = std::fs::read_to_string(&file).unwrap();
    assert!(!yaml.contains("null"), "no null stored in overrides");

    // Several language adjustments in ONE undo step.
    let r = c.call(
        "element_update",
        json!({"updates": [
            {"id": "title-plan", "scope": "lang", "lang": "de", "patch": {"font_size": 60}},
            {"id": "title-plan", "scope": "lang", "lang": "fr", "patch": {"font_size": 62}},
        ]}),
    );
    assert_eq!(
        r["step"], "Vitrine Test: Edit font_size of title-plan",
        "{r}"
    );
    let de = c.call("element_get", json!({"id": "title-plan", "lang": "de"}));
    assert_eq!(de["lang_override"]["font_size"], 60);
    c.call("history_undo", json!({}));
    let fr = c.call("element_get", json!({"id": "title-plan", "lang": "fr"}));
    assert!(fr.get("lang_override").is_none(), "undone together: {fr}");

    // Ids of two canvases in one call (phone and mac).
    let r = c.call(
        "element_reorder",
        json!({"ids": ["title-plan", "mac-title-plan"], "order": "front"}),
    );
    assert_eq!(r["applied"], true);
    let r = c.call(
        "element_update",
        json!({"ids": ["title-beds", "mac-title-plan"], "patch": {"align": "left"}}),
    );
    assert_eq!(r["changed"]["elements"].as_array().unwrap().len(), 2, "{r}");

    // Explicit reset of override fields.
    let r = c.call(
        "element_reset_override",
        json!({"id": "title-companions", "scope": "lang", "lang": "de", "fields": ["box"]}),
    );
    assert_eq!(r["applied"], true);
    let de = c.call(
        "element_get",
        json!({"id": "title-companions", "lang": "de"}),
    );
    assert!(de["lang_override"].get("box").is_none(), "{de}");
    assert!(de["lang_override"].get("fit").is_some(), "{de}");

    // Override ops through apply_ops.
    let r = c.call(
        "apply_ops",
        json!({"ops": [
            {"op": "set_lang_override", "element": "title-plan", "lang": "de", "patch": {"font_size": 58}},
            {"op": "set_lang_override", "element": "title-plan", "lang": "fr", "patch": {"font_size": 59}},
            {"op": "reset_lang_override_field", "element": "title-companions", "lang": "de", "field": "fit"},
        ]}),
    );
    assert_eq!(r["applied"], true, "{r}");

    // Docs as a tool: every topic.
    for topic in [
        "overview", "elements", "ops", "project", "markup", "targets", "scopes",
    ] {
        let r = c.call_raw("docs_get", json!({"topic": topic}));
        assert_eq!(r["isError"], false, "{topic}: {r}");
        assert!(r["content"][0]["text"].as_str().unwrap().len() > 400);
    }
    let e = c.call_err("docs_get", json!({"topic": "nope"}));
    assert_eq!(e["code"], "invalid_args");

    // Mapping-list import: per-file kind/key/name, dry run first.
    let shot = dir.join("screenshots/en/phone-sync.png");
    let files = json!([
        {"path": shot, "kind": "screenshot", "key": "promo-shot", "lang": "de", "variant": ""},
        {"path": shot, "kind": "image", "name": "promo-bg"},
        {"path": tmp.path().join("missing.png"), "kind": "asset"},
    ]);
    let r = c.call("assets_import", json!({"files": files, "dry_run": true}));
    let items = r["files"].as_array().unwrap();
    assert_eq!(items.len(), 3);
    assert_eq!(items[0]["path"], "screenshots/de/promo-shot.png");
    assert_eq!(items[1]["path"], "assets/promo-bg.png");
    assert_eq!(items[2]["ok"], false);
    assert!(
        !dir.join("assets/promo-bg.png").exists(),
        "dry run copies nothing"
    );
    c.call("assets_import", json!({"files": files}));
    assert!(dir.join("screenshots/de/promo-shot.png").is_file());
    assert!(dir.join("assets/promo-bg.png").is_file());

    // Store configuration: read-only over MCP, `set_store` refused.
    let r = c.call("store_config_get", json!({}));
    assert_eq!(r["accounts"], json!([]));
    assert_eq!(r["project"]["store"], json!({}));
    assert!(r["project"]["effective"]["apple"].is_array());
    let e = c.call_err(
        "apply_ops",
        json!({"ops": [{"op": "set_store", "store": {"google": {"package_name": "com.x.y"}}}]}),
    );
    assert!(e.to_string().contains("not available over MCP"), "{e}");
    assert!(!std::fs::read_to_string(&file).unwrap().contains("com.x.y"));

    // Uploads: the plan works (blocked: no store mapped), executing is
    // refused headless (app setting `mcp_allow_headless_upload` is off).
    let tools = c.request("tools/list", json!({}))["tools"].clone();
    let names: Vec<&str> = tools
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    for want in [
        "upload_plan",
        "upload_execute",
        "upload_status",
        "upload_log",
    ] {
        assert!(names.contains(&want), "tool {want} missing");
    }
    let execute = tools
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["name"] == "upload_execute")
        .unwrap();
    assert!(
        execute["description"]
            .as_str()
            .unwrap()
            .contains("OUTWARD-FACING")
    );
    let plan = c.call("upload_plan", json!({}));
    assert_eq!(plan["blocked"], true);
    assert_eq!(plan["problems"][0]["code"], "no_store");
    let e = c.call_err(
        "upload_execute",
        json!({"confirm_token": plan["confirm_token"]}),
    );
    assert_eq!(e["code"], "forbidden", "{e}");
    assert!(e.to_string().contains("mcp_allow_headless_upload"));
    let s = c.call("upload_status", json!({}));
    assert!(s["run"].is_null());
    let l = c.call("upload_log", json!({}));
    assert_eq!(l["runs"], json!([]));
}

fn copy_dir(src: &Path, dst: &Path) {
    std::fs::create_dir_all(dst).unwrap();
    for entry in std::fs::read_dir(src).unwrap() {
        let entry = entry.unwrap();
        let name = entry.file_name();
        if ["export", "golden", ".cache"].iter().any(|n| name == *n) {
            continue;
        }
        let to = dst.join(&name);
        if entry.file_type().unwrap().is_dir() {
            copy_dir(&entry.path(), &to);
        } else {
            std::fs::copy(entry.path(), &to).unwrap();
        }
    }
}
