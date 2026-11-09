//! Validation and export (with progress events and cancellation).

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use arrangia_core::resolve::FsAssets;
use arrangia_core::validate::{Severity, ValidateMode, ValidateOptions, ValidationReport};
use arrangia_render::export::{ExportHooks, ExportOptions, ExportReport};
use serde::{Deserialize, Serialize};
use serde_json::json;

use super::project::check_target;
use super::{AppState, NoArgs};
use crate::error::{CmdResult, CommandError, ErrorCode};
use crate::events::{EXPORT_FINISHED, EXPORT_PROGRESS};

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ValidateArgs {
    /// Only these targets (default: all).
    #[serde(default)]
    pub targets: Option<Vec<String>>,
    /// Only these languages (default: each target's languages).
    #[serde(default)]
    pub langs: Option<Vec<String>>,
    /// Editing mode: a missing screenshot is a warning, not an error.
    #[serde(default)]
    pub preview: bool,
}

/// Validation report with counts.
#[derive(Debug, Clone, Serialize)]
pub struct ValidateResult {
    pub errors: usize,
    pub warnings: usize,
    pub infos: usize,
    #[serde(flatten)]
    pub report: ValidationReport,
}

pub fn validate(app: &AppState, args: ValidateArgs) -> CmdResult<ValidateResult> {
    let session = app.session()?;
    let project = session.snapshot();
    for t in args.targets.iter().flatten() {
        check_target(&project, t)?;
    }
    let fs = FsAssets::new(session.dir());
    let ctx = session.render_context();
    let mut opts = ValidateOptions::new(&fs);
    opts.fonts = ctx.fonts().clone();
    opts.targets = args.targets;
    opts.langs = args.langs;
    opts.mode = if args.preview {
        ValidateMode::Preview
    } else {
        ValidateMode::Export
    };
    let report = arrangia_core::validate::validate(&project, &opts);
    Ok(ValidateResult {
        errors: report.count(Severity::Error),
        warnings: report.count(Severity::Warn),
        infos: report.count(Severity::Info),
        report,
    })
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExportArgs {
    /// Only these targets (default: all).
    #[serde(default)]
    pub targets: Vec<String>,
    /// Only these languages (default: each target's languages).
    #[serde(default)]
    pub langs: Vec<String>,
    /// Render every file, ignoring the incremental cache.
    #[serde(default)]
    pub force: bool,
    /// Export folder (default `<project>/export`).
    #[serde(default)]
    pub out_dir: Option<PathBuf>,
    /// Write `_gallery.png` previews (default true).
    #[serde(default)]
    pub gallery: Option<bool>,
}

/// Clears the running-export slot when the export ends (also on panic).
struct Running<'a>(&'a AppState);

impl Drop for Running<'_> {
    fn drop(&mut self) {
        self.0
            .export_cancel
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
    }
}

/// Exports targets × languages; blocks until done (call it off the UI
/// thread). Emits `export-progress` per file and `export-finished`.
pub fn export(app: &AppState, args: ExportArgs) -> CmdResult<ExportReport> {
    let session = app.session()?;
    let project = session.snapshot();
    for t in &args.targets {
        check_target(&project, t)?;
    }
    let cancel = Arc::new(AtomicBool::new(false));
    {
        let mut slot = app
            .export_cancel
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if slot.is_some() {
            return Err(CommandError::new(
                ErrorCode::Busy,
                "an export is already running",
            ));
        }
        *slot = Some(cancel.clone());
    }
    let _running = Running(app);
    let events = app.events().clone();
    let progress = Arc::new(
        move |file: &arrangia_render::export::ExportedFile, done, total| {
            events.emit(
                EXPORT_PROGRESS,
                json!({ "file": file, "done": done, "total": total }),
            );
        },
    );
    let ctx = session.render_context();
    let report = arrangia_render::export::export(
        &project,
        &ctx,
        &ExportOptions {
            out_dir: args.out_dir,
            targets: args.targets,
            langs: args.langs,
            gallery: args.gallery.unwrap_or(true),
            incremental: !args.force,
            hooks: ExportHooks {
                progress: Some(progress),
                cancel: Some(cancel),
            },
            ..ExportOptions::default()
        },
    );
    app.events().emit(
        EXPORT_FINISHED,
        json!({
            "cancelled": report.cancelled,
            "written": report.written().count(),
            "files": report.files.len(),
            "errors": report.errors.len(),
            "out_dir": report.out_dir,
        }),
    );
    Ok(report)
}

/// Whether an export was running (and is now asked to stop).
#[derive(Debug, Clone, Copy, Serialize)]
pub struct CancelResult {
    pub cancelled: bool,
}

pub fn export_cancel(app: &AppState, _: NoArgs) -> CmdResult<CancelResult> {
    let slot = app
        .export_cancel
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let cancelled = slot.as_ref().is_some_and(|c| {
        c.store(true, Ordering::Relaxed);
        true
    });
    Ok(CancelResult { cancelled })
}
