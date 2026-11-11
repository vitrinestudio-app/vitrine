//! All GUI commands as plain Rust functions: `fn(&AppState, Args) ->
//! Result<T, CommandError>` with serde `Args`/`T`. The Tauri app wraps
//! each in a `#[tauri::command]`; the dev bridge calls [`dispatch`].
//!
//! JSON commands return JSON; image commands ([`IMAGE_COMMANDS`]) return
//! a [`PreviewImage`] that each transport encodes itself (Tauri: header +
//! data as `ArrayBuffer`; bridge: bare PNG/RGBA with `X-*` headers).
//!
//! `overlay` (hit-test data per target × language × variant) and the
//! `edit_*` gesture intents (`core::edit`, answering with the
//! [`crate::session::ApplyResult`] of the step plus created ids) take the
//! view context `{target, lang, variant}` (see [`ViewContext`]);
//! `render_preview_edit` renders a gesture in progress without saving it.

// Uniform signature `fn(&AppState, Args)` for the dispatch table and the
// Tauri wrappers, whether or not a command consumes its arguments.
#![allow(clippy::needless_pass_by_value)]

mod assets;
mod catalog;
#[macro_use]
mod edit;
mod add;
mod canvases;
mod exchange;
mod export;
mod frames;
mod inspect;
mod mcp;
pub(crate) mod os;
mod project;
mod project_settings;
mod render;
mod screenshot_import;
mod stores;
mod strings;
mod system;
mod targets;
mod templates;
mod texts;
mod upload;
mod view;

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex, OnceLock, PoisonError, RwLock};

use serde::Deserialize;
use serde_json::Value;

pub use add::*;
pub use assets::*;
pub use canvases::*;
pub use catalog::*;
pub use edit::*;
pub use exchange::*;
pub use export::*;
pub use frames::*;
pub use inspect::*;
pub use mcp::*;
pub use os::*;
pub use project::*;
pub use project_settings::*;
pub use render::*;
pub use screenshot_import::*;
pub use stores::*;
pub use strings::*;
pub use system::*;
pub use targets::*;
pub use templates::*;
pub use texts::*;
pub use upload::*;
pub use view::*;

use crate::dialogs::Dialogs;
use crate::error::{CmdResult, CommandError, ErrorCode};
use crate::events::EventSink;
use crate::mcp::{McpControl, McpStatus};
use crate::preview::PreviewImage;
use crate::session::Session;
use crate::settings::{AppPaths, RecentStore, SettingsStore};
use crate::stores::ConnectionTester;
use vitrine_publish::credentials::SecretStore;

/// Arguments of commands that take none (`{}` or `null`).
#[derive(Debug, Clone, Copy, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NoArgs {}

/// Backend state shared by all commands (one per app; `Send + Sync`).
pub struct AppState {
    session: RwLock<Option<Arc<Session>>>,
    events: Arc<dyn EventSink>,
    dialogs: Arc<dyn Dialogs>,
    paths: AppPaths,
    settings: SettingsStore,
    recent: RecentStore,
    export_cancel: Mutex<Option<Arc<AtomicBool>>>,
    watch: bool,
    /// The GUI-hosted MCP endpoint (ADR 0015).
    mcp: Mutex<McpStatus>,
    mcp_token: Mutex<Option<String>>,
    mcp_control: OnceLock<Arc<dyn McpControl>>,
    /// Claude Desktop config file override (dev bridge, tests).
    claude_config: Option<PathBuf>,
    /// Keychain for store secrets (created on first use, ADR 0016).
    secrets: OnceLock<Arc<SecretStore>>,
    /// Store clients' connection test (none until Phase 5.3/5.4 wire it).
    connection_tester: OnceLock<Arc<dyn ConnectionTester>>,
    /// Store upload: config, running run, MCP confirmations (ADR 0018).
    upload: crate::upload::UploadState,
    /// Apple apps per account id for the app picker (this app session).
    store_apps: Mutex<std::collections::HashMap<String, Vec<vitrine_publish::apple::AppSummary>>>,
}

impl std::fmt::Debug for AppState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppState")
            .field("paths", &self.paths)
            .finish_non_exhaustive()
    }
}

impl AppState {
    /// State with app data in `paths`; projects are watched for external
    /// changes.
    pub fn new(paths: AppPaths, events: Arc<dyn EventSink>, dialogs: Arc<dyn Dialogs>) -> Self {
        Self {
            session: RwLock::new(None),
            events,
            dialogs,
            settings: SettingsStore::new(paths.settings_file()),
            recent: RecentStore::new(paths.database()),
            paths,
            export_cancel: Mutex::new(None),
            watch: true,
            mcp: Mutex::new(McpStatus::default()),
            mcp_token: Mutex::new(None),
            mcp_control: OnceLock::new(),
            claude_config: None,
            secrets: OnceLock::new(),
            connection_tester: OnceLock::new(),
            upload: crate::upload::UploadState::default(),
            store_apps: Mutex::new(std::collections::HashMap::new()),
        }
    }

