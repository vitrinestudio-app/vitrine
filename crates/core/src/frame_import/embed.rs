//! Imported vendor frames stored in the project (ADR 0027): "Store used
//! device frames in the project" copies the bezels a project actually uses
//! from the app-wide store into
//!
//! ```text
//! <project>/frames/imported/
//!   README.md                         licence notice (generated)
//!   embed.yaml                        options of the copy (all sizes or not)
//!   <source>/index.yaml               like the app-wide store (FrameStore layout)
//!   <source>/<model>/<colour>/<orientation>/<vendor-model>.png|.mask.png
//!   <source>/licenses/<package>/License.rtf   the package's licence, if imported with it
//! ```
//!
//! so that `vitrine export|render|validate` on a build server, where the
//! app-wide store is empty, draws the same bezels. Resolution: the project
//! copy first, then the app-wide store, then the parametric fallback
//! ([`super::FrameIndex::add_project_copies`]).
//!
//! The copies stay subject to the vendor's licence (Apple: mock-ups yes,
//! redistribution no): fine in a private repository, never in a published
//! project, template or gallery upload.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::pack::PROJECT_FRAMES_DIR;
use super::rules::slug;
use super::store::{FrameIndex, FrameOrigin, FrameStore, INDEX_FILE, InstalledFrame, RemoveFilter};
use crate::io::{from_yaml, to_yaml, write_atomic};
use crate::model::{FrameStyle, Orientation, Project};
use crate::resolve::{AssetLookup, DeviceColor, ResolveOptions, ResolvedKind, resolve};

/// Folder of the project copies below the project's `frames/` folder (a
/// reserved frame-pack id).
pub const EMBED_DIR: &str = "imported";
/// Licence notice written into [`embed_root`].
pub const README_FILE: &str = "README.md";
/// Options of the copy.
pub const OPTIONS_FILE: &str = "embed.yaml";

/// Apple's licence of the Design Resources (shown in the notice).
pub const APPLE_LICENSE_URL: &str = "https://developer.apple.com/support/downloads/terms/apple-design-resources/Apple-Design-Resources-License-20230621-English.pdf";

/// `<project>/frames/imported`.
pub fn embed_root(project: &Path) -> PathBuf {
    project.join(PROJECT_FRAMES_DIR).join(EMBED_DIR)
}

/// Project-relative form of [`embed_root`] (for messages).
pub fn embed_dir_display() -> String {
    format!("{PROJECT_FRAMES_DIR}/{EMBED_DIR}/")
}

/// `embed.yaml`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EmbedOptions {
    /// Keep every installed size (vendor model) of the used model ×
    /// colour × orientation, not only the bezels the targets draw.
    #[serde(default)]
    pub all_sizes: bool,
}

/// Reads `embed.yaml` (default when missing or unreadable).
pub fn read_options(project: &Path) -> EmbedOptions {
    fs::read_to_string(embed_root(project).join(OPTIONS_FILE))
        .ok()
        .and_then(|t| from_yaml(&t).ok())
        .unwrap_or_default()
}

/// Source folders with an `index.yaml` below [`embed_root`].
fn stored_sources(project: &Path) -> Vec<String> {
    let mut out: Vec<String> = fs::read_dir(embed_root(project))
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().join(INDEX_FILE).is_file())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|s| crate::model::is_valid_id(s))
        .collect();
    out.sort();
    out
}

/// Does the project hold stored frames?
pub fn has_copies(project: &Path) -> bool {
    !stored_sources(project).is_empty()
}

/// Adds the project's stored frames to `index` (they win over app-wide
/// frames of the same slot); unreadable indexes are skipped.
pub fn add_project_copies(index: &mut FrameIndex, project: &Path) {
    let root = embed_root(project);
    let store = FrameStore::new(&root);
    for source in stored_sources(project) {
        if let Ok(ix) = store.source_index(&source) {
            index.add_project_copies(&source, &root.join(&source), ix);
        }
    }
    index.set_project_all_sizes(read_options(project).all_sizes);
}

/// One frame identity: source × model × colour × orientation × vendor
/// model (slug).
type FrameKey = (String, String, String, Orientation, String);

