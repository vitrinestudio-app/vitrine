//! Folder picker, "show in Finder/Explorer" and help links (`std::process`,
//! so the web view keeps `core:default`).

use std::path::PathBuf;

use serde::Deserialize;

use super::AppState;
use crate::error::{CmdResult, CommandError};

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DialogPickFolderArgs {
    #[serde(default)]
    pub title: Option<String>,
}

/// Asks for a folder (`null` when cancelled).
pub fn dialog_pick_folder(
    app: &AppState,
    args: DialogPickFolderArgs,
) -> CmdResult<Option<PathBuf>> {
    let title = args.title.unwrap_or_else(|| "Choose folder".to_owned());
    Ok(app.dialogs().pick_folder(&title))
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RevealPathArgs {
    /// Absolute, or relative to the open project.
    pub path: PathBuf,
}

/// Shows a file (selected) or folder in the system file manager.
pub fn reveal_path(app: &AppState, args: RevealPathArgs) -> CmdResult<()> {
    let path = if args.path.is_absolute() {
        args.path
    } else {
        app.session()?.dir().join(args.path)
    };
    if !path.exists() {
        return Err(CommandError::not_found(format!(
            "{} does not exist",
            path.display()
        )));
    }
    let mut cmd = reveal_command(&path);
    cmd.spawn()
        .map_err(|e| CommandError::io(format!("cannot open the file manager: {e}")))?;
    Ok(())
}

/// The only URLs `open_url` opens: setup help for store keys (App settings
/// → Stores). Exact matches, https only.
pub const HELP_URLS: &[&str] = &[
    "https://appstoreconnect.apple.com/access/integrations/api",
    "https://developer.apple.com/documentation/appstoreconnectapi/creating-api-keys-for-app-store-connect-api",
    "https://console.cloud.google.com/apis/library/androidpublisher.googleapis.com",
    "https://console.cloud.google.com/iam-admin/serviceaccounts",
    "https://play.google.com/console",
    "https://developers.google.com/android-publisher/getting_started",
    // Apple Design Resources: Product Bezels to import as device frames (Help menu).
    APPLE_BEZELS_URL,
    // Updates (ADR 0026): release notes / manual download (deb/rpm installs).
    RELEASES_URL,
    // User guide on the website (top of the Help menu, by UI language).
    DOCS_URL_EN,
    DOCS_URL_DE,
];

/// The user guide on the website (English; Help menu).
pub const DOCS_URL_EN: &str = "https://www.vitrinestudio.app/docs/";

/// The user guide on the website (German; Help menu).
pub const DOCS_URL_DE: &str = "https://www.vitrinestudio.app/de/docs/";

/// Apple Design Resources, Product Bezels section (Help menu).
pub const APPLE_BEZELS_URL: &str = "https://developer.apple.com/design/resources/#product-bezels";

/// The latest release on GitHub (update notice, manual download).
pub const RELEASES_URL: &str = "https://github.com/vitrinestudio-app/vitrine/releases/latest";

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OpenUrlArgs {
    pub url: String,
}

/// The App Store Connect page of an app's versions
/// (`https://appstoreconnect.apple.com/apps/<numeric app id>/distribution`).
pub fn app_store_connect_app_url(app_id: &str) -> Option<String> {
    (!app_id.is_empty() && app_id.bytes().all(|b| b.is_ascii_digit()))
        .then(|| format!("https://appstoreconnect.apple.com/apps/{app_id}/distribution"))
}

/// A link `open_url` may open: one of the [`HELP_URLS`] or an app's page
/// in App Store Connect ([`app_store_connect_app_url`]).
pub fn is_allowed_url(url: &str) -> bool {
    HELP_URLS.contains(&url)
        || url
            .strip_prefix("https://appstoreconnect.apple.com/apps/")
            .and_then(|rest| rest.strip_suffix("/distribution"))
            .and_then(app_store_connect_app_url)
            .is_some()
}

/// Opens an allowed link ([`is_allowed_url`]) in the system browser;
/// anything else is refused (`forbidden`).
pub fn open_url(app: &AppState, args: OpenUrlArgs) -> CmdResult<()> {
    if !is_allowed_url(&args.url) {
        return Err(CommandError::new(
            crate::error::ErrorCode::Forbidden,
            format!("`{}` is not an allowed help link", args.url),
        ));
    }
    app.dialogs().open_url(&args.url).map_err(|e| {
        CommandError::new(crate::error::ErrorCode::NotImplemented, e)
            .with_details(serde_json::json!({ "url": args.url }))
    })
}

#[cfg(target_os = "macos")]
fn reveal_command(path: &std::path::Path) -> std::process::Command {
    let mut c = std::process::Command::new("open");
    if path.is_dir() {
        c.arg(path);
    } else {
        c.arg("-R").arg(path);
    }
    c
}

#[cfg(target_os = "windows")]
fn reveal_command(path: &std::path::Path) -> std::process::Command {
    let mut c = std::process::Command::new("explorer");
    if path.is_dir() {
        c.arg(path);
    } else {
        let mut arg = std::ffi::OsString::from("/select,");
        arg.push(path);
        c.arg(arg);
    }
    c
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn reveal_command(path: &std::path::Path) -> std::process::Command {
    let mut c = std::process::Command::new("xdg-open");
    c.arg(if path.is_dir() {
        path
    } else {
        path.parent().unwrap_or(path)
    });
    c
}
