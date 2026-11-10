//! `arrangia mcp` as a proxy to the GUI's in-app endpoint (ADR 0015).
//!
//! The test process plays the GUI: it hosts the MCP endpoint
//! (`arrangia_mcp::host::McpHost`) over its own `AppState` with the
//! discovery file in a temp app folder, then drives `arrangia mcp` (no
//! `--headless`) over stdio. Edits must land in the hosted session, named
//! after the stdio client.

#![allow(clippy::too_many_lines, clippy::many_single_char_names)]

mod common;

use std::path::Path;
use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, Instant};

use arrangia_app::commands::AppState;
use arrangia_app::dialogs::NoDialogs;
use arrangia_app::events::{PROJECT_EDITED, PROJECT_OPENED, RecordingSink};
use arrangia_app::settings::AppPaths;
use arrangia_mcp::ProgressRelay;
use arrangia_mcp::host::{HostOptions, McpHost};
use base64::Engine;
use common::{Client, path_str, png_size};
use serde_json::json;

/// A "GUI": app state with events recorded, endpoint hosted.
struct Gui {
    app: Arc<AppState>,
    events: Arc<RecordingSink>,
    host: Arc<McpHost>,
}

fn start_gui(app_dir: &Path, discovery: Option<std::path::PathBuf>) -> Gui {
    let events = Arc::new(RecordingSink::default());
    let relay = Arc::new(ProgressRelay::new(events.clone()));
    let paths = AppPaths::in_dir(app_dir);
    let app = Arc::new(AppState::new(paths, relay.clone(), Arc::new(NoDialogs)).without_watcher());
    let host = McpHost::install(
        &app,
        Some(relay),
        HostOptions {
            port: None,
            discovery,
        },
    );
    assert!(host.addr().is_some(), "{:?}", app.mcp_status());
    Gui { app, events, host }
}

