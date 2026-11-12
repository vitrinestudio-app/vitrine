//! Vitrine Studio desktop editor.
//!
//! The web view only shows pixmaps rendered in Rust and draws interaction
//! overlays on top. All file and system access goes through the custom
//! commands below; the web view itself only gets `core:default`. Every
//! command is a thin wrapper around `vitrine_gui::commands` (the same
//! functions the dev HTTP bridge serves), run on a blocking worker thread.
//!
//! Usage: `vitrine-gui [PROJECT_DIR]` opens that project at start (else
//! the start screen).
//!
//! The app hosts the MCP endpoint for AI assistants (ADR 0015,
//! `vitrine_mcp::host`): on `127.0.0.1`, against this app's live state,
//! with the discovery file `<app data>/mcp.json` for `vitrine mcp`
//! (removed again on exit). The setting "Allow AI assistants to connect
//! via MCP" switches it.
//!
//! JS calls them as `invoke(name, { args: {...} })` (see `ui/js/transport.js`);
//! image commands answer with an `ArrayBuffer` (header + PNG/RGBA, see
//! `vitrine_gui::preview::PreviewImage::to_tauri_bytes`).

// Hide the console window on Windows release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod native_menu;
mod updater;

use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

use tauri::{AppHandle, Emitter, RunEvent};
use tauri_plugin_dialog::DialogExt;
use vitrine_gui::commands::{self, AppState};
use vitrine_gui::dialogs::{Dialogs, FileFilter};
use vitrine_gui::settings::AppPaths;
use vitrine_gui::{CommandError, EventSink};
use vitrine_mcp::ProgressRelay;
use vitrine_mcp::host::{HostOptions, McpHost};

type Shared = Arc<AppState>;

/// Forwards backend events to the web view (`app.emit`).
#[derive(Default)]
struct TauriEvents(OnceLock<AppHandle>);

impl EventSink for TauriEvents {
    fn emit(&self, event: &str, payload: serde_json::Value) {
        if let Some(app) = self.0.get() {
            let _ = app.emit(event, payload);
        }
    }
}

/// Native dialogs via `tauri-plugin-dialog`, used from Rust only.
#[derive(Default)]
struct TauriDialogs(OnceLock<AppHandle>);

impl Dialogs for TauriDialogs {
    fn pick_folder(&self, title: &str) -> Option<PathBuf> {
        let app = self.0.get()?;
        app.dialog()
            .file()
            .set_title(title)
            .blocking_pick_folder()
            .and_then(|p| p.into_path().ok())
    }

    fn pick_files(&self, title: &str, filters: &[FileFilter], multiple: bool) -> Vec<PathBuf> {
        let Some(app) = self.0.get() else {
            return Vec::new();
        };
        let mut dialog = app.dialog().file().set_title(title);
        for f in filters {
            let ext: Vec<&str> = f.extensions.iter().map(String::as_str).collect();
            dialog = dialog.add_filter(&f.name, &ext);
        }
        let picked = if multiple {
            dialog.blocking_pick_files().unwrap_or_default()
        } else {
            dialog.blocking_pick_file().into_iter().collect()
        };
        picked
            .into_iter()
            .filter_map(|p| p.into_path().ok())
            .collect()
    }

    fn save_file(
        &self,
        title: &str,
        default_name: &str,
        filters: &[FileFilter],
    ) -> Option<PathBuf> {
        let app = self.0.get()?;
        let mut dialog = app
            .dialog()
            .file()
            .set_title(title)
            .set_file_name(default_name);
        for f in filters {
            let ext: Vec<&str> = f.extensions.iter().map(String::as_str).collect();
            dialog = dialog.add_filter(&f.name, &ext);
        }
        dialog.blocking_save_file().and_then(|p| p.into_path().ok())
    }

    fn open_url(&self, url: &str) -> Result<(), String> {
        vitrine_gui::dialogs::open_in_browser(url)
    }
}

/// Runs a command on a blocking worker thread.
async fn run<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, CommandError> + Send + 'static,
) -> Result<T, CommandError> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| CommandError::internal(format!("command panicked: {e}")))?
}

