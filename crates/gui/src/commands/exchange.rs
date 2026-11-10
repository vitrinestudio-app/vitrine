//! Translation exchange (`core::exchange`, ADR 0011): export CSV/XLIFF to
//! a path, plan an import (dry run with diff, markup issues and layout
//! findings), apply a plan as one undo step. Plus the save dialog.

use std::path::PathBuf;

use arrangia_core::exchange::{
    self, ExportFile, ExportOptions, Format, ImportInput, ImportOptions, ImportPlan,
};
use arrangia_core::resolve::FsAssets;
use arrangia_core::validate::{ValidateMode, ValidateOptions};
use serde::{Deserialize, Serialize};

use super::AppState;
use crate::dialogs::FileFilter;
use crate::error::{CmdResult, CommandError, ErrorCode};
use crate::session::ApplyResult;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DialogSaveFileArgs {
    #[serde(default)]
    pub title: Option<String>,
    /// Proposed file name.
    #[serde(default)]
    pub default_name: Option<String>,
    #[serde(default)]
    pub filters: Vec<FileFilter>,
}

/// Asks for a file to save to (`null` when cancelled).
pub fn dialog_save_file(app: &AppState, args: DialogSaveFileArgs) -> CmdResult<Option<PathBuf>> {
    let title = args.title.unwrap_or_else(|| "Save".to_owned());
    Ok(app.dialogs().save_file(
        &title,
        args.default_name.as_deref().unwrap_or_default(),
        &args.filters,
    ))
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StringsExportArgs {
    pub format: Format,
    /// Target languages (default: all but the default language).
    #[serde(default)]
    pub langs: Vec<String>,
    /// File (several XLIFF languages: `<stem>-<lang>.xlf` next to it) or
    /// folder; relative paths are relative to the project.
    pub path: PathBuf,
    /// `missing_only`, `context`, `delimiter`, `bom`, `markup`.
    #[serde(default)]
    pub options: ExportOptions,
}

/// One written file.
#[derive(Debug, Clone, Serialize)]
pub struct WrittenFile {
    pub path: PathBuf,
    #[serde(flatten)]
    pub file: ExportFile,
}

#[derive(Debug, Clone, Serialize)]
pub struct StringsExportResult {
    pub files: Vec<WrittenFile>,
}

fn exchange_err(e: exchange::ExchangeError) -> CommandError {
    match e {
        exchange::ExchangeError::UnknownLanguage(_) => CommandError::not_found(e.to_string()),
        _ => CommandError::invalid(e.to_string()),
    }
}

/// Writes the export files (default names in a folder, else at `path`).
pub fn strings_export(app: &AppState, args: StringsExportArgs) -> CmdResult<StringsExportResult> {
    let session = app.session()?;
    let project = session.snapshot();
    let mut opts = args.options;
    if !args.langs.is_empty() {
        opts.langs = args.langs;
    }
    let files = exchange::export(&project, args.format, &opts).map_err(exchange_err)?;
    let path = if args.path.is_absolute() {
        args.path
    } else {
        session.dir().join(args.path)
    };
    let paths = exchange::output_paths(&files, &path, path.is_dir());
    let mut out = Vec::new();
    for (file, path) in files.into_iter().zip(paths) {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)
                .map_err(|e| CommandError::io(format!("{}: {e}", dir.display())))?;
        }
        std::fs::write(&path, &file.content)
            .map_err(|e| CommandError::io(format!("{}: {e}", path.display())))?;
        out.push(WrittenFile { path, file });
    }
    Ok(StringsExportResult { files: out })
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StringsImportPlanArgs {
    /// CSV/XLIFF files (absolute, or relative to the project).
    pub paths: Vec<PathBuf>,
    /// `create_keys`, `empty` (`keep` | `remove`).
    #[serde(default)]
    pub options: ImportOptions,
    /// Run the layout check of the changed languages (default true).
    #[serde(default = "yes")]
    pub layout: bool,
}

fn yes() -> bool {
    true
}

/// Reads the files and compares them with the project (nothing changes).
pub fn strings_import_plan(app: &AppState, args: StringsImportPlanArgs) -> CmdResult<ImportPlan> {
    let session = app.session()?;
    if args.paths.is_empty() {
        return Err(CommandError::invalid("no files given"));
    }
    let mut inputs = Vec::new();
    for p in &args.paths {
        let path = if p.is_absolute() {
            p.clone()
        } else {
            session.dir().join(p)
        };
        let data = std::fs::read(&path)
            .map_err(|e| CommandError::io(format!("{}: {e}", path.display())))?;
        let name = path.file_name().map_or_else(
            || path.display().to_string(),
            |n| n.to_string_lossy().into_owned(),
        );
        inputs.push(ImportInput { name, data });
    }
    let project = session.snapshot();
    let mut plan = exchange::plan_import(&project, &inputs, &args.options);
    if args.layout {
        let fs = FsAssets::new(session.dir());
        let ctx = session.render_context();
        let mut opts = ValidateOptions::new(&fs);
        opts.fonts = ctx.fonts().clone();
        opts.mode = ValidateMode::Preview;
        exchange::check_layout(&mut plan, &project, &opts);
    }
    Ok(plan)
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StringsImportApplyArgs {
    /// A plan from `strings_import_plan`.
    pub plan: ImportPlan,
    #[serde(default)]
    pub label: Option<String>,
}

/// Applies a plan's changes as one undo step. Refused when a planned
/// string was edited since (plan again).
pub fn strings_import_apply(
    app: &AppState,
    args: StringsImportApplyArgs,
) -> CmdResult<ApplyResult> {
    let session = app.session()?;
    if args.plan.is_empty() {
        return Err(CommandError::invalid("the plan has no changes"));
    }
    let conflicts = session.with_project(|p| args.plan.conflicts(p));
    if !conflicts.is_empty() {
        return Err(CommandError::new(
            ErrorCode::Conflict,
            format!(
                "changed since the import was planned: {}",
                conflicts.join(", ")
            ),
        ));
    }
    let ops = args.plan.ops().map_err(CommandError::invalid)?;
    let n = ops.len();
    let label = args.label.unwrap_or_else(|| {
        format!(
            "Import translations ({n} string{})",
            if n == 1 { "" } else { "s" }
        )
    });
    session.apply_ops(ops, Some(label))
}
