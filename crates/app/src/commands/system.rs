//! App info, settings and native dialogs.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::assets::AssetKind;
use super::project::{ProjectOpenArgs, ProjectSummary, project_open};
use super::{AppState, NoArgs};
use crate::AppInfo;
use crate::dialogs::FileFilter;
use crate::error::CmdResult;
use crate::settings::{Settings, UI_LANGUAGES};

pub fn app_info(_: &AppState, _: NoArgs) -> CmdResult<AppInfo> {
    Ok(crate::app_info())
}

#[derive(Debug, Clone, Serialize)]
pub struct SettingsInfo {
    pub settings: Settings,
    pub ui_languages: &'static [&'static str],
    /// `settings.yaml`.
    pub settings_file: PathBuf,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SettingsSetArgs {
    /// Fields to change (`null` clears an optional field).
    pub settings: serde_json::Value,
}

pub fn settings_get(app: &AppState, _: NoArgs) -> CmdResult<SettingsInfo> {
    Ok(SettingsInfo {
        settings: app.settings_store().load()?,
        ui_languages: &UI_LANGUAGES,
        settings_file: app.paths().settings_file(),
    })
}

pub fn settings_set(app: &AppState, args: SettingsSetArgs) -> CmdResult<Settings> {
    let settings = app.settings_store().update(&args.settings)?;
    if let Ok(session) = app.session() {
        session.set_system_fonts(settings.system_fonts);
    }
    Ok(settings)
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DialogOpenProjectArgs {
    #[serde(default)]
    pub title: Option<String>,
}

/// Asks for a project folder and opens it (`null` when cancelled).
pub fn dialog_open_project(
    app: &AppState,
    args: DialogOpenProjectArgs,
) -> CmdResult<Option<ProjectSummary>> {
    let title = args.title.unwrap_or_else(|| "Open project".to_owned());
    match app.dialogs().pick_folder(&title) {
        Some(dir) => project_open(app, ProjectOpenArgs { dir }).map(Some),
        None => Ok(None),
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DialogPickFilesArgs {
    #[serde(default)]
    pub title: Option<String>,
    /// File type filter (default: any file).
    #[serde(default)]
    pub kind: Option<AssetKind>,
    #[serde(default = "yes")]
    pub multiple: bool,
    /// Explicit extensions (`["p8"]`); used instead of `kind`.
    #[serde(default)]
    pub extensions: Option<Vec<String>>,
}

fn yes() -> bool {
    true
}

/// Asks for files (empty when cancelled); import them with `assets_import`.
pub fn dialog_pick_files(app: &AppState, args: DialogPickFilesArgs) -> CmdResult<Vec<PathBuf>> {
    let filters: Vec<FileFilter> = if let Some(ext) = args.extensions.filter(|e| !e.is_empty()) {
        vec![FileFilter {
            name: ext.join(", "),
            extensions: ext,
        }]
    } else {
        args.kind
            .map(|k| FileFilter {
                name: format!("{k:?}"),
                extensions: k.extensions().iter().map(|e| (*e).to_owned()).collect(),
            })
            .into_iter()
            .collect()
    };
    let title = args.title.unwrap_or_else(|| "Choose files".to_owned());
    Ok(app.dialogs().pick_files(&title, &filters, args.multiple))
}
