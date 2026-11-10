//! `arrangia mcp`: the stdio entry point MCP clients start themselves
//! (ADR 0014, ADR 0015).
//!
//! 1. A running Arrangia GUI ([`discover_gui`]: discovery file in the app
//!    data folder, health check) → forward stdio to its in-app endpoint
//!    ([`crate::proxy`], feature `proxy`), so the assistant edits the session the
//!    user sees.
//! 2. None, and the app setting `mcp_autostart_gui` is on (default) →
//!    start the GUI ([`launch_gui`]), wait up to [`GUI_START_TIMEOUT`] for
//!    its discovery file, then proxy.
//! 3. Otherwise (and always with `--headless`) serve headless over stdio
//!    with an own [`AppState`].
//!
//! Nothing but MCP messages may go to stdout; diagnostics go to stderr.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use arrangia_app::commands::AppState;
use arrangia_app::dialogs::NoDialogs;
use arrangia_app::settings::{AppPaths, SettingsStore};
use rmcp::ServiceExt;

use crate::discovery::{self, Discovery};
use crate::progress::ProgressRelay;
use crate::server::{ArrangiaMcp, Host};

/// Overrides the GUI binary that `arrangia mcp` starts (tests, dev).
pub const GUI_ENV: &str = "ARRANGIA_GUI";
/// How long `arrangia mcp` waits for a GUI it started.
pub const GUI_START_TIMEOUT: Duration = Duration::from_secs(20);

/// Options of `arrangia mcp`.
#[derive(Debug, Clone, Default)]
pub struct StdioOptions {
    /// Project to open at start (headless; passed to a GUI it starts).
    pub project: Option<PathBuf>,
    /// App data folder (settings, recent projects, installed frames,
    /// discovery file); default `$ARRANGIA_APP_DIR` or the platform folder.
    pub app_dir: Option<PathBuf>,
    /// Never connect to (or start) a GUI.
    pub headless: bool,
}

/// A running GUI serving MCP.
pub type GuiEndpoint = Discovery;

/// Finds a running GUI that serves MCP: the discovery file in
/// `paths.data_dir` when its endpoint answers (a stale file is removed).
pub fn discover_gui(paths: &AppPaths) -> Option<GuiEndpoint> {
    discovery::find(paths)
}

/// App folders for `app_dir` (default: `$ARRANGIA_APP_DIR` or the
/// platform folders).
pub fn app_paths(app_dir: Option<&Path>) -> AppPaths {
    match app_dir {
        Some(dir) => AppPaths::in_dir(dir),
        None => AppPaths::platform()
            .unwrap_or_else(|| AppPaths::in_dir(&std::env::temp_dir().join("arrangia"))),
    }
}

/// A headless server: own app state (file watcher on), `project` opened.
pub fn headless_server(paths: AppPaths, project: Option<&Path>) -> Result<ArrangiaMcp, String> {
    let relay = Arc::new(ProgressRelay::default());
    let app = Arc::new(AppState::new(paths, relay.clone(), Arc::new(NoDialogs)));
    if let Some(dir) = project {
        app.open_project(dir)
            .map_err(|e| format!("cannot open `{}`: {}", dir.display(), e.message))?;
    }
    Ok(ArrangiaMcp::new(app, Some(relay), Host::Headless))
}

/// The command that starts the GUI: [`GUI_ENV`]; a macOS app bundle
/// around the running CLI (`open -a <bundle>`); else `arrangia-gui` next
/// to the CLI binary (dev builds, Windows/Linux installs).
pub fn gui_command(project: Option<&Path>) -> Option<std::process::Command> {
    let with_project = |mut c: std::process::Command| {
        if let Some(p) = project {
            c.arg(p);
        }
        c
    };
    if let Some(p) = std::env::var_os(GUI_ENV).filter(|v| !v.is_empty()) {
        return Some(with_project(std::process::Command::new(p)));
    }
    let exe = std::env::current_exe().ok()?;
    #[cfg(target_os = "macos")]
    {
        // …/Arrangia.app/Contents/MacOS/arrangia
        let bundle = exe.ancestors().nth(3).filter(|b| {
            b.extension().is_some_and(|e| e == "app")
                && exe.parent().is_some_and(|d| d.ends_with("Contents/MacOS"))
        });
        if let Some(bundle) = bundle {
            let mut c = std::process::Command::new("open");
            c.arg("-a").arg(bundle);
            if let Some(p) = project {
                c.arg("--args").arg(p);
            }
            return Some(c);
        }
    }
    let gui = exe
        .parent()?
        .join(format!("arrangia-gui{}", std::env::consts::EXE_SUFFIX));
    gui.is_file()
        .then(|| with_project(std::process::Command::new(gui)))
}