fn key_of(source: &str, f: &InstalledFrame) -> FrameKey {
    (
        source.to_owned(),
        f.model.clone(),
        f.color.clone(),
        f.orientation,
        slug(&f.apple_model),
    )
}

/// A bezel the project draws.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FrameUse {
    pub source: String,
    pub model: String,
    pub color: String,
    pub color_label: String,
    pub orientation: Orientation,
    /// Vendor model (`iPhone 18 Pro`).
    pub bezel: String,
    /// Where it is drawn from now.
    pub origin: FrameOrigin,
    /// Targets drawing it.
    pub targets: Vec<String>,
}

/// A device asking for an imported frame that is installed nowhere (the
/// parametric frame is drawn).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MissingUse {
    pub source: String,
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    pub orientation: Orientation,
    pub targets: Vec<String>,
}

/// What the targets of a project draw.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Usage {
    pub used: Vec<FrameUse>,
    pub missing: Vec<MissingUse>,
}

/// Languages a target is exported in (its `langs`, else all available,
/// at least the default).
fn target_langs(project: &Project, target: &crate::model::Target) -> Vec<String> {
    match &target.langs {
        Some(l) => l.iter().map(|l| l.as_str().to_owned()).collect(),
        None if project.languages.available.is_empty() => {
            vec![project.languages.default.as_str().to_owned()]
        }
        None => project
            .languages
            .available
            .iter()
            .map(|l| l.as_str().to_owned())
            .collect(),
    }
}

/// Resolves every target × language with `index` and collects the
/// imported vendor bezels drawn (frame packs are not included: project
/// packs travel with the project anyway).
pub fn used_frames(project: &Project, index: &FrameIndex, assets: &dyn AssetLookup) -> Usage {
    let opts = ResolveOptions::new(assets).with_frames(Some(index));
    let mut used: BTreeMap<FrameKey, FrameUse> = BTreeMap::new();
    let mut missing: BTreeMap<(String, String, Option<String>, Orientation), BTreeSet<String>> =
        BTreeMap::new();
    for target in &project.targets {
        let tid = target.id.to_string();
        for lang in target_langs(project, target) {
            let Ok(scene) = resolve(project, target.id.as_str(), &lang, &opts) else {
                continue;
            };
            for e in &scene.elements {
                let ResolvedKind::Device(d) = &e.props else {
                    continue;
                };
                let FrameStyle::Imported(source) = &d.frame else {
                    continue;
                };
                let Some(f) = &d.imported else {
                    if super::Rules::builtin(source).is_some_and(|r| !r.covers(&d.model)) {
                        continue;
                    }
                    let color = match &d.color {
                        DeviceColor::Named(c) => Some(c.clone()),
                        _ => None,
                    };
                    missing
                        .entry((source.clone(), d.model.clone(), color, d.orientation))
                        .or_default()
                        .insert(tid.clone());
                    continue;
                };
                // `file` is unique within one origin's folder.
                let Some(frame) = index
                    .frames(source)
                    .iter()
                    .find(|x| x.origin == f.origin && x.file == f.file)
                else {
                    continue;
                };
                let u = used
                    .entry(key_of(source, frame))
                    .or_insert_with(|| FrameUse {
                        source: source.clone(),
                        model: frame.model.clone(),
                        color: frame.color.clone(),
                        color_label: frame.color_label.clone(),
                        orientation: frame.orientation,
                        bezel: frame.apple_model.clone(),
                        origin: frame.origin,
                        targets: Vec::new(),
                    });
                if !u.targets.contains(&tid) {
                    u.targets.push(tid.clone());
                }
            }
        }
    }
    Usage {
        used: used.into_values().collect(),
        missing: missing
            .into_iter()
            .map(|((source, model, color, orientation), t)| MissingUse {
                source,
                model,
                color,
                orientation,
                targets: t.into_iter().collect(),
            })
            .collect(),
    }
}

/// What [`plan`] / [`apply`] does with one stored frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EmbedAction {
    /// Copied from this computer's store (not in the project yet).
    Add,
    /// The project copy differs from this computer's (re-imported):
    /// replaced by it.
    Update,
    /// Stored and current (or this computer has no copy to compare).
    Keep,
    /// Stored but no longer used: removed.
    Remove,
}

