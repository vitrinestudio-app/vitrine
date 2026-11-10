//! Arrangia desktop editor.
//!
//! The web view only shows pixmaps rendered in Rust and draws interaction
//! overlays on top. All file and system access goes through the custom
//! commands below; the web view itself only gets `core:default`. Every
//! command is a thin wrapper around `arrangia_gui::commands` (the same
//! functions the dev HTTP bridge serves), run on a blocking worker thread.
//!
//! Usage: `arrangia-gui [PROJECT_DIR]` opens that project at start (else
//! the start screen).
//!
//! JS calls them as `invoke(name, { args: {...} })` (see `ui/js/transport.js`);
//! image commands answer with an `ArrayBuffer` (header + PNG/RGBA, see
//! `arrangia_gui::preview::PreviewImage::to_tauri_bytes`).

// Hide the console window on Windows release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

use arrangia_gui::commands::{self, AppState};
use arrangia_gui::dialogs::{Dialogs, FileFilter};
use arrangia_gui::settings::AppPaths;
use arrangia_gui::{CommandError, EventSink};
use tauri::{AppHandle, Emitter};
use tauri_plugin_dialog::DialogExt;

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
    edit_reset_override: commands::EditResetOverrideArgs,
    element_get: commands::ElementGetArgs,
    element_patch: commands::ElementPatchArgs,
    edit_rename: commands::EditRenameArgs,
    edit_arrange: commands::EditArrangeArgs,
    element_add: commands::ElementAddArgs,
    strip_get: commands::StripArgs,
    strip_patch: commands::StripPatchArgs,
    strip_variant: commands::StripVariantArgs,
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
    assets_upload: commands::AssetsUploadArgs,
    target_get: commands::TargetIdArgs,
    target_add_from_preset: commands::TargetAddArgs,
    target_patch: commands::TargetPatchArgs,
    target_remove: commands::TargetIdArgs,
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
}

image_commands! {
    render_image: commands::RenderImageArgs,
    render_canvas: commands::RenderCanvasArgs,
    render_gallery: commands::RenderGalleryArgs,
    render_preview_edit: commands::RenderPreviewEditArgs,
    render_preview_string: commands::RenderPreviewStringArgs,
    asset_thumbnail: commands::AssetThumbnailArgs,
    frames_thumbnail: commands::FramesThumbnailArgs,
}

/// `arrangia-gui [PROJECT_DIR]`: the project to open right away.
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
        .unwrap_or_else(|| AppPaths::in_dir(&std::env::temp_dir().join("arrangia")));
    let state: Shared = Arc::new(AppState::new(paths, events.clone(), dialogs.clone()));
    let open_at_start = project_arg();
    let startup = state.clone();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(state)
        .setup(move |app| {
            let _ = events.0.set(app.handle().clone());
            let _ = dialogs.0.set(app.handle().clone());
            if let Some(dir) = &open_at_start
                && let Err(e) =
                    commands::project_open(&startup, commands::ProjectOpenArgs { dir: dir.clone() })
            {
                eprintln!("cannot open {}: {}", dir.display(), e.message);
            }
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
            edit_reset_override,
            element_get,
            element_patch,
            edit_rename,
            edit_arrange,
            element_add,
            strip_get,
            strip_patch,
            strip_variant,
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
            assets_upload,
            render_preview_string,
            asset_thumbnail,
            frames_import_plan,
            frames_import_apply,
            frames_list,
            frames_remove,
            frames_thumbnail,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Arrangia");
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
        for name in arrangia_gui::commands::JSON_COMMANDS
            .iter()
            .chain(arrangia_gui::commands::IMAGE_COMMANDS)
        {
            assert!(
                handler.split(',').any(|n| n.trim() == *name),
                "`{name}` is not registered in main.rs"
            );
        }
    }
}