    /// Uses `store` for store secrets instead of the environment's choice
    /// (tests: `SecretStore::mock()`).
    #[must_use]
    pub fn with_secret_store(self, store: SecretStore) -> Self {
        let _ = self.secrets.set(Arc::new(store));
        self
    }

    /// The keychain for store secrets: set by [`Self::with_secret_store`],
    /// else `SecretStore::from_env()` (system keychain, or the in-memory
    /// store with `VITRINE_KEYRING=mock`). Unit tests always get a mock.
    pub fn secret_store(&self) -> CmdResult<Arc<SecretStore>> {
        if let Some(s) = self.secrets.get() {
            return Ok(s.clone());
        }
        #[cfg(test)]
        let store = SecretStore::mock();
        #[cfg(not(test))]
        let store = SecretStore::from_env().map_err(keychain_error)?;
        Ok(self.secrets.get_or_init(|| Arc::new(store)).clone())
    }

    /// Registers the store clients' connection test (once).
    pub fn set_connection_tester(&self, tester: Arc<dyn ConnectionTester>) {
        let _ = self.connection_tester.set(tester);
    }

    pub fn connection_tester(&self) -> Option<&Arc<dyn ConnectionTester>> {
        self.connection_tester.get()
    }

    /// Store upload state (config, run, MCP confirmations).
    pub fn upload(&self) -> &crate::upload::UploadState {
        &self.upload
    }

    /// Cached app list of an Apple account (app picker).
    pub(crate) fn cached_store_apps(
        &self,
        account: &str,
    ) -> Option<Vec<vitrine_publish::apple::AppSummary>> {
        self.store_apps
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(account)
            .cloned()
    }

    /// Stores (`Some`) or forgets (`None`) the app list of an account.
    pub(crate) fn cache_store_apps(
        &self,
        account: &str,
        apps: Option<Vec<vitrine_publish::apple::AppSummary>>,
    ) {
        let mut cache = self
            .store_apps
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        match apps {
            Some(a) => {
                cache.insert(account.to_owned(), a);
            }
            None => {
                cache.remove(account);
            }
        }
    }

    /// Uses `path` as Claude Desktop's config file instead of the real one
    /// (dev bridge, tests).
    #[must_use]
    pub fn with_claude_desktop_config(mut self, path: PathBuf) -> Self {
        self.claude_config = Some(path);
        self
    }

    /// Claude Desktop's config file (override, `VITRINE_CLAUDE_DESKTOP_CONFIG`
    /// or the platform path).
    pub fn claude_desktop_config(&self) -> CmdResult<PathBuf> {
        self.claude_config
            .clone()
            .or_else(crate::claude_desktop::default_config_path)
            .ok_or_else(|| CommandError::io("no home folder for Claude Desktop's config"))
    }

    /// Status of the MCP endpoint (`connected` recomputed for now).
    pub fn mcp_status(&self) -> McpStatus {
        let mut s = self
            .mcp
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        s.refresh(crate::mcp::now_ms());
        s
    }

    /// Changes the MCP status and emits `mcp-status` with the result.
    pub fn update_mcp_status(&self, f: impl FnOnce(&mut McpStatus)) {
        {
            let mut s = self.mcp.lock().unwrap_or_else(PoisonError::into_inner);
            f(&mut s);
        }
        let status = self.mcp_status();
        self.events.emit(
            crate::events::MCP_STATUS,
            serde_json::to_value(status).unwrap_or_default(),
        );
    }

    /// The bearer token of the running endpoint (only `mcp_status` hands
    /// it out, never events).
    pub fn set_mcp_token(&self, token: Option<String>) {
        *self
            .mcp_token
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = token;
    }

