//! Imported vendor frames (KONZEPT §4.2, ADR 0009): app settings →
//! "Frame packages". The store lives in the app data folder
//! (`<data>/frames/`), never in a project; after every change the open
//! session renders with the new index.

use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use arrangia_core::frame_import::{
    Assignment, FRAMES_DIR, FrameIndex, FrameStore, ImportPlan, InstalledFrame, Package, Rules,
    import, normalize_name,
};
use arrangia_core::model::Orientation;
use arrangia_render::raster::{Filter, SourceImage};
use serde::{Deserialize, Serialize};

use super::{AppState, NoArgs};
use crate::error::{CmdResult, CommandError, ErrorCode};
use crate::preview::{PixelFormat, PreviewImage};

/// The only built-in source so far.
const DEFAULT_SOURCE: &str = "apple";

fn default_source() -> String {
    DEFAULT_SOURCE.to_owned()
}

impl AppState {
    /// Root of the app-wide frame store.
    pub fn frames_root(&self) -> PathBuf {
        self.paths().data_dir.join(FRAMES_DIR)
    }

    /// Installed frames (`None` when nothing is installed).
    pub fn frame_index(&self) -> Option<Arc<FrameIndex>> {
        let index = FrameIndex::load(&self.frames_root());
        (!index.is_empty()).then(|| Arc::new(index))
    }

    /// Hands the current index to the open project (re-render, re-validate).
    fn refresh_frames(&self) {
        if let Ok(s) = self.session() {
            s.set_frames(self.frame_index(), true);
        }
    }
}

fn rules(source: &str) -> CmdResult<&'static Rules> {
    Rules::builtin(source)
        .ok_or_else(|| CommandError::invalid(format!("unknown frame source `{source}`")))
}

