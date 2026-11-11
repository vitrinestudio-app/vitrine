//! In-app updates (ADR 0026) with `tauri-plugin-updater`, called from Rust
//! only (the web view keeps `core:default`).
//!
//! - The app asks GitHub Releases for `latest.json` (endpoint and public key
//!   in `tauri.conf.json` → `plugins.updater`); the downloaded installer is
//!   verified against that key (minisign) and the version signed into it
//!   (`requireSignedVersion`).
//! - Nothing is installed without the user: `update_check` only reports,
//!   `update_install` runs after the user clicked "Install and restart".
//! - Automatic check: once a day at start (setting `update_check`, default
//!   on; `update_checked_at` remembers the time); no telemetry beyond the
//!   one GET.
//! - Offered only where an update can replace this installation: macOS
//!   `.app` bundle, Windows NSIS/MSI install, Linux `AppImage`. deb/rpm
//!   installs only get the notice (update via the package / download
//!   page); dev builds, debug builds and builds whose public key is still
//!   the placeholder get nothing.

use std::sync::{Arc, Mutex, OnceLock, PoisonError};

use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tauri_plugin_updater::{Update, UpdaterExt};
use vitrine_gui::commands::AppState;
use vitrine_gui::{CommandError, ErrorCode};

/// Marker of the placeholder public key in `tauri.conf.json`.
pub const PUBKEY_PLACEHOLDER: &str = "REPLACE_WITH_TAURI_UPDATER_PUBLIC_KEY";
/// Seconds between automatic checks.
pub const CHECK_INTERVAL_S: i64 = 24 * 60 * 60;
/// Download progress event: `{downloaded, total}` (bytes; `total` may be null).
pub const PROGRESS_EVENT: &str = "update-progress";

/// What this build can do with an update.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    /// Download, verify, install, restart.
    Install,
    /// Only tell the user (deb/rpm: the package manager or the download
    /// page updates it).
    Notify,
    /// No updates (see [`Reason`]).
    Off,
}

/// Why updates are off (or limited).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Reason {
    Ok,
    /// Debug build or not running from an installed bundle.
    DevBuild,
    /// The public key in `tauri.conf.json` is still the placeholder.
    NotConfigured,
    /// Installed by deb/rpm.
    PackageManager,
}

/// How this copy of the app was installed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Install {
    MacApp,
    WindowsInstaller,
    AppImage,
    LinuxPackage,
    Unbundled,
}

/// Decision for `install` with `pubkey` (pure, tested).
pub fn availability(pubkey: &str, install: Install, debug: bool) -> (Mode, Reason) {
    if debug || install == Install::Unbundled {
        return (Mode::Off, Reason::DevBuild);
    }
    if pubkey.trim().is_empty() || pubkey.contains(PUBKEY_PLACEHOLDER) {
        return (Mode::Off, Reason::NotConfigured);
    }
    match install {
        Install::LinuxPackage => (Mode::Notify, Reason::PackageManager),
        _ => (Mode::Install, Reason::Ok),
    }
}

/// An automatic check is due (`last`, `now`: unix seconds).
pub fn check_due(last: Option<i64>, now: i64) -> bool {
    last.is_none_or(|t| now - t >= CHECK_INTERVAL_S || t > now)
}

/// This process' installation kind.
fn current_install() -> Install {
    use tauri::utils::config::BundleType;
    use tauri::utils::platform::bundle_type;
    if cfg!(target_os = "macos") {
        // `bundle_type()` says `App` for every macOS build; check the path.
        let in_bundle = std::env::current_exe().is_ok_and(|exe| {
            exe.parent().is_some_and(|d| d.ends_with("Contents/MacOS"))
                && exe
                    .ancestors()
                    .nth(3)
                    .is_some_and(|b| b.extension().is_some_and(|e| e == "app"))
        });
        return if in_bundle {
            Install::MacApp
        } else {
            Install::Unbundled
        };
    }
    match bundle_type() {
        Some(BundleType::Nsis | BundleType::Msi) => Install::WindowsInstaller,
        Some(BundleType::AppImage) => Install::AppImage,
        Some(BundleType::Deb | BundleType::Rpm) => Install::LinuxPackage,
        _ => Install::Unbundled,
    }
}

