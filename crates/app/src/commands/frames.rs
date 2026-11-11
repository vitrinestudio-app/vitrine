//! Imported vendor frames (KONZEPT §4.2, ADR 0012) and own frame packs
//! (ADR 0024): app settings → "Frame packages". The store lives in the app data folder
//! (`<data>/frames/`), never in a project; after every change the open
//! session renders with the new index.

use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use vitrine_core::frame_import::pack::{
    self, PACKS_DIR, PROJECT_FRAMES_DIR, PackImport, PackInfo, PackTarget,
};
use vitrine_core::frame_import::{
    Assignment, FRAMES_DIR, FrameIndex, FrameStore, ImportPlan, InstalledColor, InstalledFrame,
    Package, PackageError, RemoveFilter, Rules, import, normalize_name,
};
use vitrine_core::model::Orientation;
use vitrine_render::frame_packs;
use vitrine_render::raster::{Filter, SourceImage};

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

    /// Installed frames plus frame packs (app-wide and those of the
    /// project in `project`, ADR 0024); `None` when there is nothing.
    pub fn frame_index_for(&self, project: Option<&Path>) -> Option<Arc<FrameIndex>> {
        frame_packs::frame_index(Some(&self.frames_root()), project)
    }

    /// [`Self::frame_index_for`] the open project (if any).
    pub fn frame_index(&self) -> Option<Arc<FrameIndex>> {
        let dir = self.session().ok().map(|s| s.dir().to_owned());
        self.frame_index_for(dir.as_deref())
    }

    /// Hands the current index to the open project (re-render, re-validate).
    pub(crate) fn refresh_frames(&self) {
        if let Ok(s) = self.session() {
            s.set_frames(self.frame_index_for(Some(s.dir())), true);
        }
    }
}

fn rules(source: &str) -> CmdResult<&'static Rules> {
    Rules::builtin(source)
        .ok_or_else(|| CommandError::invalid(format!("unknown frame source `{source}`")))
}

/// Opens a package. A disk image with a licence agreement gives an error
/// with `details: {kind: "dmg_license", dmg, volume}` so the UI can explain
/// it (localized) and offer the folder picker; Vitrine Studio never accepts the
/// agreement itself.
fn open_package(path: &Path) -> CmdResult<Package> {
    Package::open(path).map_err(package_error)
}

pub(super) fn package_error(e: PackageError) -> CommandError {
    match &e {
        PackageError::LicenseAgreement { path, volume } => {
            CommandError::new(ErrorCode::Io, e.to_string()).with_details(serde_json::json!({
                "kind": "dmg_license",
                "dmg": path,
                "volume": volume,
            }))
        }
        PackageError::Other(m) => CommandError::new(ErrorCode::Io, m.clone()),
    }
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
                    if vitrine_core::catalog::device(&model).is_none() {
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
    /// Installed colours per model, with their measured swatches.
    pub colors: BTreeMap<String, Vec<InstalledColor>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FramesList {
    /// The store folder (app data, never a project).
    pub root: PathBuf,
    pub sources: Vec<FrameSourceInfo>,
    /// Own frame packs (ADR 0024): the open project's (`frames/`) first,
    /// then the app-wide ones (`<root>/packs/`).
    pub packs: Vec<PackInfo>,
}

pub fn frames_list(app: &AppState, _: NoArgs) -> CmdResult<FramesList> {
    let root = app.frames_root();
    // Stores imported before swatches existed: measure them once.
    if FrameStore::new(&root).fill_all_swatches() > 0 {
        app.refresh_frames();
    }
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
                colors: index
                    .frames(&r.source)
                    .iter()
                    .map(|f| f.model.clone())
                    .collect::<std::collections::BTreeSet<_>>()
                    .into_iter()
                    .map(|m| {
                        let c = index.installed_colors(&r.source, &m);
                        (m, c)
                    })
                    .collect(),
            }
        })
        .collect();
    let project = app.session().ok().map(|s| s.dir().to_owned());
    let packs = frame_packs::load_packs(Some(&root), project.as_deref());
    Ok(FramesList {
        root,
        sources,
        packs,
    })
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FramesPackImportArgs {
    /// Pack folder (with `frame.yaml`) or `.zip`.
    pub path: PathBuf,
    /// `app` (this computer, default) or `project` (the open project's
    /// `frames/` folder, travels with it).
    #[serde(default = "default_pack_target")]
    pub target: PackTarget,
    /// Pack id (default: from the pack's name).
    #[serde(default)]
    pub id: Option<String>,
    /// Only check (compile into a temporary folder).
    #[serde(default)]
    pub dry_run: bool,
}