/// `#[tauri::command]` wrappers answering with JSON.
macro_rules! json_commands {
    ($($name:ident: $args:ty),* $(,)?) => {$(
        #[tauri::command]
        async fn $name(
            state: tauri::State<'_, Shared>,
            args: $args,
        ) -> Result<serde_json::Value, CommandError> {
            let app = state.inner().clone();
            run(move || {
                let out = commands::$name(&app, args)?;
                serde_json::to_value(out).map_err(|e| CommandError::internal(e.to_string()))
            })
            .await
        }
    )*};
}

/// `#[tauri::command]` wrappers answering with a binary preview image.
macro_rules! image_commands {
    ($($name:ident: $args:ty),* $(,)?) => {$(
        #[tauri::command]
        async fn $name(
            state: tauri::State<'_, Shared>,
            args: $args,
        ) -> Result<tauri::ipc::Response, CommandError> {
            let app = state.inner().clone();
            run(move || {
                let image = commands::$name(&app, args)?;
                Ok(tauri::ipc::Response::new(image.to_tauri_bytes()))
            })
            .await
        }
    )*};
}

json_commands! {
    app_info: commands::NoArgs,
    project_new: commands::ProjectNewArgs,
    project_open: commands::ProjectOpenArgs,
    project_close: commands::NoArgs,
    project_summary: commands::NoArgs,
    project_get: commands::NoArgs,
    recent_projects: commands::NoArgs,
    recent_remove: commands::RecentRemoveArgs,
    apply_ops: commands::ApplyOpsArgs,
    undo: commands::NoArgs,
    redo: commands::NoArgs,
    history_state: commands::NoArgs,
    overlay: commands::OverlayArgs,
    edit_move: commands::EditMoveArgs,
    edit_move_by: commands::EditMoveByArgs,
    edit_resize: commands::EditResizeArgs,
    edit_rotate: commands::EditRotateArgs,
    edit_set_page: commands::EditSetPageArgs,
    edit_reorder: commands::EditReorderArgs,
    edit_duplicate: commands::EditDuplicateArgs,
    edit_remove: commands::EditIdsArgs,
    edit_group: commands::EditGroupArgs,
    edit_ungroup: commands::EditIdsArgs,
    edit_copy: commands::EditIdsArgs,
    edit_cut: commands::EditIdsArgs,
    edit_paste: commands::EditPasteArgs,
    edit_clipboard: commands::NoArgs,
    edit_reset_override: commands::EditResetOverrideArgs,
    element_reset_override: commands::ElementResetOverrideArgs,
    element_get: commands::ElementGetArgs,
    element_patch: commands::ElementPatchArgs,
    edit_rename: commands::EditRenameArgs,
    edit_arrange: commands::EditArrangeArgs,
    element_add: commands::ElementAddArgs,
    canvas_get: commands::CanvasArgs,
    canvas_patch: commands::CanvasPatchArgs,
    canvas_variant: commands::CanvasVariantArgs,
    canvas_add: commands::CanvasAddArgs,
    canvas_rename: commands::CanvasRenameArgs,
    canvas_remove: commands::CanvasArgs,
    validate: commands::ValidateArgs,
    export: commands::ExportArgs,
    export_cancel: commands::NoArgs,
    strings_get: commands::StringsGetArgs,
    strings_missing: commands::NoArgs,
    assets_list: commands::NoArgs,
    assets_import: commands::AssetsImportArgs,
    assets_remove: commands::AssetsRemoveArgs,
    catalog_devices: commands::NoArgs,
    catalog_targets: commands::NoArgs,
    catalog_galleries: commands::NoArgs,
    catalog_fonts: commands::NoArgs,
    settings_get: commands::NoArgs,
    settings_set: commands::SettingsSetArgs,
    dialog_open_project: commands::DialogOpenProjectArgs,
    dialog_pick_files: commands::DialogPickFilesArgs,
    preview_stats: commands::NoArgs,
    element_info: commands::ElementInfoArgs,
    markup_check: commands::MarkupCheckArgs,
    assets_probe: commands::AssetsProbeArgs,
    asset_info: commands::AssetInfoArgs,
    assets_upload: commands::AssetsUploadArgs,
    target_get: commands::TargetIdArgs,
    target_add_from_preset: commands::TargetAddArgs,
    target_patch: commands::TargetPatchArgs,
    target_remove: commands::TargetIdArgs,
    canvas_banner_options: commands::BannerOptionsArgs,
    target_add_banner: commands::BannerFromCanvasArgs,
    project_rename: commands::ProjectRenameArgs,
    language_add: commands::LanguageArgs,
    language_remove: commands::LanguageArgs,
    languages_set: commands::LanguagesSetArgs,
    default_language_impact: commands::LanguageArgs,
    default_language_set: commands::LanguageArgs,
    styles_get: commands::NoArgs,
    styles_patch: commands::StylesPatchArgs,
    dialog_pick_folder: commands::DialogPickFolderArgs,
    reveal_path: commands::RevealPathArgs,
    dialog_save_file: commands::DialogSaveFileArgs,
    strings_export: commands::StringsExportArgs,
    strings_import_plan: commands::StringsImportPlanArgs,
    strings_import_apply: commands::StringsImportApplyArgs,
    import_plan: commands::ImportPlanArgs,
    import_apply: commands::ImportApplyArgs,
    frames_import_plan: commands::FramesImportPlanArgs,
    frames_import_apply: commands::FramesImportApplyArgs,
    frames_list: commands::NoArgs,
    frames_remove: commands::FramesRemoveArgs,
    frames_pack_import: commands::FramesPackImportArgs,
    frames_pack_remove: commands::FramesPackRemoveArgs,
    frames_embed_status: commands::FramesEmbedStatusArgs,
    frames_embed: commands::FramesEmbedArgs,
    frames_unembed: commands::NoArgs,
    view_get: commands::NoArgs,
    view_set: vitrine_gui::ViewState,
    mcp_status: commands::NoArgs,
    mcp_set_enabled: commands::McpSetEnabledArgs,
    claude_desktop_plan: commands::NoArgs,
    claude_desktop_install: commands::ClaudeDesktopInstallArgs,
    cli_install_status: commands::NoArgs,
    cli_install: commands::CliInstallArgs,
    store_accounts_list: commands::NoArgs,
    store_account_import: commands::StoreAccountImportArgs,
    store_account_remove: commands::StoreAccountIdArgs,
    store_account_test: commands::StoreAccountIdArgs,
    store_apps_list: commands::StoreAppsListArgs,
    store_package_check: commands::StorePackageCheckArgs,
    open_url: commands::OpenUrlArgs,
    project_store_get: commands::NoArgs,
    project_store_patch: commands::ProjectStorePatchArgs,
    store_config_get: commands::NoArgs,
    upload_readiness: commands::UploadReadinessArgs,
    upload_plan: commands::PlanArgs,
    upload_execute: commands::UploadExecuteArgs,
    upload_status: commands::NoArgs,
    upload_cancel: commands::NoArgs,
    upload_log: commands::UploadLogArgs,
    upload_confirm: commands::UploadConfirmArgs,
    upload_refdata: commands::UploadRefdataArgs,
    template_list: commands::NoArgs,
    template_tags: commands::NoArgs,
    template_save: commands::TemplateSaveArgs,
    template_import: commands::TemplateImportArgs,
    template_export: commands::TemplateExportArgs,
    template_remove: commands::TemplateIdArgs,
    template_insert_plan: commands::TemplateInsertArgs,
    template_insert: commands::TemplateInsertArgs,
    gallery_list: commands::GalleryListArgs,
    gallery_download: commands::GalleryDownloadArgs,
}