    pub fn mcp_token(&self) -> Option<String> {
        self.mcp_token
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// Registers the host that starts/stops the endpoint (once).
    pub fn set_mcp_control(&self, control: Arc<dyn McpControl>) {
        let _ = self.mcp_control.set(control);
    }

    pub fn mcp_control(&self) -> Option<&Arc<dyn McpControl>> {
        self.mcp_control.get()
    }

    /// Disables the file watcher (tests that drive changes by hand).
    #[must_use]
    pub fn without_watcher(mut self) -> Self {
        self.watch = false;
        self
    }

    /// The open project or [`ErrorCode::NoProject`].
    pub fn session(&self) -> CmdResult<Arc<Session>> {
        self.session
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
            .ok_or_else(CommandError::no_project)
    }

    fn set_session(&self, session: Option<Arc<Session>>) {
        let old = std::mem::replace(
            &mut *self.session.write().unwrap_or_else(PoisonError::into_inner),
            session,
        );
        if let Some(old) = old {
            old.stop_watching();
        }
    }

    pub fn events(&self) -> &Arc<dyn EventSink> {
        &self.events
    }

    pub fn dialogs(&self) -> &Arc<dyn Dialogs> {
        &self.dialogs
    }

    pub fn paths(&self) -> &AppPaths {
        &self.paths
    }

    pub fn settings_store(&self) -> &SettingsStore {
        &self.settings
    }

    pub fn recent_store(&self) -> &RecentStore {
        &self.recent
    }

    /// Opens `dir` as the current project (closing the previous one).
    pub fn open_project(&self, dir: &std::path::Path) -> CmdResult<Arc<Session>> {
        let session = Session::open(dir, self.events.clone())?;
        if self.settings.load().is_ok_and(|s| s.system_font_fallback) {
            session.set_system_font_fallback(true);
        }
        session.set_frames(self.frame_index(), false);
        if self.watch {
            session.watch()?;
        }
        self.set_session(Some(session.clone()));
        let name = session.with_project(|p| p.name.clone());
        // The recent list is a convenience; a broken database must not
        // prevent opening projects.
        let _ = self.recent.touch(session.dir(), &name);
        Ok(session)
    }

    pub fn close_project(&self) {
        self.set_session(None);
    }
}

/// Result of [`dispatch`].
#[derive(Debug, Clone)]
pub enum Reply {
    Json(Value),
    Image(Arc<PreviewImage>),
}

macro_rules! command_table {
    (json: [$($j:ident),* $(,)?], image: [$($i:ident),* $(,)?] $(,)?) => {
        /// Names of the commands answering with JSON.
        pub const JSON_COMMANDS: &[&str] = &[$(stringify!($j)),*];
        /// Names of the commands answering with a preview image.
        pub const IMAGE_COMMANDS: &[&str] = &[$(stringify!($i)),*];

        /// Runs command `name` with JSON `args` (`null` = no arguments).
        pub fn dispatch(app: &AppState, name: &str, args: Value) -> CmdResult<Reply> {
            let args = if args.is_null() { Value::Object(Default::default()) } else { args };
            match name {
                $(stringify!($j) => {
                    let out = $j(app, serde_json::from_value(args)?)?;
                    serde_json::to_value(out)
                        .map(Reply::Json)
                        .map_err(|e| CommandError::internal(e.to_string()))
                })*
                $(stringify!($i) => Ok(Reply::Image($i(app, serde_json::from_value(args)?)?)),)*
                _ => Err(CommandError::new(
                    ErrorCode::UnknownCommand,
                    format!("unknown command `{name}`"),
                )),
            }
        }
    };
}

command_table! {
    json: [
        app_info,
        project_new, project_open, project_close, project_summary, project_get,
        recent_projects, recent_remove,
        apply_ops, undo, redo, history_state,
        overlay,
        edit_move, edit_move_by, edit_resize, edit_rotate, edit_set_page,
        edit_reorder, edit_duplicate, edit_remove, edit_group, edit_ungroup,
        edit_reset_override, element_reset_override,
        element_get, element_patch, edit_rename, edit_arrange, element_add,
        canvas_get, canvas_patch, canvas_variant, canvas_add, canvas_rename, canvas_remove,
        validate, export, export_cancel,
        strings_get, strings_missing,
        assets_list, assets_import, assets_remove,
        catalog_devices, catalog_targets, catalog_galleries, catalog_fonts,
        settings_get, settings_set,
        dialog_open_project, dialog_pick_files,
        preview_stats,
        element_info, markup_check, assets_probe, assets_upload,
        target_get, target_add_from_preset, target_patch, target_remove,
        project_rename, language_add, language_remove, languages_set,
        default_language_impact, default_language_set, styles_get, styles_patch,
        dialog_pick_folder, reveal_path,
        dialog_save_file, strings_export, strings_import_plan, strings_import_apply,
        import_plan, import_apply,
        frames_import_plan, frames_import_apply, frames_list, frames_remove,
        view_get, view_set,
        mcp_status, mcp_set_enabled, claude_desktop_plan, claude_desktop_install,
        store_accounts_list, store_account_import, store_account_remove, store_account_test,
        store_apps_list, store_package_check, open_url,
        project_store_get, project_store_patch, store_config_get,
        upload_readiness, upload_plan, upload_execute, upload_status, upload_cancel, upload_log, upload_confirm,
        upload_refdata,
        template_list, template_save, template_import, template_export, template_remove,
        template_insert_plan, template_insert,
    ],
    image: [
        render_image, render_canvas, render_gallery, render_preview_edit,
        render_preview_string, asset_thumbnail, frames_thumbnail, template_preview,
    ],
}

/// [`dispatch`] on behalf of `actor` (history labels and `project-edited`
/// events name it; see [`crate::actor`]).
pub fn dispatch_as(
    app: &AppState,
    actor: crate::Actor,
    name: &str,
    args: Value,
) -> CmdResult<Reply> {
    actor.scope(|| dispatch(app, name, args))
}

/// Default language of the open project when `lang` is `None`.
fn lang_or_default(session: &Session, lang: Option<String>) -> String {
    lang.unwrap_or_else(|| session.with_project(|p| p.languages.default.to_string()))
}

/// `dir` as given, or relative to the current folder.
fn absolute(dir: &std::path::Path) -> CmdResult<PathBuf> {
    std::path::absolute(dir).map_err(|e| CommandError::io(format!("{}: {e}", dir.display())))
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod tests_texts;

#[cfg(test)]
mod tests_exchange;

#[cfg(test)]
mod tests_import;

#[cfg(test)]
mod tests_frames;

#[cfg(test)]
mod tests_stores;

#[cfg(test)]
mod tests_templates;