fn open_package(path: &Path) -> CmdResult<Package> {
    Package::open(path).map_err(|e| CommandError::new(ErrorCode::Io, e))
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FramesImportPlanArgs {
    /// Folder, `.zip` or `.dmg`.
    pub path: PathBuf,
    #[serde(default = "default_source")]
    pub source: String,
}

/// What an import of `path` would do (nothing is written).
pub fn frames_import_plan(app: &AppState, args: FramesImportPlanArgs) -> CmdResult<ImportPlan> {
    let rules = rules(&args.source)?;
    let package = open_package(&args.path)?;
    import(&FrameStore::new(app.frames_root()), &package, rules, false)
        .map_err(|e| CommandError::new(ErrorCode::Io, e))
}

/// A manual assignment of an unmatched file (`model: null` = ignore it).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileAssignment {
    /// Normalised file name, as in the plan (`name`).
    pub name: String,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub orientation: Option<Orientation>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FramesImportApplyArgs {
    pub path: PathBuf,
    #[serde(default = "default_source")]
    pub source: String,
    /// Saved as user mapping overrides (also for later imports).
    #[serde(default)]
    pub assignments: Vec<FileAssignment>,
}

/// Saves the assignments, imports, and refreshes the open project.
pub fn frames_import_apply(app: &AppState, args: FramesImportApplyArgs) -> CmdResult<ImportPlan> {
    let rules = rules(&args.source)?;
    let store = FrameStore::new(app.frames_root());
    if !args.assignments.is_empty() {
        let io = |e: String| CommandError::new(ErrorCode::Io, e);
        let mut mapping = store.user_mapping(&args.source).map_err(io)?;
        for a in args.assignments {
            let value = match (a.model, a.color, a.orientation) {
                (Some(model), Some(color), Some(orientation)) => {
                    if arrangia_core::catalog::device(&model).is_none() {
                        return Err(CommandError::invalid(format!("unknown model `{model}`")));
                    }
                    if color.trim().is_empty() {
                        return Err(CommandError::invalid(format!(
                            "`{}`: a colour is required",
                            a.name
                        )));
                    }
                    Some(Assignment {
                        model,
                        color: color.trim().to_owned(),
                        orientation,
                    })
                }
                (None, _, _) => None,
                _ => {
                    return Err(CommandError::invalid(format!(
                        "`{}`: model, colour and orientation are required",
                        a.name
                    )));
                }
            };
            mapping.files.insert(normalize_name(&a.name), value);
        }
        store
            .save_user_mapping(&args.source, &mapping)
            .map_err(io)?;
    }
    let package = open_package(&args.path)?;
    let plan =
        import(&store, &package, rules, true).map_err(|e| CommandError::new(ErrorCode::Io, e))?;
    drop(package);
    app.refresh_frames();
    Ok(plan)
}

/// One frame source in [`FramesList`].
#[derive(Debug, Clone, Serialize)]
pub struct FrameSourceInfo {
    pub source: String,
    pub label: String,
    /// The file-name rules are checked against a real package.
    pub verified: bool,
    /// Models the rules know (choices for manual assignment).
    pub models: Vec<String>,
    pub frames: Vec<InstalledFrame>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FramesList {
    /// The store folder (app data, never a project).
    pub root: PathBuf,
    pub sources: Vec<FrameSourceInfo>,
}

pub fn frames_list(app: &AppState, _: NoArgs) -> CmdResult<FramesList> {
    let root = app.frames_root();
    let index = FrameIndex::load(&root);
    let sources = [DEFAULT_SOURCE]
        .iter()
        .filter_map(|s| Rules::builtin(s))
        .map(|r| {
            let mut models: Vec<String> = r.rules.iter().map(|x| x.model.clone()).collect();
            models.sort();
            models.dedup();
            FrameSourceInfo {
                source: r.source.clone(),
                label: r.label.clone(),
                verified: r.verified,
                models,
                frames: index.frames(&r.source).to_vec(),
            }
        })
        .collect();
    Ok(FramesList { root, sources })
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FramesRemoveArgs {
    #[serde(default = "default_source")]
    pub source: String,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub orientation: Option<Orientation>,
}

/// Removes matching frames (no filter = all of the source); returns the
/// count.
pub fn frames_remove(app: &AppState, args: FramesRemoveArgs) -> CmdResult<usize> {
    let n = FrameStore::new(app.frames_root())
        .remove(
            &args.source,
            args.model.as_deref(),
            args.color.as_deref(),
            args.orientation,
        )
        .map_err(|e| CommandError::new(ErrorCode::Io, e))?;
    app.refresh_frames();
    Ok(n)
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FramesThumbnailArgs {
    #[serde(default = "default_source")]
    pub source: String,
    /// `file` of an installed frame (relative to the source folder).
    pub file: String,
    #[serde(default)]
    pub max_edge: Option<u32>,
}

/// PNG thumbnail of an installed bezel.
pub fn frames_thumbnail(app: &AppState, args: FramesThumbnailArgs) -> CmdResult<Arc<PreviewImage>> {
    let rel = Path::new(&args.file);
    let ok = !args.file.is_empty()
        && rel.components().all(|c| matches!(c, Component::Normal(_)))
        && Path::new(&args.source)
            .components()
            .all(|c| matches!(c, Component::Normal(_)));
    if !ok {
        return Err(CommandError::invalid(format!(
            "`{}` is not an installed frame file",
            args.file
        )));
    }
    let abs = app.frames_root().join(&args.source).join(rel);
    let render_err = |e: String| CommandError::new(ErrorCode::Render, e);
    let img = SourceImage::load(&abs)
        .map_err(|e| CommandError::not_found(format!("{}: {e}", args.file)))?;
    let edge = args.max_edge.unwrap_or(160).clamp(16, 1024);
    let k = (f64::from(edge) / f64::from(img.width.max(img.height).max(1))).min(1.0);
    let (tw, th) = (
        ((f64::from(img.width) * k).round() as u32).max(1),
        ((f64::from(img.height) * k).round() as u32).max(1),
    );
    let pm = img
        .resample(tw, th, None, Filter::Bilinear)
        .map_err(|e| render_err(e.to_string()))?;
    Ok(Arc::new(PreviewImage::encode(&pm, None, PixelFormat::Png)?))
}