image_commands! {
    render_image: commands::RenderImageArgs,
    render_canvas: commands::RenderCanvasArgs,
    render_gallery: commands::RenderGalleryArgs,
    render_preview_edit: commands::RenderPreviewEditArgs,
    render_preview_string: commands::RenderPreviewStringArgs,
    asset_thumbnail: commands::AssetThumbnailArgs,
    frames_thumbnail: commands::FramesThumbnailArgs,
    template_preview: commands::TemplatePreviewArgs,
    gallery_preview: commands::GalleryPreviewArgs,
}

/// GUI-only command (no `vitrine-app` counterpart): installs the native
/// menu bar from the UI's spec (`native_menu`). Answers `{native: true}`
/// on macOS; elsewhere the spec is only checked and the in-app menu stays
/// the only menu (`{native: false}`).
#[tauri::command]
#[allow(
    clippy::needless_pass_by_value,
    reason = "Tauri command arguments are passed by value"
)]
fn menu_set(
    app: AppHandle,
    args: native_menu::MenuSpec,
) -> Result<serde_json::Value, CommandError> {
    args.validate().map_err(CommandError::invalid)?;
    #[cfg(target_os = "macos")]
    {
        native_menu::install(&app, &args)
            .map_err(|e| CommandError::internal(format!("cannot set the menu: {e}")))?;
        Ok(serde_json::json!({ "native": true }))
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = app;
        Ok(serde_json::json!({ "native": false }))
    }
}