/// One frame of an [`EmbedPlan`].
#[derive(Debug, Clone, Serialize)]
pub struct EmbedItem {
    pub source: String,
    pub model: String,
    pub color: String,
    pub color_label: String,
    pub orientation: Orientation,
    /// Vendor model (`iPhone 18 Pro`).
    pub bezel: String,
    pub action: EmbedAction,
    /// Bezel + mask size in bytes (of the copy that is or will be stored).
    pub bytes: u64,
    /// Targets drawing it (empty for extra sizes and removed copies).
    pub targets: Vec<String>,
    /// Bezel PNG relative to its source folder (thumbnails).
    pub file: String,
    /// Where `file` is: `project` (stored) or `app` (to be copied).
    pub origin: FrameOrigin,
}

/// Result of [`plan`] / [`apply`].
#[derive(Debug, Clone, Serialize)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "flat JSON for the front ends"
)]
pub struct EmbedPlan {
    /// `<project>/frames/imported`.
    pub dir: PathBuf,
    /// The project holds stored frames (before this call for a plan, after
    /// it for [`apply`]).
    pub stored: bool,
    pub all_sizes: bool,
    pub items: Vec<EmbedItem>,
    /// Devices whose imported frame is installed nowhere (stay parametric).
    pub missing: Vec<MissingUse>,
    /// Bytes stored in the project now (bezels + masks).
    pub bytes_now: u64,
    /// Bytes stored after applying the plan.
    pub bytes_after: u64,
    /// Licence files of the packages (`apple/licenses/<package>/License.rtf`,
    /// relative to `dir`) stored with the frames (after applying).
    pub licenses: Vec<String>,
    /// Packages whose licence file is not available on this computer (the
    /// store was filled before licence files were kept; re-import the
    /// package to add it).
    pub licenses_missing: Vec<String>,
    /// Nothing to add, update or remove.
    pub up_to_date: bool,
    /// `false` = dry run.
    pub applied: bool,
}

fn file_len(p: &Path) -> u64 {
    fs::metadata(p).map_or(0, |m| m.len())
}

fn rel_path(base: &Path, rel: &str) -> PathBuf {
    rel.split('/').fold(base.to_owned(), |p, c| p.join(c))
}

/// Bytes of a frame's bezel + mask in `index`.
fn frame_bytes(index: &FrameIndex, source: &str, f: &InstalledFrame) -> u64 {
    let dir = index.source_dir(&f.files_key(source));
    file_len(&rel_path(&dir, &f.file)) + file_len(&rel_path(&dir, &f.mask))
}

/// Does the stored copy `p` differ from this computer's `a` (re-imported
/// since)? Compares the recorded analysis and the bezel file size.
pub fn differs(index: &FrameIndex, source: &str, p: &InstalledFrame, a: &InstalledFrame) -> bool {
    let meta = |f: &InstalledFrame| {
        (
            f.size,
            f.screen,
            f.imported.clone(),
            f.package.clone(),
            f.source_file.clone(),
        )
    };
    if meta(p) != meta(a) {
        return true;
    }
    let len = |f: &InstalledFrame| {
        let dir = index.source_dir(&f.files_key(source));
        file_len(&rel_path(&dir, &f.file))
    };
    len(p) != len(a)
}

/// Sources of imported vendor frames in `index` (no frame packs, no
/// project-copy keys).
fn vendor_sources(index: &FrameIndex) -> Vec<String> {
    index
        .source_dirs()
        .into_iter()
        .map(|(k, _)| k)
        .filter(|k| !k.contains(':'))
        .collect()
}