fn default_pack_target() -> PackTarget {
    PackTarget::App
}

/// Folder the packs of `target` live in.
fn packs_dir(app: &AppState, target: PackTarget) -> CmdResult<PathBuf> {
    Ok(match target {
        PackTarget::App => app.frames_root().join(PACKS_DIR),
        PackTarget::Project => app.session()?.dir().join(PROJECT_FRAMES_DIR),
    })
}

/// Checks (`dry_run`) or imports an own frame pack (ADR 0024) and
/// refreshes the open project.
pub fn frames_pack_import(app: &AppState, args: FramesPackImportArgs) -> CmdResult<PackImport> {
    if !pack::is_pack(&args.path) {
        return Err(CommandError::invalid(format!(
            "{}: not a frame pack (no `{}`)",
            args.path.display(),
            pack::PACK_FILE
        ))
        .with_details(serde_json::json!({ "kind": "not_a_pack" })));
    }
    let dest = packs_dir(app, args.target)?;
    let r = pack::import_pack(
        &args.path,
        &dest,
        args.target,
        args.id.as_deref(),
        !args.dry_run,
        Some(&frame_packs::ResvgRasterizer),
    )
    .map_err(CommandError::invalid)?;
    if r.applied {
        app.refresh_frames();
    }
    Ok(r)
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FramesPackRemoveArgs {
    pub id: String,
    /// Where the pack lives (`app` or `project`).
    #[serde(default = "default_pack_target")]
    pub target: PackTarget,
}

/// Removes an own frame pack (its folder); `false` if there was none.
pub fn frames_pack_remove(app: &AppState, args: FramesPackRemoveArgs) -> CmdResult<bool> {
    let dest = packs_dir(app, args.target)?;
    let project = match args.target {
        PackTarget::Project => Some(app.session()?.dir().to_owned()),
        PackTarget::App => None,
    };
    let removed = pack::remove_pack(&dest, &args.id, project.as_deref())
        .map_err(|e| CommandError::new(ErrorCode::Io, e))?;
    if removed {
        app.refresh_frames();
    }
    Ok(removed)
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
    /// Vendor model (`iPhone 18 Pro`).
    #[serde(default)]
    pub bezel: Option<String>,
}

/// Removes matching frames (no filter = all of the source); returns the
/// count.
pub fn frames_remove(app: &AppState, args: FramesRemoveArgs) -> CmdResult<usize> {
    let filter = RemoveFilter {
        model: args.model.as_deref(),
        color: args.color.as_deref(),
        orientation: args.orientation,
        bezel: args.bezel.as_deref(),
    };
    let n = FrameStore::new(app.frames_root())
        .remove(&args.source, &filter)
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
    let pack_id = args.source.strip_prefix(pack::PACK_PREFIX);
    let ok = !args.file.is_empty()
        && rel.components().all(|c| matches!(c, Component::Normal(_)))
        && match pack_id {
            Some(id) => vitrine_core::model::is_valid_id(id),
            None => Path::new(&args.source)
                .components()
                .all(|c| matches!(c, Component::Normal(_))),
        };
    if !ok {
        return Err(CommandError::invalid(format!(
            "`{}` is not an installed frame file",
            args.file
        )));
    }
    let base = match pack_id {
        // Compiled pack files (project packs win over app packs).
        Some(_) => app
            .frame_index()
            .map(|ix| ix.source_dir(&args.source))
            .ok_or_else(|| CommandError::not_found(format!("no frame pack `{}`", args.source)))?,
        None => app.frames_root().join(&args.source),
    };
    let abs = base.join(rel);
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