/// GUI-only (ADR 0026): what this build can do with updates, the
/// setting and the last check (`updater::status`).
#[tauri::command]
#[allow(
    clippy::needless_pass_by_value,
    reason = "Tauri command arguments are passed by value"
)]
fn update_status(
    app: AppHandle,
    state: tauri::State<'_, Shared>,
) -> Result<updater::StatusReply, CommandError> {
    updater::status(&app, state.inner())
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct UpdateCheckArgs {
    /// Automatic check at start (only when due and switched on).
    #[serde(default)]
    auto: bool,
}

/// GUI-only: asks GitHub Releases for a newer version (never installs).
#[tauri::command]
async fn update_check(
    app: AppHandle,
    state: tauri::State<'_, Shared>,
    updater: tauri::State<'_, updater::UpdaterState>,
    args: UpdateCheckArgs,
) -> Result<updater::CheckReply, CommandError> {
    updater::check(&app, state.inner(), updater.inner(), args.auto).await
}

/// GUI-only: installs the update found by `update_check` and restarts;
/// the UI calls it only after the user confirmed.
#[tauri::command]
async fn update_install(
    app: AppHandle,
    state: tauri::State<'_, Shared>,
    updater: tauri::State<'_, updater::UpdaterState>,
) -> Result<(), CommandError> {
    updater::install(&app, state.inner(), updater.inner()).await
}

/// `vitrine-gui [PROJECT_DIR]`: the project to open right away.
fn project_arg() -> Option<PathBuf> {
    std::env::args_os()
        .skip(1)
        .find(|a| !a.to_string_lossy().starts_with('-'))
        .map(PathBuf::from)
}

#[allow(clippy::too_many_lines, reason = "the generate_handler! command list")]
fn main() {
    let events = Arc::new(TauriEvents::default());
    let dialogs = Arc::new(TauriDialogs::default());
    let paths = AppPaths::platform()
        .unwrap_or_else(|| AppPaths::in_dir(&std::env::temp_dir().join("vitrine")));
    // Export progress of MCP calls is tapped from the app's event stream.
    let relay = Arc::new(ProgressRelay::new(events.clone()));
    let discovery = vitrine_mcp::discovery::path(&paths);
    let state: Shared = Arc::new(AppState::new(paths, relay.clone(), dialogs.clone()));
    // The real app always uploads to the production stores (no env overrides).
    state
        .upload()
        .set_config(vitrine_gui::upload::UploadConfig::default());
    // "Test connection" of store accounts uses the real store clients (read-only).
    state.set_connection_tester(Arc::new(
        vitrine_gui::store_test::StoreClientTester::default(),
    ));
    let open_at_start = project_arg();
    let startup = state.clone();
    let mcp_host: Arc<OnceLock<Arc<McpHost>>> = Arc::default();
    let mcp_setup = mcp_host.clone();
    // Updates: the endpoint is stopped before the app exits for the
    // installer or the restart (the `Exit` event may not come then).
    let updates = updater::UpdaterState::default();
    let mcp_exit = mcp_host.clone();
    updates.set_before_exit(Arc::new(move || {
        if let Some(host) = mcp_exit.get() {
            host.stop();
        }
    }));

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(state)
        .manage(updates)
        // Native menu items (macOS) run the UI's own actions.
        .on_menu_event(|app, event| {
            let _ = app.emit(native_menu::MENU_EVENT, event.id().0.clone());
        })
        .setup(move |app| {
            let _ = events.0.set(app.handle().clone());
            let _ = dialogs.0.set(app.handle().clone());
            if let Some(dir) = &open_at_start
                && let Err(e) =
                    commands::project_open(&startup, commands::ProjectOpenArgs { dir: dir.clone() })
            {
                eprintln!("cannot open {}: {}", dir.display(), e.message);
            }
            let host = McpHost::install(
                &startup,
                Some(relay.clone()),
                HostOptions {
                    port: None,
                    discovery: Some(discovery.clone()),
                },
            );
            let _ = mcp_setup.set(host);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            app_info,
            project_new,
            project_open,
            project_close,
            project_summary,
            project_get,
            recent_projects,
            recent_remove,
            apply_ops,
            undo,
            redo,
            history_state,
            overlay,
            edit_move,
            edit_move_by,
            edit_resize,
            edit_rotate,
            edit_set_page,
            edit_reorder,
            edit_duplicate,
            edit_remove,
            edit_group,
            edit_ungroup,
            edit_copy,
            edit_cut,
            edit_paste,
            edit_clipboard,
            edit_reset_override,
            element_reset_override,
            element_get,
            element_patch,
            edit_rename,
            edit_arrange,
            element_add,
            canvas_get,
            canvas_patch,
            canvas_variant,
            canvas_add,
            canvas_rename,
            canvas_remove,
            validate,
            export,
            export_cancel,
            strings_get,
            strings_missing,
            assets_list,
            assets_import,
            assets_remove,
            catalog_devices,
            catalog_targets,
            catalog_galleries,
            catalog_fonts,
            settings_get,
            settings_set,
            dialog_open_project,
            dialog_pick_files,
            preview_stats,
            target_get,
            target_add_from_preset,
            target_patch,
            target_remove,
            canvas_banner_options,
            target_add_banner,
            project_rename,
            language_add,
            language_remove,
            languages_set,
            default_language_impact,
            default_language_set,
            styles_get,
            styles_patch,
            dialog_pick_folder,
            reveal_path,
            dialog_save_file,
            strings_export,
            strings_import_plan,
            strings_import_apply,
            import_plan,
            import_apply,
            render_image,
            render_canvas,
            render_gallery,
            render_preview_edit,
            element_info,
            markup_check,
            assets_probe,
            asset_info,
            assets_upload,
            render_preview_string,
            asset_thumbnail,
            frames_import_plan,
            frames_import_apply,
            frames_list,
            frames_remove,
            frames_pack_import,
            frames_pack_remove,
            frames_embed_status,
            frames_embed,
            frames_unembed,
            frames_thumbnail,
            view_get,
            view_set,
            mcp_status,
            mcp_set_enabled,
            claude_desktop_plan,
            claude_desktop_install,
            cli_install_status,
            cli_install,
            store_accounts_list,
            store_account_import,
            store_account_remove,
            store_account_test,
            store_apps_list,
            store_package_check,
            open_url,
            project_store_get,
            project_store_patch,
            store_config_get,
            upload_readiness,
            upload_plan,
            upload_execute,
            upload_status,
            upload_cancel,
            upload_log,
            upload_confirm,
            upload_refdata,
            template_list,
            template_tags,
            template_save,
            template_import,
            template_export,
            template_remove,
            template_insert_plan,
            template_insert,
            template_preview,
            gallery_list,
            gallery_download,
            gallery_preview,
            menu_set,
            update_status,
            update_check,
            update_install,
        ])
        .build(tauri::generate_context!())
        .expect("error while building Vitrine Studio")
        .run(move |_, event| {
            // Clean exit: stop the endpoint, remove the discovery file.
            if let RunEvent::Exit = event
                && let Some(host) = mcp_host.get()
            {
                host.stop();
            }
        });
}

#[cfg(test)]
mod tests {
    #[test]
    fn every_library_command_has_a_tauri_wrapper() {
        let source = include_str!("main.rs");
        let handler = source
            .split("generate_handler![")
            .nth(1)
            .and_then(|s| s.split(']').next())
            .unwrap();
        for name in vitrine_gui::commands::JSON_COMMANDS
            .iter()
            .chain(vitrine_gui::commands::IMAGE_COMMANDS)
        {
            assert!(
                handler.split(',').any(|n| n.trim() == *name),
                "`{name}` is not registered in main.rs"
            );
        }
    }
}
