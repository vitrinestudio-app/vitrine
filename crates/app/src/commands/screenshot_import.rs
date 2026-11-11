//! Screenshot import from a folder (`core::import`): `import_plan` shows
//! what would go where (fastlane, Vitrine Studio or flat layout, with per-row
//! corrections and extra mappings), `import_apply` re-plans with the same
//! arguments and copies the files, optionally saving the effective
//! mapping to `project.yaml` (`import.fastlane`, one undo step). The
//! folder watcher then refreshes assets and previews.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use vitrine_core::import::{
    ApplyReport, ImportPlan, ImportRules, ItemOverride, SourceFormat, apply, plan_files,
    plan_screenshot_import,
};
use vitrine_core::model::{ImportMapping, ImportSettings};
use vitrine_core::ops::Op;

use super::{AppState, absolute};
use crate::error::{CmdResult, CommandError};
use crate::session::ApplyResult;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImportPlanArgs {
    /// Source folder (absolute, or relative to the current folder).
    #[serde(default)]
    pub dir: Option<PathBuf>,
    /// Single files instead of a folder (drag & drop; flat rules).
    #[serde(default)]
    pub files: Vec<PathBuf>,
    /// Layout (default: auto-detect).
    #[serde(default)]
    pub format: Option<SourceFormat>,
    /// Extra device/locale/key maps on top of the project's.
    #[serde(default)]
    pub mapping: ImportMapping,
    /// Per-row corrections by source path.
    #[serde(default)]
    pub overrides: BTreeMap<String, ItemOverride>,
    #[serde(default)]
    pub include_framed: bool,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImportApplyArgs {
    #[serde(default)]
    pub dir: Option<PathBuf>,
    #[serde(default)]
    pub files: Vec<PathBuf>,
    #[serde(default)]
    pub format: Option<SourceFormat>,
    #[serde(default)]
    pub mapping: ImportMapping,
    #[serde(default)]
    pub overrides: BTreeMap<String, ItemOverride>,
    #[serde(default)]
    pub include_framed: bool,
    /// Store the plan's effective mapping in `project.yaml`.
    #[serde(default)]
    pub save_mapping: bool,
}

impl From<ImportApplyArgs> for ImportPlanArgs {
    fn from(a: ImportApplyArgs) -> Self {
        Self {
            dir: a.dir,
            files: a.files,
            format: a.format,
            mapping: a.mapping,
            overrides: a.overrides,
            include_framed: a.include_framed,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ImportApplyResult {
    pub plan: ImportPlan,
    pub report: ApplyReport,
    /// The apply result of the mapping change (`None` when not saved or
    /// unchanged).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mapping: Option<ApplyResult>,
}

fn make_plan(app: &AppState, args: &ImportPlanArgs) -> CmdResult<ImportPlan> {
    let session = app.session()?;
    args.mapping.check().map_err(CommandError::invalid)?;
    let rules = ImportRules {
        format: args.format.unwrap_or_default(),
        mapping: args.mapping.clone(),
        include_framed: args.include_framed,
        overrides: args.overrides.clone(),
    };
    let project = session.snapshot();
    match (&args.dir, args.files.is_empty()) {
        (Some(dir), true) => {
            let dir = absolute(dir)?;
            plan_screenshot_import(&project, session.dir(), &dir, &rules).map_err(|e| match e {
                vitrine_core::import::ImportError::NotADir(_) => {
                    CommandError::not_found(e.to_string())
                }
                vitrine_core::import::ImportError::Io { .. } => CommandError::io(e.to_string()),
            })
        }
        (None, false) => {
            for f in &args.files {
                if !f.is_file() {
                    return Err(CommandError::not_found(format!(
                        "`{}` is not a file",
                        f.display()
                    )));
                }
            }
            Ok(plan_files(&project, session.dir(), &args.files, &rules))
        }
        _ => Err(CommandError::invalid("pass either `dir` or `files`")),
    }
}

/// The import plan of a folder (or files); writes nothing.
pub fn import_plan(app: &AppState, args: ImportPlanArgs) -> CmdResult<ImportPlan> {
    make_plan(app, &args)
}

/// Re-plans with the same arguments and copies the files; with
/// `save_mapping` also stores the effective mapping (one undo step).
pub fn import_apply(app: &AppState, args: ImportApplyArgs) -> CmdResult<ImportApplyResult> {
    let save = args.save_mapping;
    let plan = make_plan(app, &args.into())?;
    let session = app.session()?;
    let report = apply(&plan, session.dir());
    let mut mapping = None;
    if save && session.with_project(|p| p.import.fastlane != plan.mapping) {
        let import = ImportSettings {
            fastlane: plan.mapping.clone(),
        };
        mapping = Some(session.apply_ops(
            vec![Op::SetImport { import }],
            Some("import mapping".to_owned()),
        )?);
    }
    Ok(ImportApplyResult {
        plan,
        report,
        mapping,
    })
}
