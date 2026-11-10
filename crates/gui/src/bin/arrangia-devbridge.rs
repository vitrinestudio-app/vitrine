//! `arrangia-devbridge`: serves `crates/gui/ui/` and the GUI commands over
//! HTTP on 127.0.0.1 so the UI runs in a normal browser (dev only, never
//! bundled; docs/adr/0008).
//!
//! ```sh
//! cargo run -p arrangia-gui --features devbridge --bin arrangia-devbridge -- \
//!     --project tests/fixtures/example
//! ```
//!
//! Options: `--port N` (default 0 = free port), `--project DIR` (open at
//! start), `--ui DIR` (default `crates/gui/ui`), `--app-dir DIR` (settings
//! and recent list; default a temp folder, so the real app data is left
//! alone), `--pick-folder DIR` / `--pick-file FILE` / `--save-file FILE`
//! (answers of the folder/file/save dialogs; otherwise dialogs are
//! cancelled), `--copy` (open a
//! temporary copy of `--project`, so edits never touch e.g. the test
//! fixture), `--mcp` (also host the MCP endpoint like the GUI, with the
//! discovery file in `<app-dir>/data/mcp.json`, so `arrangia mcp
//! --app-dir <app-dir>` or a script can drive Claude-like edits while the
//! browser shows them). "Add to Claude Desktop" always writes
//! `<app-dir>/claude_desktop_config.json` here, never the real config.

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;

use arrangia_gui::bridge::{SseHub, serve};
use arrangia_gui::commands::AppState;
use arrangia_gui::dialogs::FixedDialogs;
use arrangia_gui::settings::AppPaths;
use arrangia_mcp::ProgressRelay;
use arrangia_mcp::host::{HostOptions, McpHost};

#[derive(Debug, Default)]
struct Options {
    port: u16,
    project: Option<PathBuf>,
    ui: Option<PathBuf>,
    app_dir: Option<PathBuf>,
    dialogs: FixedDialogs,
    copy: bool,
    mcp: bool,
}

const USAGE: &str = "usage: arrangia-devbridge [--port N] [--project DIR] [--ui DIR] [--app-dir DIR] [--pick-folder DIR] [--pick-file FILE]... [--save-file FILE] [--copy] [--mcp]";

fn parse() -> Result<Options, String> {
    let mut o = Options::default();
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        let mut value = || args.next().ok_or_else(|| format!("{a} needs a value"));
        match a.as_str() {
            "--port" => o.port = value()?.parse().map_err(|e| format!("--port: {e}"))?,
            "--project" => o.project = Some(value()?.into()),
            "--ui" => o.ui = Some(value()?.into()),
            "--app-dir" => o.app_dir = Some(value()?.into()),
            "--pick-folder" => o.dialogs.folder = Some(value()?.into()),
            "--pick-file" => o.dialogs.files.push(value()?.into()),
            "--save-file" => o.dialogs.save = Some(value()?.into()),
            "--copy" => o.copy = true,
            "--mcp" => o.mcp = true,
            "-h" | "--help" => return Err(USAGE.to_owned()),
            _ => return Err(format!("unknown argument `{a}`\n{USAGE}")),
        }
    }
    Ok(o)
}

fn main() -> ExitCode {
    let o = match parse() {
        Ok(o) => o,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(2);
        }
    };
    let ui =
        o.ui.unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("ui"));
    let app_dir = o
        .app_dir
        .unwrap_or_else(|| std::env::temp_dir().join("arrangia-devbridge"));
    let hub = Arc::new(SseHub::default());
    let relay = Arc::new(ProgressRelay::new(hub.clone()));
    let paths = AppPaths::in_dir(&app_dir);
    let discovery = arrangia_mcp::discovery::path(&paths);
    let app = Arc::new(
        AppState::new(paths, relay.clone(), Arc::new(o.dialogs))
            .with_claude_desktop_config(app_dir.join("claude_desktop_config.json")),
    );
    let project = match (&o.project, o.copy) {
        (Some(dir), true) => {
            let copy = app_dir.join("project-copy");
            let _ = std::fs::remove_dir_all(&copy);
            if let Err(e) = arrangia_gui::testutil::copy_project(dir, &copy) {
                eprintln!("cannot copy `{}`: {e}", dir.display());
                return ExitCode::from(2);
            }
            Some(copy)
        }
        (dir, _) => dir.clone(),
    };
    if let Some(dir) = &project
        && let Err(e) = app.open_project(dir)
    {
        eprintln!("cannot open `{}`: {e}", dir.display());
        return ExitCode::from(2);
    }
    let mcp = o.mcp.then(|| {
        McpHost::install(
            &app,
            Some(relay),
            HostOptions {
                port: None,
                discovery: Some(discovery.clone()),
            },
        )
    });
    let server = match serve(app, hub, ui.clone(), o.port) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("cannot start the bridge: {e}");
            return ExitCode::from(2);
        }
    };
    println!("Arrangia dev bridge: {}", server.url());
    println!("  ui: {}  app data: {}", ui.display(), app_dir.display());
    if let Some(dir) = &project {
        println!("  project: {}", dir.display());
    }
    if let Some(url) = mcp.as_ref().and_then(|h| h.url()) {
        println!("  mcp: {url} (discovery {})", discovery.display());
    }
    server.join();
    if let Some(h) = mcp {
        h.stop();
    }
    ExitCode::SUCCESS
}