/// Starts the GUI (detached, no stdio) and waits until its endpoint
/// answers. `Err` says why not (stderr log line for the fallback).
pub fn launch_gui(
    paths: &AppPaths,
    app_dir: Option<&Path>,
    project: Option<&Path>,
    timeout: Duration,
) -> Result<GuiEndpoint, String> {
    let mut cmd = gui_command(project).ok_or("no Arrangia GUI binary found next to the CLI")?;
    cmd.stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    if let Some(dir) = app_dir {
        cmd.env(arrangia_core::frame_import::APP_DIR_ENV, dir);
    }
    // Its own process group: the GUI outlives this proxy.
    #[cfg(unix)]
    std::os::unix::process::CommandExt::process_group(&mut cmd, 0);
    let mut child = cmd
        .spawn()
        .map_err(|e| format!("cannot start the GUI: {e}"))?;
    let start = Instant::now();
    while start.elapsed() < timeout {
        if let Some(gui) = discover_gui(paths) {
            return Ok(gui);
        }
        if let Ok(Some(status)) = child.try_wait()
            && !status.success()
        {
            return Err(format!("the GUI exited ({status})"));
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    Err(format!(
        "the GUI did not offer MCP within {} s (is \"Allow AI assistants to connect via MCP\" off?)",
        timeout.as_secs()
    ))
}

/// Runs `arrangia mcp` until the client closes stdin.
pub fn run(opts: &StdioOptions) -> Result<(), String> {
    let paths = app_paths(opts.app_dir.as_deref());
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|e| format!("cannot start the async runtime: {e}"))?;
    if !opts.headless
        && let Some(gui) = find_or_start_gui(&paths, opts)
    {
        return proxy(&runtime, paths, gui);
    }
    let server = headless_server(paths, opts.project.as_deref())?;
    runtime.block_on(async move {
        let service = server
            .serve(rmcp::transport::stdio())
            .await
            .map_err(|e| format!("MCP initialisation failed: {e}"))?;
        service
            .waiting()
            .await
            .map_err(|e| format!("MCP server stopped: {e}"))?;
        Ok(())
    })
}

/// A running GUI, or one started now (setting `mcp_autostart_gui`).
fn find_or_start_gui(paths: &AppPaths, opts: &StdioOptions) -> Option<GuiEndpoint> {
    if !cfg!(feature = "proxy") {
        return None;
    }
    if let Some(gui) = discover_gui(paths) {
        eprintln!(
            "arrangia mcp: connected to the running Arrangia app ({})",
            gui.url
        );
        return Some(gui);
    }
    let autostart = SettingsStore::new(paths.settings_file())
        .load()
        .map_or(true, |s| s.mcp_autostart_gui);
    if !autostart {
        eprintln!("arrangia mcp: no running Arrangia app; working headless");
        return None;
    }
    match launch_gui(
        paths,
        opts.app_dir.as_deref(),
        opts.project.as_deref(),
        GUI_START_TIMEOUT,
    ) {
        Ok(gui) => {
            eprintln!("arrangia mcp: started the Arrangia app ({})", gui.url);
            Some(gui)
        }
        Err(e) => {
            eprintln!("arrangia mcp: {e}; working headless");
            None
        }
    }
}

#[cfg(feature = "proxy")]
fn proxy(
    runtime: &tokio::runtime::Runtime,
    paths: AppPaths,
    gui: GuiEndpoint,
) -> Result<(), String> {
    runtime.block_on(crate::proxy::run(paths, gui))
}

#[cfg(not(feature = "proxy"))]
fn proxy(_: &tokio::runtime::Runtime, _: AppPaths, _: GuiEndpoint) -> Result<(), String> {
    Err("this build cannot proxy to the GUI; use --headless".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_dir_option_wins() {
        let tmp = tempfile::tempdir().unwrap();
        assert_eq!(app_paths(Some(tmp.path())), AppPaths::in_dir(tmp.path()));
        assert_eq!(discover_gui(&AppPaths::in_dir(tmp.path())), None);
    }

    #[test]
    fn headless_server_reports_unloadable_projects() {
        let tmp = tempfile::tempdir().unwrap();
        let err = headless_server(AppPaths::in_dir(tmp.path()), Some(&tmp.path().join("nope")))
            .unwrap_err();
        assert!(err.contains("cannot open"), "{err}");
        let ok = headless_server(AppPaths::in_dir(tmp.path()), None).unwrap();
        assert_eq!(ok.host(), Host::Headless);
        assert!(ok.tools().len() > 40);
    }
}
