//! `vitrine-devbridge`: serves `crates/gui/ui/` and the GUI commands over
//! HTTP on 127.0.0.1 so the UI runs in a normal browser (dev only, never
//! bundled; ARCHITECTURE.md §12.2).
//!
//! ```sh
//! cargo run -p vitrine-gui --features devbridge --bin vitrine-devbridge -- \
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
//! discovery file in `<app-dir>/data/mcp.json`, so `vitrine mcp
//! --app-dir <app-dir>` or a script can drive assistant edits while the
//! browser shows them). "Add to Claude Desktop" always writes
//! `<app-dir>/claude_desktop_config.json` here, never the real config;
//! "Install command line tool" links into `<app-dir>/bin` (macOS/Linux;
//! on Windows it would change the real user PATH). Updates (`update_*`)
//! are GUI-only and absent here.
//! Store account secrets go to an in-memory keychain (lost on exit) unless
//! `--system-keychain` is given (then `VITRINE_KEYRING_SERVICE` decides
//! the service name; use a test name). "Test connection" calls the real
//! stores (read-only) unless `VITRINE_ASC_BASE_URL` /
//! `VITRINE_PLAY_TOKEN_URL` point to a loopback mock server.

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;

use vitrine_gui::bridge::{SseHub, serve};
use vitrine_gui::commands::AppState;
use vitrine_gui::dialogs::FixedDialogs;
use vitrine_gui::settings::AppPaths;
use vitrine_mcp::ProgressRelay;
use vitrine_mcp::host::{HostOptions, McpHost};

#[derive(Debug, Default)]
struct Options {
    port: u16,
    project: Option<PathBuf>,
    ui: Option<PathBuf>,
    app_dir: Option<PathBuf>,
    dialogs: FixedDialogs,
    copy: bool,
    mcp: bool,
    system_keychain: bool,
}

const USAGE: &str = "usage: vitrine-devbridge [--port N] [--project DIR] [--ui DIR] [--app-dir DIR] [--pick-folder DIR] [--pick-file FILE]... [--save-file FILE] [--copy] [--mcp] [--system-keychain]";

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
            "--system-keychain" => o.system_keychain = true,
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
        .unwrap_or_else(|| std::env::temp_dir().join("vitrine-devbridge"));
    let hub = Arc::new(SseHub::default());
    let relay = Arc::new(ProgressRelay::new(hub.clone()));
    let paths = AppPaths::in_dir(&app_dir);
    let discovery = vitrine_mcp::discovery::path(&paths);
    let mut state = AppState::new(paths, relay.clone(), Arc::new(o.dialogs))
        .with_claude_desktop_config(app_dir.join("claude_desktop_config.json"))
        .with_cli_link_dir(app_dir.join("bin"));
    if !o.system_keychain {
        state = state.with_secret_store(vitrine_gui::stores::credentials::SecretStore::mock());
    }
    // Store connection test with the real clients; `VITRINE_ASC_BASE_URL`,
    // `VITRINE_PLAY_TOKEN_URL` (loopback only) point it at a mock server.
    state.set_connection_tester(Arc::new(
        vitrine_gui::store_test::StoreClientTester::from_env(),
    ));
    let app = Arc::new(state);
    let project = match (&o.project, o.copy) {
        (Some(dir), true) => {
            let copy = app_dir.join("project-copy");
            let _ = std::fs::remove_dir_all(&copy);
            if let Err(e) = vitrine_gui::testutil::copy_project(dir, &copy) {
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
    println!("Vitrine Studio dev bridge: {}", server.url());
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
