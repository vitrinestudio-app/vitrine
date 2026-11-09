//! # arrangia-gui (library part)
//!
//! The transport-agnostic GUI backend (docs/plan/phase-2.md, ADR 0008):
//! plain Rust functions with serde arguments and results, used by the Tauri
//! app (`src/main.rs`, thin `#[tauri::command]` wrappers) and by the dev
//! HTTP bridge (`arrangia-devbridge`, feature `devbridge`).
//!
//! ```text
//! commands  AppState + one function per command, `dispatch(name, json)`
//! session   the open project: model, history, atomic save, live reload
//! preview   preview render cache and the binary image payload
//! settings  app settings (YAML) and recent projects (SQLite)
//! watch     debounced file watcher of the project folder
//! events    EventSink (Tauri `emit` / bridge SSE) and event names
//! dialogs   Dialogs trait (Tauri dialog plugin / bridge stub)
//! bridge    dev HTTP bridge (feature `devbridge`)
//! ```
//!
//! All model changes go through the core Operations API.

#[cfg(feature = "devbridge")]
pub mod bridge;
pub mod commands;
pub mod dialogs;
pub mod error;
pub mod events;
pub mod preview;
pub mod session;
pub mod settings;
pub mod watch;

/// Helpers for tests (also used by the integration tests).
#[doc(hidden)]
pub mod testutil {
    use std::path::{Path, PathBuf};

    /// `tests/fixtures/example` in the repository.
    pub fn example_fixture() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/example")
    }

    /// Copies `src` to `dst` recursively, skipping `export/`, `golden/` and
    /// `.cache/` (render output, not project input).
    pub fn copy_project(src: &Path, dst: &Path) -> std::io::Result<()> {
        std::fs::create_dir_all(dst)?;
        for entry in std::fs::read_dir(src)? {
            let entry = entry?;
            let name = entry.file_name();
            if ["export", "golden", ".cache"].iter().any(|n| name == *n) {
                continue;
            }
            let to = dst.join(&name);
            if entry.file_type()?.is_dir() {
                copy_project(&entry.path(), &to)?;
            } else {
                std::fs::copy(entry.path(), &to)?;
            }
        }
        Ok(())
    }

    /// A temp folder holding a copy of the example project (in `project/`).
    #[cfg(test)]
    pub fn example_copy() -> (tempfile::TempDir, PathBuf) {
        let tmp = tempfile::tempdir().expect("temp dir");
        let dir = tmp.path().join("project");
        copy_project(&example_fixture(), &dir).expect("copy example project");
        (tmp, dir)
    }
}

pub use commands::{AppState, Reply, dispatch};
pub use error::{CommandError, ErrorCode};
pub use events::EventSink;
pub use session::Session;

/// Basic information about the running application.
#[derive(Debug, Clone, serde::Serialize)]
pub struct AppInfo {
    pub name: &'static str,
    pub version: &'static str,
}

/// Name and version of the app.
pub fn app_info() -> AppInfo {
    AppInfo {
        name: "Arrangia",
        version: env!("CARGO_PKG_VERSION"),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn app_info_reports_package_version() {
        let info = super::app_info();
        assert_eq!(info.name, "Arrangia");
        assert_eq!(info.version, env!("CARGO_PKG_VERSION"));
    }
}