/// What storing the used frames in the project would do. `index` must be
/// the full index (app store + project copies, [`add_project_copies`]);
/// `all_sizes` keeps every installed vendor model of a used model ×
/// colour × orientation.
#[allow(clippy::too_many_lines)]
pub fn plan(
    project: &Project,
    project_dir: &Path,
    index: &FrameIndex,
    assets: &dyn AssetLookup,
    all_sizes: bool,
) -> EmbedPlan {
    let usage = used_frames(project, index, assets);
    let mut wanted: BTreeMap<FrameKey, Vec<String>> = usage
        .used
        .iter()
        .map(|u| {
            (
                (
                    u.source.clone(),
                    u.model.clone(),
                    u.color.clone(),
                    u.orientation,
                    slug(&u.bezel),
                ),
                u.targets.clone(),
            )
        })
        .collect();
    let sources = vendor_sources(index);
    if all_sizes {
        let slots: BTreeSet<(String, String, String, Orientation)> = wanted
            .keys()
            .map(|(s, m, c, o, _)| (s.clone(), m.clone(), c.clone(), *o))
            .collect();
        for source in &sources {
            for f in index.frames(source).iter().chain(index.shadowed(source)) {
                let slot = (
                    source.clone(),
                    f.model.clone(),
                    f.color.clone(),
                    f.orientation,
                );
                if slots.contains(&slot) {
                    wanted.entry(key_of(source, f)).or_default();
                }
            }
        }
    }
    let mut items = Vec::new();
    let mut bytes_now = 0;
    let mut bytes_after = 0;
    for source in &sources {
        let stored = index.project_frames(source);
        let app = index.app_frames(source);
        for p in &stored {
            bytes_now += frame_bytes(index, source, p);
        }
        for (k, targets) in wanted.iter().filter(|(k, _)| &k.0 == source) {
            let p = stored.iter().find(|f| key_of(source, f) == *k);
            let a = app.iter().find(|f| key_of(source, f) == *k);
            let (action, f) = match (p, a) {
                (None, Some(a)) => (EmbedAction::Add, *a),
                (Some(p), Some(a)) if differs(index, source, p, a) => (EmbedAction::Update, *a),
                (Some(p), _) => (EmbedAction::Keep, *p),
                (None, None) => continue,
            };
            let bytes = frame_bytes(index, source, f);
            bytes_after += bytes;
            items.push(EmbedItem {
                source: source.clone(),
                model: f.model.clone(),
                color: f.color.clone(),
                color_label: f.color_label.clone(),
                orientation: f.orientation,
                bezel: f.apple_model.clone(),
                action,
                bytes,
                targets: targets.clone(),
                file: f.file.clone(),
                origin: f.origin,
            });
        }
        for p in stored
            .iter()
            .filter(|f| !wanted.contains_key(&key_of(source, f)))
        {
            items.push(EmbedItem {
                source: source.clone(),
                model: p.model.clone(),
                color: p.color.clone(),
                color_label: p.color_label.clone(),
                orientation: p.orientation,
                bezel: p.apple_model.clone(),
                action: EmbedAction::Remove,
                bytes: frame_bytes(index, source, p),
                targets: Vec::new(),
                file: p.file.clone(),
                origin: FrameOrigin::Project,
            });
        }
    }
    let up_to_date = items.iter().all(|i| i.action == EmbedAction::Keep);
    let (licenses, licenses_missing) = licence_files(index, project_dir, &items);
    EmbedPlan {
        dir: embed_root(project_dir),
        stored: has_copies(project_dir),
        all_sizes,
        items,
        missing: usage.missing,
        bytes_now,
        bytes_after,
        licenses,
        licenses_missing,
        up_to_date,
        applied: false,
    }
}

/// The frame of `index` an item refers to.
fn item_frame<'a>(index: &'a FrameIndex, i: &EmbedItem) -> Option<&'a InstalledFrame> {
    index
        .frames(&i.source)
        .iter()
        .chain(index.shadowed(&i.source))
        .find(|f| f.file == i.file && f.origin == i.origin)
}

/// Licence files the stored frames need: `(available, missing)`; available
/// as paths relative to the embed root, missing as package names.
fn licence_files(
    index: &FrameIndex,
    project_dir: &Path,
    items: &[EmbedItem],
) -> (Vec<String>, Vec<String>) {
    let root = embed_root(project_dir);
    let project_store = FrameStore::new(&root);
    let app_store = FrameStore::new(index.root());
    let mut have = BTreeSet::new();
    let mut missing = BTreeSet::new();
    for i in items.iter().filter(|i| i.action != EmbedAction::Remove) {
        let Some(f) = item_frame(index, i) else {
            continue;
        };
        let Some(name) = &f.license else {
            continue;
        };
        let in_project = project_store.license_path(&i.source, &f.package, name);
        let in_app = app_store.license_path(&i.source, &f.package, name);
        match (in_project, in_app) {
            (Some(p), _) if p.is_file() => {
                have.insert(rel_display(&root, &p));
            }
            (Some(p), Some(a)) if a.is_file() => {
                have.insert(rel_display(&root, &p));
            }
            _ => {
                missing.insert(f.package.clone());
            }
        }
    }
    (have.into_iter().collect(), missing.into_iter().collect())
}