fn wait_until(what: &str, mut f: impl FnMut() -> bool) {
    let start = Instant::now();
    while !f() {
        assert!(
            start.elapsed() < Duration::from_secs(20),
            "timed out: {what}"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn proxy_client(app_dir: &Path, cwd: &Path, name: &str, envs: &[(&str, &Path)]) -> Client {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_arrangia"));
    cmd.args(["mcp", "--app-dir", path_str(app_dir)])
        .current_dir(cwd);
    for (k, v) in envs {
        cmd.env(k, v);
    }
    Client::spawn(cmd, app_dir, name)
}

#[test]
fn proxy_forwards_to_the_hosted_session() {
    let tmp = tempfile::tempdir().unwrap();
    let app_dir = tmp.path().join("appdata");
    let discovery = arrangia_mcp::discovery::path(&AppPaths::in_dir(&app_dir));
    let gui = start_gui(&app_dir, Some(discovery.clone()));
    assert!(discovery.is_file());

    let mut c = proxy_client(&app_dir, tmp.path(), "claude-ai", &[]);
    let tools = c.request("tools/list", json!({}))["tools"].clone();
    assert!(tools.as_array().unwrap().len() > 40);

    // The assistant creates a project: the "GUI" opens it (event for the UI).
    let dir = tmp.path().join("Demo");
    let s = c.call(
        "project_create",
        json!({"dir": path_str(&dir), "name": "Demo", "template": "minimal", "langs": ["en", "de"]}),
    );
    assert_eq!(s["name"], "Demo");
    let session = gui
        .app
        .session()
        .expect("the hosted app opened the project");
    assert_eq!(session.dir(), dir.canonicalize().unwrap());
    let opened = gui.events.named(PROJECT_OPENED);
    assert_eq!(opened.last().unwrap()["actor"]["client"], "claude-ai");

    // The user looks at German: omitted languages default to it.
    let target = s["targets"][0]["id"].as_str().unwrap().to_owned();
    session.set_view(arrangia_app::ViewState {
        target: Some(target.clone()),
        lang: Some("de".into()),
        ..Default::default()
    });
    let r = c.call(
        "element_add",
        json!({"kind": "text", "id": "promo", "key": "promo.title", "text": "Hallo **Welt**",
               "fields": {"at": {"x": 500, "y": 560}, "width": 840}}),
    );
    assert_eq!(r["created"], json!(["promo"]));
    assert_eq!(r["step"], "Claude Desktop: Add text");
    let history = session.history_state();
    assert_eq!(history.undo.last().unwrap(), "Claude Desktop: Add text");
    let edited = gui.events.named(PROJECT_EDITED);
    let last = edited.last().unwrap();
    assert_eq!(
        last["actor"],
        json!({"kind": "assistant", "client": "claude-ai", "name": "Claude Desktop"})
    );
    assert_eq!(last["changes"]["elements"], json!(["promo"]));

    // A preview through the SSE/JSON answer path, as an image.
    let raw = c.call_raw("render_preview", json!({"max_edge": 400}));
    assert_eq!(raw["isError"], false, "{raw}");
    assert_eq!(
        raw["structuredContent"]["lang"], "de",
        "GUI language is the default"
    );
    assert_eq!(raw["structuredContent"]["target"], target.as_str());
    let png = base64::engine::general_purpose::STANDARD
        .decode(raw["content"][1]["data"].as_str().unwrap())
        .unwrap();
    let (w, h) = png_size(&png);
    assert!(w.max(h) <= 400 && w > 0);

    // view_get sees the GUI.
    let v = c.call("view_get", json!({}));
    assert_eq!(v["host"], "gui");

    // The status names the client while connected …
    let status = gui.app.mcp_status();
    let client = status
        .clients
        .iter()
        .find(|c| c.name == "claude-ai")
        .unwrap();
    assert_eq!(client.sessions, 1, "{status:?}");
    assert!(client.calls >= 4 && client.connected);
    // … and the session ends when the client goes away (proxy sends DELETE).
    assert!(c.close().success());
    wait_until("session closed", || {
        gui.app.mcp_status().clients.iter().all(|c| c.sessions == 0)
    });

    gui.host.stop();
    assert!(!discovery.exists(), "discovery file removed on stop");
}

/// Resources and the docs tool work through the proxy (feedback: the
/// assistant in Claude Desktop could not read `arrangia://docs/ops`).
#[test]
fn proxy_forwards_resources_and_docs() {
    let tmp = tempfile::tempdir().unwrap();
    let app_dir = tmp.path().join("appdata");
    let discovery = arrangia_mcp::discovery::path(&AppPaths::in_dir(&app_dir));
    let dir = tmp.path().join("p");
    arrangia_app::testutil::copy_project(&arrangia_app::testutil::example_fixture(), &dir).unwrap();
    let gui = start_gui(&app_dir, Some(discovery));
    gui.app.open_project(&dir).unwrap();
    let mut c = proxy_client(&app_dir, tmp.path(), "claude-ai", &[]);
    let list = c.request("resources/list", json!({}));
    let uris: Vec<&str> = list["resources"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|r| r["uri"].as_str())
        .collect();
    for uri in [
        "arrangia://project/project.yaml",
        "arrangia://project/i18n/de.yaml",
        "arrangia://docs/ops",
        "arrangia://docs/elements",
    ] {
        assert!(uris.contains(&uri), "{uri} in {uris:?}");
    }
    let templates = c.request("resources/templates/list", json!({}));
    assert_eq!(templates["resourceTemplates"].as_array().unwrap().len(), 1);
    let ops = c.request("resources/read", json!({"uri": "arrangia://docs/ops"}));
    assert!(
        ops["contents"][0]["text"]
            .as_str()
            .unwrap()
            .contains("set_lang_override")
    );
    let project = c.request(
        "resources/read",
        json!({"uri": "arrangia://project/project.yaml"}),
    );
    assert!(
        project["contents"][0]["text"]
            .as_str()
            .unwrap()
            .contains("format: arrangia/1")
    );
    // The same docs as a tool (clients that do not let the model read
    // resources).
    let r = c.call_raw("docs_get", json!({"topic": "elements"}));
    assert_eq!(r["isError"], false, "{r}");
    let text = r["content"][0]["text"].as_str().unwrap();
    assert!(
        text.contains("focus") && text.contains("kind: device"),
        "{text}"
    );
    gui.host.stop();
}

#[test]
fn proxy_survives_a_gui_restart() {
    let tmp = tempfile::tempdir().unwrap();
    let app_dir = tmp.path().join("appdata");
    let discovery = arrangia_mcp::discovery::path(&AppPaths::in_dir(&app_dir));
    let dir = tmp.path().join("p");
    arrangia_app::testutil::copy_project(&arrangia_app::testutil::example_fixture(), &dir).unwrap();
    let gui = start_gui(&app_dir, Some(discovery.clone()));
    gui.app.open_project(&dir).unwrap();
    let mut c = proxy_client(&app_dir, tmp.path(), "claude-code", &[]);
    assert_eq!(c.call("project_summary", json!({}))["name"], "Fernwood");
    // New port and token, sessions gone.
    gui.host.stop();
    gui.host.start().unwrap();
    assert_eq!(c.call("project_summary", json!({}))["name"], "Fernwood");
    gui.host.stop();
}

#[cfg(unix)]
#[test]
fn proxy_starts_the_gui_when_none_runs() {
    use std::os::unix::fs::PermissionsExt;

    let tmp = tempfile::tempdir().unwrap();
    let app_dir = tmp.path().join("appdata");
    let paths = AppPaths::in_dir(&app_dir);
    let discovery = arrangia_mcp::discovery::path(&paths);
    // The real endpoint, but its discovery file is held back: the fake
    // GUI binary "starts" by putting it in place.
    let staged = tmp.path().join("staged-mcp.json");
    let gui = start_gui(&app_dir, Some(staged.clone()));
    assert!(!discovery.exists());
    let launched = tmp.path().join("launched.txt");
    let fake = tmp.path().join("fake-gui.sh");
    std::fs::write(
        &fake,
        format!(
            "#!/bin/sh\necho \"$@\" > '{}'\nsleep 1\ncp '{}' \"$ARRANGIA_APP_DIR/data/mcp.json\"\n",
            launched.display(),
            staged.display()
        ),
    )
    .unwrap();
    std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
    let project = tmp.path().join("p");
    arrangia_app::testutil::copy_project(&arrangia_app::testutil::example_fixture(), &project)
        .unwrap();
    gui.app.open_project(&project).unwrap();

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_arrangia"));
    cmd.args([
        "--project",
        path_str(&project),
        "mcp",
        "--app-dir",
        path_str(&app_dir),
    ])
    .current_dir(tmp.path())
    .env("ARRANGIA_GUI", &fake);
    let mut c = Client::spawn(cmd, &app_dir, "claude-ai");
    let args = std::fs::read_to_string(&launched).unwrap();
    assert!(args.trim().ends_with("/p"), "GUI got the project: {args}");
    let v = c.call("view_get", json!({}));
    assert_eq!(v["host"], "gui", "proxied, not headless");
    drop(c);
    gui.host.stop();

    // Autostart off: headless right away.
    std::fs::remove_file(&discovery).ok();
    arrangia_app::settings::SettingsStore::new(paths.settings_file())
        .update(&json!({"mcp_autostart_gui": false}))
        .unwrap();
    std::fs::remove_file(&launched).unwrap();
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_arrangia"));
    cmd.args([
        "--project",
        path_str(&project),
        "mcp",
        "--app-dir",
        path_str(&app_dir),
    ])
    .current_dir(tmp.path())
    .env("ARRANGIA_GUI", &fake);
    let mut c = Client::spawn(cmd, &app_dir, "claude-ai");
    assert_eq!(c.call("view_get", json!({}))["host"], "headless");
    assert!(!launched.exists(), "no GUI started");
}

/// Raw HTTP/1.1 request to the endpoint; returns (status, body).
fn http(port: u16, head: &str, body: &str) -> (u16, String) {
    use std::io::{Read, Write};
    let mut s = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
    let req = format!(
        "{head}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    s.write_all(req.as_bytes()).unwrap();
    let mut out = String::new();
    let _ = s.read_to_string(&mut out);
    let status = out
        .split_whitespace()
        .nth(1)
        .and_then(|c| c.parse().ok())
        .unwrap_or(0);
    let body = out.split("\r\n\r\n").nth(1).unwrap_or_default().to_owned();
    (status, body)
}

#[test]
fn endpoint_refuses_missing_tokens_foreign_hosts_and_origins() {
    let tmp = tempfile::tempdir().unwrap();
    let app_dir = tmp.path().join("appdata");
    let gui = start_gui(&app_dir, None);
    let port = gui.host.addr().unwrap().port();
    let token = gui.host.token().unwrap();
    let init = json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {
        "protocolVersion": "2025-11-25", "capabilities": {},
        "clientInfo": {"name": "curl", "version": "1"}}})
    .to_string();
    let post = |extra: &str| {
        format!(
            "POST /mcp HTTP/1.1\r\nContent-Type: application/json\r\nAccept: application/json, text/event-stream{extra}"
        )
    };
    let host = format!("\r\nHost: 127.0.0.1:{port}");
    let auth = format!("\r\nAuthorization: Bearer {token}");

    assert_eq!(http(port, &post(&host), &init).0, 401, "no token");
    let wrong = "\r\nAuthorization: Bearer nope";
    assert_eq!(http(port, &post(&format!("{host}{wrong}")), &init).0, 401);
    let evil_host = format!("\r\nHost: evil.example:{port}{auth}");
    assert_eq!(http(port, &post(&evil_host), &init).0, 403, "DNS rebinding");
    let evil_origin = format!("{host}{auth}\r\nOrigin: https://evil.example");
    assert_eq!(
        http(port, &post(&evil_origin), &init).0,
        403,
        "browser origin"
    );
    let null_origin = format!("{host}{auth}\r\nOrigin: null");
    assert_eq!(http(port, &post(&null_origin), &init).0, 403);
    // Health: with the token only.
    let (code, body) = http(port, &format!("GET /health HTTP/1.1{host}{auth}"), "");
    assert_eq!(code, 200);
    assert!(body.contains("\"arrangia\""), "{body}");
    assert_eq!(
        http(port, &format!("GET /health HTTP/1.1{host}"), "").0,
        401
    );
    assert_eq!(
        http(port, &format!("GET /other HTTP/1.1{host}{auth}"), "").0,
        404
    );
    // Correct: token, loopback host and (optional) loopback origin.
    let ok = format!("{host}{auth}\r\nOrigin: http://localhost:5173");
    let (code, body) = http(port, &post(&ok), &init);
    assert_eq!(code, 200, "{body}");
    assert!(body.contains("\"serverInfo\""), "{body}");

    // Switched off in the settings: stops listening.
    arrangia_app::dispatch(&gui.app, "mcp_set_enabled", json!({"enabled": false})).unwrap();
    assert!(gui.host.addr().is_none());
    assert!(std::net::TcpStream::connect(("127.0.0.1", port)).is_err());
    assert!(!gui.app.mcp_status().running);
    assert!(gui.app.mcp_token().is_none());
    arrangia_app::dispatch(&gui.app, "mcp_set_enabled", json!({"enabled": true})).unwrap();
    assert!(gui.app.mcp_status().running);
    assert_ne!(gui.host.token().unwrap(), token, "new token per start");
    gui.host.stop();
}