fn configured_pubkey(app: &AppHandle) -> String {
    app.config()
        .plugins
        .0
        .get("updater")
        .and_then(|u| u.get("pubkey"))
        .and_then(|k| k.as_str())
        .unwrap_or_default()
        .to_owned()
}

fn now_s() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
}

/// The update found by the last check, kept for `update_install`.
#[derive(Default)]
pub struct UpdaterState {
    pending: Mutex<Option<Update>>,
    /// Runs before the app exits for the installer / restart (stops the
    /// MCP endpoint and removes its discovery file).
    before_exit: OnceLock<Arc<dyn Fn() + Send + Sync>>,
}

impl UpdaterState {
    pub fn set_before_exit(&self, f: Arc<dyn Fn() + Send + Sync>) {
        let _ = self.before_exit.set(f);
    }
}

#[derive(Debug, Serialize)]
pub struct StatusReply {
    pub current_version: &'static str,
    pub mode: Mode,
    pub reason: Reason,
    /// Setting `update_check`.
    pub auto_check: bool,
    /// Unix seconds of the last check.
    pub checked_at: Option<i64>,
    pub releases_url: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct UpdateInfo {
    pub version: String,
    pub current_version: String,
    /// Release notes (Markdown/plain text from `latest.json`).
    pub notes: Option<String>,
    /// RFC 3339.
    pub date: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CheckReply {
    /// `false`: an automatic check that was not due / switched off / not
    /// possible in this build.
    pub checked: bool,
    pub mode: Mode,
    pub reason: Reason,
    pub update: Option<UpdateInfo>,
}

fn status_of(app: &AppHandle, state: &AppState) -> Result<StatusReply, CommandError> {
    let settings = state.settings_store().load()?;
    let (mode, reason) = availability(
        &configured_pubkey(app),
        current_install(),
        cfg!(debug_assertions),
    );
    Ok(StatusReply {
        current_version: env!("CARGO_PKG_VERSION"),
        mode,
        reason,
        auto_check: settings.update_check,
        checked_at: settings.update_checked_at,
        releases_url: vitrine_gui::commands::RELEASES_URL,
    })
}

/// `update_status`: what this build can do, the setting, the last check.
pub fn status(app: &AppHandle, state: &AppState) -> Result<StatusReply, CommandError> {
    status_of(app, state)
}

/// `update_check {auto}`: asks the endpoint. `auto: true` (app start)
/// only checks when the setting is on and the last check is a day old.
pub async fn check(
    app: &AppHandle,
    state: &AppState,
    updater: &UpdaterState,
    auto: bool,
) -> Result<CheckReply, CommandError> {
    let st = status_of(app, state)?;
    let skip =
        st.mode == Mode::Off || (auto && (!st.auto_check || !check_due(st.checked_at, now_s())));
    if skip {
        return Ok(CheckReply {
            checked: false,
            mode: st.mode,
            reason: st.reason,
            update: None,
        });
    }
    let mut builder = app.updater_builder();
    if let Some(f) = updater.before_exit.get().cloned() {
        // Windows: the plugin exits the app right after starting the
        // installer; stop the MCP endpoint first.
        builder = builder.on_before_exit(move || f());
    }
    let found = builder
        .build()
        .map_err(|e| CommandError::internal(format!("updater: {e}")))?
        .check()
        .await
        .map_err(|e| {
            CommandError::new(
                ErrorCode::Unreachable,
                format!("cannot check for updates: {e}"),
            )
        })?;
    // The time of the last successful check (also manual ones).
    let _ = state
        .settings_store()
        .update(&serde_json::json!({ "update_checked_at": now_s() }));
    let info = found.as_ref().map(|u| UpdateInfo {
        version: u.version.clone(),
        current_version: u.current_version.clone(),
        notes: u.body.clone(),
        date: u.raw_json["pub_date"].as_str().map(str::to_owned),
    });
    *updater
        .pending
        .lock()
        .unwrap_or_else(PoisonError::into_inner) = found;
    Ok(CheckReply {
        checked: true,
        mode: st.mode,
        reason: st.reason,
        update: info,
    })
}

/// `update_install`: downloads, verifies and installs the update found by
/// the last check, then restarts the app (Windows: the installer
/// restarts it). Only after the user confirmed in the UI.
pub async fn install(
    app: &AppHandle,
    state: &AppState,
    updater: &UpdaterState,
) -> Result<(), CommandError> {
    let st = status_of(app, state)?;
    if st.mode != Mode::Install {
        return Err(CommandError::invalid(
            "this installation cannot update itself (see the release page)",
        ));
    }
    let update = updater
        .pending
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone()
        .ok_or_else(|| CommandError::invalid("no update found: check for updates first"))?;
    let mut downloaded: u64 = 0;
    let progress = app.clone();
    update
        .download_and_install(
            move |chunk, total| {
                downloaded += chunk as u64;
                let _ = progress.emit(
                    PROGRESS_EVENT,
                    serde_json::json!({ "downloaded": downloaded, "total": total }),
                );
            },
            || {},
        )
        .await
        .map_err(|e| CommandError::internal(format!("cannot install the update: {e}")))?;
    // macOS/Linux: the new version is in place; start it.
    if let Some(f) = updater.before_exit.get() {
        f();
    }
    app.restart();
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk=";

    #[test]
    fn placeholder_and_dev_builds_offer_nothing() {
        let placeholder = format!("{PUBKEY_PLACEHOLDER} (see docs)");
        assert_eq!(
            availability(&placeholder, Install::MacApp, false),
            (Mode::Off, Reason::NotConfigured)
        );
        assert_eq!(
            availability("", Install::WindowsInstaller, false),
            (Mode::Off, Reason::NotConfigured)
        );
        assert_eq!(
            availability(KEY, Install::MacApp, true),
            (Mode::Off, Reason::DevBuild)
        );
        assert_eq!(
            availability(KEY, Install::Unbundled, false),
            (Mode::Off, Reason::DevBuild)
        );
    }

    #[test]
    fn installers_update_packages_only_notify() {
        for i in [
            Install::MacApp,
            Install::WindowsInstaller,
            Install::AppImage,
        ] {
            assert_eq!(availability(KEY, i, false), (Mode::Install, Reason::Ok));
        }
        assert_eq!(
            availability(KEY, Install::LinuxPackage, false),
            (Mode::Notify, Reason::PackageManager)
        );
    }

    #[test]
    fn automatic_check_once_a_day() {
        let now = 1_762_790_405;
        assert!(check_due(None, now));
        assert!(!check_due(Some(now - 3600), now));
        assert!(check_due(Some(now - CHECK_INTERVAL_S), now));
        // A clock that went backwards does not block checks for ever.
        assert!(check_due(Some(now + 3600), now));
    }

    #[test]
    fn the_shipped_config_still_has_the_placeholder_or_a_real_key() {
        let conf: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        let updater = &conf["plugins"]["updater"];
        let key = updater["pubkey"].as_str().unwrap();
        assert!(
            key.contains(PUBKEY_PLACEHOLDER) || !key.contains(' '),
            "{key}"
        );
        assert_eq!(
            updater["endpoints"][0],
            "https://github.com/vitrinestudio-app/vitrine/releases/latest/download/latest.json"
        );
        assert_eq!(updater["requireSignedVersion"], true);
        // Updater artifacts only in the release overlay (needs the private key).
        assert!(conf["bundle"].get("createUpdaterArtifacts").is_none());
    }
}