fn rel_display(base: &Path, p: &Path) -> String {
    p.strip_prefix(base)
        .unwrap_or(p)
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

/// Stores the used frames in the project (see [`plan`]): copies new and
/// re-imported bezels from this computer, removes unused copies, writes
/// the licence notice, `embed.yaml` and the packages' licence files.
/// Removes the folder when nothing is left. Returns the plan with
/// `applied: true`.
pub fn apply(
    project: &Project,
    project_dir: &Path,
    index: &FrameIndex,
    assets: &dyn AssetLookup,
    all_sizes: bool,
) -> Result<EmbedPlan, String> {
    let mut plan = plan(project, project_dir, index, assets, all_sizes);
    let root = embed_root(project_dir);
    let store = FrameStore::new(&root);
    let app_store = FrameStore::new(index.root());
    for i in &plan.items {
        match i.action {
            EmbedAction::Add | EmbedAction::Update => {
                let Some(f) = item_frame(index, i) else {
                    continue;
                };
                let dir = index.source_dir(&i.source);
                let bezel =
                    fs::read(rel_path(&dir, &f.file)).map_err(|e| format!("{}: {e}", f.file))?;
                let mask =
                    fs::read(rel_path(&dir, &f.mask)).map_err(|e| format!("{}: {e}", f.mask))?;
                store.install(&i.source, f.clone(), &bezel, &mask)?;
                if let Some(name) = &f.license
                    && let Some(src) = app_store.license_path(&i.source, &f.package, name)
                    && let Ok(bytes) = fs::read(src)
                {
                    store.save_license(&i.source, &f.package, name, &bytes)?;
                }
            }
            EmbedAction::Remove => {
                store.remove(
                    &i.source,
                    &RemoveFilter {
                        model: Some(&i.model),
                        color: Some(&i.color),
                        orientation: Some(i.orientation),
                        bezel: Some(&i.bezel),
                    },
                )?;
            }
            EmbedAction::Keep => {}
        }
    }
    let kept = plan.items.iter().any(|i| i.action != EmbedAction::Remove);
    if kept {
        write_atomic(&root.join(README_FILE), readme().as_bytes()).map_err(|e| e.to_string())?;
        let yaml = to_yaml(&EmbedOptions { all_sizes }).map_err(|e| e.to_string())?;
        write_atomic(&root.join(OPTIONS_FILE), yaml.as_bytes()).map_err(|e| e.to_string())?;
        prune_licences(&store, &root);
    } else {
        remove(project_dir)?;
    }
    plan.applied = true;
    plan.stored = has_copies(project_dir);
    Ok(plan)
}

fn package_dir(package: &str) -> String {
    match slug(package) {
        s if s.is_empty() => "package".to_owned(),
        s => s,
    }
}

/// Drops `licenses/<package>/` folders no stored frame refers to.
fn prune_licences(store: &FrameStore, root: &Path) {
    for e in fs::read_dir(root).into_iter().flatten().flatten() {
        let source = e.file_name().to_string_lossy().into_owned();
        if !e.path().join(INDEX_FILE).is_file() {
            continue;
        }
        let Ok(ix) = store.source_index(&source) else {
            continue;
        };
        let keep: BTreeSet<String> = ix
            .frames
            .iter()
            .filter(|f| f.license.is_some())
            .map(|f| package_dir(&f.package))
            .collect();
        let dir = store.source_dir(&source).join(super::store::LICENSES_DIR);
        for p in fs::read_dir(&dir).into_iter().flatten().flatten() {
            if !keep.contains(&*p.file_name().to_string_lossy()) {
                let _ = fs::remove_dir_all(p.path());
            }
        }
        let _ = fs::remove_dir(&dir);
    }
}

/// Removes the stored frames (the whole [`embed_root`], and `frames/` when
/// it is empty then). Returns the bytes freed.
pub fn remove(project_dir: &Path) -> Result<u64, String> {
    let root = embed_root(project_dir);
    if !root.exists() {
        return Ok(0);
    }
    let bytes = dir_bytes(&root);
    fs::remove_dir_all(&root).map_err(|e| format!("{}: {e}", root.display()))?;
    let _ = fs::remove_dir(project_dir.join(PROJECT_FRAMES_DIR));
    Ok(bytes)
}

/// Total size of the files below `dir`.
pub fn dir_bytes(dir: &Path) -> u64 {
    fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| {
            let p = e.path();
            if p.is_dir() {
                dir_bytes(&p)
            } else {
                file_len(&p)
            }
        })
        .sum()
}

