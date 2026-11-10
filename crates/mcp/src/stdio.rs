//! `arrangia mcp`: the stdio entry point MCP clients start themselves.
//!
//! Planned behaviour (ADR 0014): first look for a running Arrangia GUI
//! ([`discover_gui`], discovery file in the app data folder) and forward
//! stdio to its in-app HTTP endpoint, so Claude edits the session the user
//! sees. That proxy comes with Phase 4.3b; until then (and always with
//! `--headless`) the server runs headless with its own [`AppState`].

use std::path::PathBuf;
use std::sync::Arc;

use arrangia_app::commands::AppState;
use arrangia_app::dialogs::NoDialogs;
use arrangia_app::settings::AppPaths;
use rmcp::ServiceExt;

use crate::progress::ProgressRelay;
use crate::server::{ArrangiaMcp, Host};

/// Options of `arrangia mcp`.
#[derive(Debug, Clone, Default)]
pub struct StdioOptions {
    /// Project to open at start (headless).
    pub project: Option<PathBuf>,
    /// App data folder (settings, recent projects, installed frames);
    /// default `$ARRANGIA_APP_DIR` or the platform folder.
    pub app_dir: Option<PathBuf>,
    /// Never connect to a running GUI.
    pub headless: bool,
}

/// A running GUI serving MCP (4.3b: read from the discovery file).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuiEndpoint {
    /// Streamable HTTP endpoint on 127.0.0.1.
    pub url: String,
    /// Bearer token.
    pub token: String,
}

/// Finds a running GUI that serves MCP. Phase 4.3b reads the discovery
/// file in `paths.data_dir`; until then there is none.
pub fn discover_gui(paths: &AppPaths) -> Option<GuiEndpoint> {
    let _ = paths;
    None
}

/// App folders for `app_dir` (default: `$ARRANGIA_APP_DIR` or the
/// platform folders).
pub fn app_paths(app_dir: Option<&std::path::Path>) -> AppPaths {
    match app_dir {
        Some(dir) => AppPaths::in_dir(dir),
        None => AppPaths::platform()
            .unwrap_or_else(|| AppPaths::in_dir(&std::env::temp_dir().join("arrangia"))),
    }
}

/// A headless server: own app state (file watcher on), `project` opened.
pub fn headless_server(
    paths: AppPaths,
    project: Option<&std::path::Path>,
) -> Result<ArrangiaMcp, String> {
    let relay = Arc::new(ProgressRelay::default());
    let app = Arc::new(AppState::new(paths, relay.clone(), Arc::new(NoDialogs)));
    if let Some(dir) = project {
        app.open_project(dir)
            .map_err(|e| format!("cannot open `{}`: {}", dir.display(), e.message))?;
    }
    Ok(ArrangiaMcp::new(app, Some(relay), Host::Headless))
}

/// Runs `arrangia mcp` until the client closes stdin. Nothing but MCP
/// messages may go to stdout; diagnostics go to stderr.
pub fn run(opts: &StdioOptions) -> Result<(), String> {
    let paths = app_paths(opts.app_dir.as_deref());
    if !opts.headless
        && let Some(gui) = discover_gui(&paths)
    {
        // Phase 4.3b: forward stdio to the GUI's endpoint.
        return Err(format!(
            "a running Arrangia GUI serves MCP at {} but the stdio proxy is not implemented yet; use --headless",
            gui.url
        ));
    }
    let server = headless_server(paths, opts.project.as_deref())?;
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|e| format!("cannot start the async runtime: {e}"))?;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_dir_option_wins() {
        let dir = std::path::Path::new("/tmp/x");
        assert_eq!(app_paths(Some(dir)), AppPaths::in_dir(dir));
        assert_eq!(discover_gui(&AppPaths::in_dir(dir)), None);
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
