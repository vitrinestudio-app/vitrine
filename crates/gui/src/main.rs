//! Arrangia desktop editor.
//!
//! The web view only shows pixmaps rendered in Rust and draws interaction
//! overlays on top. All file and system access goes through custom Tauri
//! commands defined here; the web view itself only gets `core:default`.

// Hide the console window on Windows release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::Serialize;

/// Basic information about the running application.
#[derive(Debug, Clone, Serialize)]
struct AppInfo {
    name: &'static str,
    version: &'static str,
}

fn app_info_value() -> AppInfo {
    AppInfo {
        name: "Arrangia",
        version: env!("CARGO_PKG_VERSION"),
    }
}

/// Returns name and version of the app to the frontend.
#[tauri::command]
fn app_info() -> AppInfo {
    app_info_value()
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![app_info])
        .run(tauri::generate_context!())
        .expect("error while running Arrangia");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_info_reports_package_version() {
        let info = app_info_value();
        assert_eq!(info.name, "Arrangia");
        assert_eq!(info.version, env!("CARGO_PKG_VERSION"));
    }
}