/// The licence notice written into the folder.
pub fn readme() -> String {
    format!(
        "# Device frames stored in this project\n\
         \n\
         Vitrine Studio copied the imported device frames this project uses into\n\
         this folder (\"Store used device frames in the project\"), so that\n\
         `vitrine export`, `render` and `validate` draw the same bezels on a build\n\
         server (CI) as on the computer they were imported on.\n\
         \n\
         **Licence.** These files are Apple Product Bezels from the Apple Design\n\
         Resources, downloaded and imported by a user of this project. They remain\n\
         subject to Apple's licence: the package's licence file is kept in\n\
         `apple/licenses/<package>/` when the package was imported with one; the\n\
         licence is also available at\n\
         <{APPLE_LICENSE_URL}>.\n\
         It permits using the bezels in marketing mock-ups of apps for Apple\n\
         platforms, but not redistributing them.\n\
         \n\
         - Keep this project in a **private** repository.\n\
         - Do **not** publish it with this folder: no public repository, no\n  \
         template, no gallery upload, no archive shared outside your team.\n  \
         Vitrine Studio templates never contain this folder.\n\
         - For a public repository, add `frames/imported/` to `.gitignore` (a\n  \
         build server then draws the built-in parametric frames) or remove the\n  \
         copies: `vitrine frames unembed`.\n\
         \n\
         Update or remove the copies in Project settings → Frames, or with\n\
         `vitrine frames embed` / `vitrine frames unembed`. This folder is\n\
         generated; do not edit it by hand.\n"
    )
}

/// Identity of a drawn frame (source, model, colour, orientation, vendor
/// model slug), see [`unused_copies`].
pub type UseKey = (String, String, String, Orientation, String);

/// [`UseKey`] of a resolved device's imported frame.
pub fn use_key(model: &str, orientation: Orientation, f: &crate::resolve::ImportedFrame) -> UseKey {
    (
        f.source.clone(),
        model.to_owned(),
        f.color.clone(),
        orientation,
        slug(&f.bezel),
    )
}

/// Frames stored in the project but no longer drawn (validation info):
/// with `all_sizes` ([`EmbedOptions`]), another stored size of a used
/// model × colour × orientation counts as used.
pub fn unused_copies<'a>(
    index: &'a FrameIndex,
    used: &BTreeSet<UseKey>,
) -> Vec<(String, &'a InstalledFrame)> {
    let all = index.project_all_sizes();
    let mut out = Vec::new();
    for source in vendor_sources(index) {
        for f in index.project_frames(&source) {
            let k = key_of(&source, f);
            let slot_used = used
                .iter()
                .any(|u| u.0 == k.0 && u.1 == k.1 && u.2 == k.2 && u.3 == k.3);
            if !(used.contains(&k) || (all && slot_used)) {
                out.push((source.clone(), f));
            }
        }
    }
    out
}

/// Stored frames whose app-wide copy on this computer differs (re-imported
/// since they were stored).
pub fn outdated_copies(index: &FrameIndex) -> Vec<(String, &InstalledFrame)> {
    let mut out = Vec::new();
    for source in vendor_sources(index) {
        for p in index.project_frames(&source) {
            if index
                .shadowed(&source)
                .iter()
                .any(|a| a.same_slot(p) && differs(index, &source, p, a))
            {
                out.push((source.clone(), p));
            }
        }
    }
    out
}
