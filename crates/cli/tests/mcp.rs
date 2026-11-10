//! End-to-end tests of `arrangia mcp` (Phase 4.4): the binary runs as a
//! subprocess and is driven over stdio with newline-delimited JSON-RPC,
//! like Claude Code / Claude Desktop do.

// One long scripted session reads best as one test; JSON values are
// passed by value like `json!` literals.
#![allow(
    clippy::too_many_lines,
    clippy::many_single_char_names,
    clippy::needless_pass_by_value
)]

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{Receiver, RecvTimeoutError, channel};
use std::time::Duration;

use base64::Engine;
use serde_json::{Value, json};

/// Generous: debug builds render and export slowly.
const TIMEOUT: Duration = Duration::from_secs(240);

/// A minimal MCP client over the child's stdio.
struct Client {
    child: Child,
    stdin: ChildStdin,
    rx: Receiver<Value>,
    next_id: u64,
    /// Notifications received while waiting for responses.
    notifications: Vec<Value>,
}

impl Client {
    fn start(args: &[&str], app_dir: &Path, cwd: &Path) -> Client {
        let mut child = Command::new(env!("CARGO_BIN_EXE_arrangia"))
            .args(args)
            .env("ARRANGIA_APP_DIR", app_dir)
            .env("NO_COLOR", "1")
            .current_dir(cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .expect("start arrangia mcp");
        let stdin = child.stdin.take().expect("stdin");
        let stdout = child.stdout.take().expect("stdout");
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else { break };
                if line.trim().is_empty() {
                    continue;
                }
                let v: Value = serde_json::from_str(&line)
                    .unwrap_or_else(|e| panic!("stdout is not JSON-RPC ({e}): {line}"));
                if tx.send(v).is_err() {
                    break;
                }
            }
        });
        let mut c = Client {
            child,
            stdin,
            rx,
            next_id: 1,
            notifications: Vec::new(),
        };
        let init = c.request(
            "initialize",
            json!({
                "protocolVersion": "2025-11-25",
                "capabilities": {},
                "clientInfo": {"name": "test-client", "version": "1.0"},
            }),
        );
        assert_eq!(init["serverInfo"]["name"], "arrangia", "{init}");
        assert!(
            init["instructions"]
                .as_str()
                .unwrap()
                .contains("render_preview")
        );
        assert!(init["capabilities"]["tools"].is_object());
        assert!(init["capabilities"]["resources"].is_object());
        c.send(&json!({"jsonrpc": "2.0", "method": "notifications/initialized"}));
        c
    }

    fn send(&mut self, msg: &Value) {
        writeln!(self.stdin, "{msg}").expect("write to server");
        self.stdin.flush().expect("flush");
    }

    /// Sends a request and waits for its response; returns `result`.
    fn request(&mut self, method: &str, params: Value) -> Value {
        let id = self.next_id;
        self.next_id += 1;
        self.send(&json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}));
        loop {
            let msg = match self.rx.recv_timeout(TIMEOUT) {
                Ok(m) => m,
                Err(RecvTimeoutError::Timeout) => panic!("no answer to {method}"),
                Err(RecvTimeoutError::Disconnected) => panic!("server exited during {method}"),
            };
            if msg["id"] == json!(id) {
                assert!(msg.get("error").is_none(), "{method}: {msg}");
                return msg["result"].clone();
            }
            self.notifications.push(msg);
        }
    }

    /// Calls a tool; returns the whole `CallToolResult`.
    fn call_raw(&mut self, name: &str, args: Value) -> Value {
        self.request("tools/call", json!({"name": name, "arguments": args}))
    }

    /// Calls a tool that must succeed; returns its structured content.
    fn call(&mut self, name: &str, args: Value) -> Value {
        let r = self.call_raw(name, args.clone());
        assert_eq!(r["isError"], false, "{name} {args}: {r}");
        let text = r["content"][0]["text"].as_str().expect("text block");
        assert_ne!(text, "");
        r["structuredContent"].clone()
    }

    /// Calls a tool that must fail; returns its error object.
    fn call_err(&mut self, name: &str, args: Value) -> Value {
        let r = self.call_raw(name, args.clone());
        assert_eq!(r["isError"], true, "{name} {args} should fail: {r}");
        r["structuredContent"]["error"].clone()
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Width and height of PNG bytes (IHDR).
fn png_size(bytes: &[u8]) -> (u32, u32) {
    assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n", "not a PNG");
    let be = |i: usize| u32::from_be_bytes(bytes[i..i + 4].try_into().unwrap());
    (be(16), be(20))
}

fn path_str(p: &Path) -> &str {
    p.to_str().expect("utf-8 path")
}

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
        "strip_add",
        "target_add",
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
    assert!(r["step"].as_str().unwrap().starts_with("Claude: "), "{r}");
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
    assert_eq!(r["step"], "Claude: Set 2 de strings");
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
    if let Ok(out) = std::env::var("ARRANGIA_MCP_PREVIEW_OUT") {
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

    // History: Claude's steps are labelled; undo works.
    let h = c.call("history_state", json!({}));
    let undo = h["undo"].as_array().unwrap();
    assert_ne!(undo.len(), 0);
    assert!(
        undo.iter()
            .all(|l| l.as_str().unwrap().starts_with("Claude: ")),
        "{h}"
    );
    let r = c.call("history_undo", json!({}));
    assert_eq!(r["step"], "Claude: Set 2 de strings");
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
    assert!(
        uris.contains(&"arrangia://project/i18n/de.yaml"),
        "{uris:?}"
    );
    let r = c.request(
        "resources/read",
        json!({"uri": "arrangia://project/project.yaml"}),
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
