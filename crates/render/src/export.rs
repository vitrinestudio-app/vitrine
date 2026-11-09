//! Export (KONZEPT §6.3): page images, banners and store gallery previews
//! for every target × language, in parallel (rayon).
//!
//! File names below the export folder (default `<project>/export`):
//!
//! | kind | path |
//! |---|---|
//! | page image | `<target>/<lang>/<nn>_<page-id>.<ext>`; `nn` = 1-based image number, two digits |
//! | grouped page image | `<target>/<lang>/<nn>_<id1>+<id2>.<ext>` (ids of all pages in the image) |
//! | banner | `<target>/<lang>/banner.<ext>` |
//! | gallery preview | `<target>/<lang>/_gallery.png` (pages targets only) |
//!
//! `ext` is `png` (24-bit RGB, no alpha, sRGB chunk) or `jpg` for targets
//! with `format: jpeg` (`quality`, default 90). Alpha is flattened onto
//! white; pages are opaque anyway because the canvas background covers them.
//!
//! **Incremental** (§6.3): every file gets a hash of the resolved scene,
//! the image index, the output format, the mtimes of all assets the scene
//! references (screenshots, images, project fonts) and the renderer version.
//! Hashes are kept in `<export>/.arrangia-export.json`; an existing file
//! with an unchanged hash is not rendered again.

use std::collections::{BTreeMap, BTreeSet};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Instant;

use arrangia_core::Project;
use arrangia_core::model::{ImageFormat, TargetKind};
use arrangia_core::resolve::{
    Diagnostic, FsAssets, ResolvedFill, ResolvedKind, ResolvedScene, resolve,
};
use rayon::prelude::*;
use serde::Serialize;
use tiny_skia::{FillRule, FilterQuality, Pattern, Pixmap, SpreadMode, Stroke, Transform};

use crate::paint::{round_rect_xy, shader_paint, solid};
use crate::raster::{encode_jpeg, encode_png_rgb_with};
use crate::scene::{PreparedScene, Quality, RenderContext, RenderIssue, RenderOptions, Scale};

/// Background pages are flattened onto (alpha is removed).
const FLATTEN_ON: [u8; 3] = [255, 255, 255];
/// Gallery preview background when the profile has none.
const GALLERY_BACKGROUND: [u8; 4] = [0xF2, 0xF2, 0xF7, 0xFF];
/// Name of the incremental-export manifest in the export folder.
pub const MANIFEST: &str = ".arrangia-export.json";
/// Bump when the renderer's output changes, to invalidate incremental
/// exports made by an older build.
pub const RENDER_REVISION: u32 = 2;
/// PNG compression of exported files (High costs ~2× the time for ~5 %).
const PNG_COMPRESSION: png::Compression = png::Compression::Balanced;

/// What to export.
#[derive(Debug, Clone)]
pub struct ExportOptions {
    /// Export folder; default `<project root>/export`.
    pub out_dir: Option<PathBuf>,
    /// Target ids; empty = all targets.
    pub targets: Vec<String>,
    /// Languages; empty = each target's `langs` or all available languages.
    pub langs: Vec<String>,
    /// Write `_gallery.png` previews for pages targets.
    pub gallery: bool,
    /// Gallery preview height limit in px.
    pub gallery_height: u32,
    /// Skip files whose inputs did not change.
    pub incremental: bool,
    /// Progress reporting and cancellation (GUI).
    pub hooks: ExportHooks,
}

/// Called after every file (written, skipped or failed) with the file,
/// the number of files done and the total number of files. Runs on the
/// export's worker threads.
pub type ProgressFn = dyn Fn(&ExportedFile, usize, usize) + Send + Sync;

/// Optional callbacks of an export.
#[derive(Clone, Default)]
pub struct ExportHooks {
    pub progress: Option<Arc<ProgressFn>>,
    /// When set to `true`, files not started yet are not exported; the
    /// report has `cancelled: true`.
    pub cancel: Option<Arc<AtomicBool>>,
}

impl std::fmt::Debug for ExportHooks {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExportHooks")
            .field("progress", &self.progress.is_some())
            .field("cancel", &self.cancel)
            .finish()
    }
}

impl ExportHooks {
    fn cancelled(&self) -> bool {
        self.cancel
            .as_ref()
            .is_some_and(|c| c.load(Ordering::Relaxed))
    }
}

impl Default for ExportOptions {
    fn default() -> Self {
        Self {
            out_dir: None,
            targets: Vec::new(),
            langs: Vec::new(),
            gallery: true,
            gallery_height: 1200,
            incremental: true,
            hooks: ExportHooks::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FileKind {
    Page,
    Banner,
    Gallery,
}

/// One written (or skipped) file.
#[derive(Debug, Clone, Serialize)]
pub struct ExportedFile {
    /// Path relative to the export folder (`/`-separated).
    pub path: String,
    pub target: String,
    pub lang: String,
    pub kind: FileKind,
    /// Output image index (pages and banner).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<u32>,
    pub width: u32,
    pub height: u32,
    pub bytes: u64,
    /// Unchanged since the last export; not rendered.
    pub skipped: bool,
    pub render_ms: f64,
    pub encode_ms: f64,
}

/// Where an issue comes from.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "source", rename_all = "snake_case")]
pub enum IssueSource {
    Resolve(Diagnostic),
    Render(RenderIssue),
}

#[derive(Debug, Clone, Serialize)]
pub struct ExportIssue {
    pub target: String,
    pub lang: String,
    #[serde(flatten)]
    pub source: IssueSource,
}

/// Result of [`export`].
#[derive(Debug, Clone, Default, Serialize)]
pub struct ExportReport {
    /// Export folder.
    pub out_dir: PathBuf,
    /// Files in path order.
    pub files: Vec<ExportedFile>,
    /// Resolve diagnostics, text layout and asset issues (deduplicated per
    /// target × language).
    pub issues: Vec<ExportIssue>,
    /// Failures (unknown target, resolve error, write error).
    pub errors: Vec<String>,
    pub resolve_ms: f64,
    pub total_ms: f64,
    /// Cancelled via [`ExportHooks::cancel`]; `files` holds what was done.
    pub cancelled: bool,
    #[serde(skip)]
    seen: BTreeSet<(String, String, String)>,
}

impl ExportReport {
    fn push_issue(&mut self, target: &str, lang: &str, source: IssueSource) {
        let key = (
            target.to_owned(),
            lang.to_owned(),
            serde_json::to_string(&source).unwrap_or_default(),
        );
        if self.seen.insert(key) {
            self.issues.push(ExportIssue {
                target: target.to_owned(),
                lang: lang.to_owned(),
                source,
            });
        }
    }

    pub fn written(&self) -> impl Iterator<Item = &ExportedFile> {
        self.files.iter().filter(|f| !f.skipped)
    }
}

fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1000.0
}

struct Job {
    target: String,
    lang: String,
    format: ImageFormat,
    quality: u8,
    scene: ResolvedScene,
    hash: u64,
}

enum Task {
    Image(u32),
    Gallery,
}

struct Output {
    file: ExportedFile,
    hash: u64,
    issues: Vec<RenderIssue>,
    error: Option<String>,
}

/// Exports `project` (folder = `ctx.root()`).
pub fn export(project: &Project, ctx: &RenderContext, opts: &ExportOptions) -> ExportReport {
    let start = Instant::now();
    let root = ctx.root().to_path_buf();
    let out_dir = opts.out_dir.clone().unwrap_or_else(|| root.join("export"));
    let mut report = ExportReport {
        out_dir: out_dir.clone(),
        ..ExportReport::default()
    };
    for t in &opts.targets {
        if project.target(t).is_none() {
            report.errors.push(format!("unknown target `{t}`"));
        }
    }
    let pairs = pairs(project, opts);

    // 1. Resolve all target × language pairs.
    let t = Instant::now();
    let fonts_state = fonts_state(&root);
    let fs = FsAssets::new(&root);
    let resolved: Vec<Result<Job, String>> = pairs
        .par_iter()
        .map(|(target, lang)| job(project, ctx, &fs, &fonts_state, target, lang))
        .collect();
    let mut jobs = Vec::new();
    for r in resolved {
        match r {
            Ok(j) => jobs.push(j),
            Err(e) => report.errors.push(e),
        }
    }
    let prepared: Vec<PreparedScene<'_>> = jobs.par_iter().map(|j| ctx.prepare(&j.scene)).collect();
    report.resolve_ms = ms(t);

    // 2. Render, encode and write every file in parallel.
    let manifest_path = out_dir.join(MANIFEST);
    let old: BTreeMap<String, String> = if opts.incremental {
        std::fs::read(&manifest_path)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    } else {
        BTreeMap::new()
    };
    let mut tasks: Vec<(usize, Task)> = Vec::new();
    for (i, j) in jobs.iter().enumerate() {
        for img in &j.scene.frame.images {
            tasks.push((i, Task::Image(img.index)));
        }
        if opts.gallery && j.scene.frame.kind == TargetKind::Pages {
            tasks.push((i, Task::Gallery));
        }
    }
    let outputs = run_tasks(ctx, &jobs, &prepared, &tasks, &out_dir, &old, opts);
    if outputs.len() < tasks.len() {
        report.cancelled = true;
        report.errors.push(format!(
            "export cancelled after {} of {} files",
            outputs.len(),
            tasks.len()
        ));
    }

    // 3. Collect.
    let mut manifest = old;
    for (j, p) in jobs.iter().zip(&prepared) {
        for d in &j.scene.diagnostics {
            report.push_issue(&j.target, &j.lang, IssueSource::Resolve(d.clone()));
        }
        for d in &p.issues {
            report.push_issue(&j.target, &j.lang, IssueSource::Render(d.clone()));
        }
    }
    for o in outputs {
        for d in o.issues {
            report.push_issue(&o.file.target, &o.file.lang, IssueSource::Render(d));
        }
        if let Some(e) = o.error {
            manifest.remove(&o.file.path);
            report.errors.push(e);
        } else {
            manifest.insert(o.file.path.clone(), format!("{:016x}", o.hash));
            report.files.push(o.file);
        }
    }
    report.files.sort_by(|a, b| a.path.cmp(&b.path));
    if let Err(e) = std::fs::create_dir_all(&out_dir).and_then(|()| {
        std::fs::write(
            &manifest_path,
            serde_json::to_vec_pretty(&manifest).unwrap_or_default(),
        )
    }) {
        report
            .errors
            .push(format!("cannot write {}: {e}", manifest_path.display()));
    }
    report.total_ms = ms(start);
    report
}

/// Runs all tasks in parallel, with progress reports; stops starting new
/// tasks once cancelled.
fn run_tasks(
    ctx: &RenderContext,
    jobs: &[Job],
    prepared: &[PreparedScene<'_>],
    tasks: &[(usize, Task)],
    out_dir: &Path,
    old: &BTreeMap<String, String>,
    opts: &ExportOptions,
) -> Vec<Output> {
    let total = tasks.len();
    let done = AtomicUsize::new(0);
    tasks
        .par_iter()
        .filter_map(|(i, task)| {
            if opts.hooks.cancelled() {
                return None;
            }
            let o = run_task(ctx, &jobs[*i], &prepared[*i], task, out_dir, old, opts);
            if let Some(progress) = &opts.hooks.progress {
                progress(&o.file, done.fetch_add(1, Ordering::Relaxed) + 1, total);
            }
            Some(o)
        })
        .collect()
}

fn job(
    project: &Project,
    ctx: &RenderContext,
    fs: &FsAssets,
    fonts_state: &[(String, Option<std::time::SystemTime>)],
    target: &str,
    lang: &str,
) -> Result<Job, String> {
    let scene = resolve(project, target, lang, &ctx.resolve_options(fs))
        .map_err(|e| format!("{target}/{lang}: {e}"))?;
    let tgt = project.target(target).expect("filtered");
    let format = tgt.format.unwrap_or(ImageFormat::Png);
    let quality = tgt.quality.unwrap_or(90);
    let hash = scene_hash(&scene, ctx, fonts_state, format, quality);
    Ok(Job {
        target: target.to_owned(),
        lang: lang.to_owned(),
        format,
        quality,
        scene,
        hash,
    })
}

/// Target × language pairs to export.
fn pairs(project: &Project, opts: &ExportOptions) -> Vec<(String, String)> {
    project
        .targets
        .iter()
        .filter(|t| opts.targets.is_empty() || opts.targets.iter().any(|x| x == t.id.as_str()))
        .flat_map(|t| {
            let langs: Vec<String> = t.langs.as_ref().map_or_else(
                || {
                    project
                        .languages
                        .available
                        .iter()
                        .map(|l| l.as_str().to_owned())
                        .collect()
                },
                |l| l.iter().map(|l| l.as_str().to_owned()).collect(),
            );
            langs
                .into_iter()
                .filter(|l| opts.langs.is_empty() || opts.langs.contains(l))
                .map(|l| (t.id.as_str().to_owned(), l))
                .collect::<Vec<_>>()
        })
        .collect()
}

/// File name of an output image (see module docs).
pub fn image_file_name(scene: &ResolvedScene, image: u32, format: ImageFormat) -> String {
    let ext = match format {
        ImageFormat::Png => "png",
        ImageFormat::Jpeg => "jpg",
    };
    if scene.frame.kind == TargetKind::Banner {
        return format!("banner.{ext}");
    }
    let img = &scene.frame.images[image as usize];
    let ids: Vec<&str> = img
        .pages
        .iter()
        .filter_map(|&p| scene.frame.pages.get(p as usize).map(|p| p.id.as_str()))
        .collect();
    format!("{:02}_{}.{ext}", image + 1, ids.join("+"))
}

fn run_task(
    ctx: &RenderContext,
    job: &Job,
    prepared: &PreparedScene<'_>,
    task: &Task,
    out_dir: &Path,
    old: &BTreeMap<String, String>,
    opts: &ExportOptions,
) -> Output {
    let (name, kind, image, format) = match task {
        Task::Image(i) => {
            let kind = if job.scene.frame.kind == TargetKind::Banner {
                FileKind::Banner
            } else {
                FileKind::Page
            };
            (
                image_file_name(&job.scene, *i, job.format),
                kind,
                Some(*i),
                job.format,
            )
        }
        Task::Gallery => (
            "_gallery.png".to_owned(),
            FileKind::Gallery,
            None,
            ImageFormat::Png,
        ),
    };
    let rel = format!("{}/{}/{name}", job.target, job.lang);
    let path = out_dir.join(&rel);
    let mut h = DefaultHasher::new();
    job.hash.hash(&mut h);
    image.hash(&mut h);
    opts.gallery_height.hash(&mut h);
    let hash = h.finish();
    let mut file = ExportedFile {
        path: rel.clone(),
        target: job.target.clone(),
        lang: job.lang.clone(),
        kind,
        image,
        width: 0,
        height: 0,
        bytes: 0,
        skipped: false,
        render_ms: 0.0,
        encode_ms: 0.0,
    };
    if opts.incremental
        && old.get(&rel) == Some(&format!("{hash:016x}"))
        && let Ok(meta) = std::fs::metadata(&path)
    {
        file.skipped = true;
        file.bytes = meta.len();
        if let Some((w, h)) = image_size(&path) {
            file.width = w;
            file.height = h;
        }
        return Output {
            file,
            hash,
            issues: Vec::new(),
            error: None,
        };
    }
    let t = Instant::now();
    let (pixmap, issues) = match task {
        Task::Image(i) => {
            let r = ctx.rasterize_prepared(prepared, *i as usize, &RenderOptions::export());
            (r.pixmap, r.issues)
        }
        Task::Gallery => gallery(ctx, prepared, opts.gallery_height),
    };
    file.render_ms = ms(t);
    file.width = pixmap.width();
    file.height = pixmap.height();
    let t = Instant::now();
    let encoded = match format {
        ImageFormat::Png => encode_png_rgb_with(&pixmap, FLATTEN_ON, PNG_COMPRESSION),
        ImageFormat::Jpeg => encode_jpeg(&pixmap, FLATTEN_ON, job.quality),
    };
    file.encode_ms = ms(t);
    let error = match encoded {
        Ok(bytes) => {
            file.bytes = bytes.len() as u64;
            path.parent()
                .map_or(Ok(()), std::fs::create_dir_all)
                .and_then(|()| std::fs::write(&path, &bytes))
                .err()
                .map(|e| format!("cannot write {}: {e}", path.display()))
        }
        Err(e) => Some(format!("cannot encode {rel}: {e}")),
    };
    Output {
        file,
        hash,
        issues,
        error,
    }
}

fn image_size(path: &Path) -> Option<(u32, u32)> {
    image::ImageReader::open(path)
        .ok()?
        .with_guessed_format()
        .ok()?
        .into_dimensions()
        .ok()
}

/// Store gallery preview: the output images side by side at their canvas
/// positions (so content crossing a gap lines up as in the store), with the
/// gallery profile's gap, corner radius and background. At most `max_height`
/// px high (never upscaled).
pub fn gallery(
    ctx: &RenderContext,
    prepared: &PreparedScene<'_>,
    max_height: u32,
) -> (Pixmap, Vec<RenderIssue>) {
    let frame = &prepared.scene.frame;
    let (Some(first), Some(last)) = (frame.images.first(), frame.images.last()) else {
        return (Pixmap::new(1, 1).expect("1x1"), Vec::new());
    };
    let factor = (f64::from(max_height) / f64::from(frame.size.h)).min(1.0);
    let px_per_lu = first.viewport.scale * factor;
    let info = frame.gallery.as_ref();
    let gap_px = info.map_or(0.0, |g| g.gap * px_per_lu);
    let pad = gap_px.max(24.0).round() as u32;
    let (rx, ry) = info.map_or((0.0, 0.0), |g| {
        (
            (g.radius.x * px_per_lu) as f32,
            (g.radius.y * px_per_lu) as f32,
        )
    });
    let bg = info
        .and_then(|g| g.background)
        .map_or(GALLERY_BACKGROUND, |c| [c.r, c.g, c.b, 255]);
    let opts = RenderOptions {
        scale: Scale::Factor(factor),
        quality: Quality::Export,
    };
    let renders: Vec<_> = (0..frame.images.len())
        .map(|i| ctx.rasterize_prepared(prepared, i, &opts))
        .collect();
    let ih = renders[0].pixmap.height();
    let width = ((last.source.right() - first.source.x) * px_per_lu).round() as u32 + 2 * pad;
    let mut pm = Pixmap::new(width.max(1), ih + 2 * pad).expect("gallery size");
    pm.fill(tiny_skia::Color::from_rgba8(bg[0], bg[1], bg[2], bg[3]));
    let mut issues = Vec::new();
    for (img, r) in frame.images.iter().zip(renders) {
        let x = pad as f32 + ((img.source.x - first.source.x) * px_per_lu) as f32;
        let y = pad as f32;
        let (w, h) = (r.pixmap.width() as f32, r.pixmap.height() as f32);
        let Some(path) = round_rect_xy(x, y, w, h, rx, ry) else {
            continue;
        };
        pm.stroke_path(
            &path,
            &solid([0, 0, 0, 30]),
            &Stroke {
                width: 1.0,
                ..Stroke::default()
            },
            Transform::identity(),
            None,
        );
        let pat = Pattern::new(
            r.pixmap.as_ref(),
            SpreadMode::Pad,
            FilterQuality::Nearest,
            1.0,
            Transform::from_translate(x, y),
        );
        pm.fill_path(
            &path,
            &shader_paint(pat),
            FillRule::Winding,
            Transform::identity(),
            None,
        );
        issues.extend(r.issues);
    }
    (pm, issues)
}

/// mtimes of the project fonts (they affect text layout).
fn fonts_state(root: &Path) -> Vec<(String, Option<std::time::SystemTime>)> {
    let mut v: Vec<_> = std::fs::read_dir(root.join("fonts"))
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| {
            (
                e.file_name().to_string_lossy().into_owned(),
                e.metadata().and_then(|m| m.modified()).ok(),
            )
        })
        .collect();
    v.sort();
    v
}

/// Assets referenced by a scene (project-relative paths).
pub fn scene_assets(scene: &ResolvedScene) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let fill = |f: &ResolvedFill, out: &mut BTreeSet<String>| {
        if let ResolvedFill::Image { asset, .. } = f {
            out.insert(asset.clone());
        }
    };
    for f in &scene.backgrounds {
        fill(f, &mut out);
    }
    for e in &scene.elements {
        match &e.props {
            ResolvedKind::Background { fill: f } => fill(f, &mut out),
            ResolvedKind::Shape(s) => {
                if let Some(f) = &s.fill {
                    fill(f, &mut out);
                }
            }
            ResolvedKind::Image(i) => {
                out.insert(i.asset.clone());
            }
            ResolvedKind::Device(d) => {
                if let Some(p) = d.screenshot.as_ref().and_then(|s| s.path.clone()) {
                    out.insert(p);
                }
            }
            ResolvedKind::Text(_) => {}
        }
    }
    out
}

fn scene_hash(
    scene: &ResolvedScene,
    ctx: &RenderContext,
    fonts: &[(String, Option<std::time::SystemTime>)],
    format: ImageFormat,
    quality: u8,
) -> u64 {
    let mut h = DefaultHasher::new();
    env!("CARGO_PKG_VERSION").hash(&mut h);
    RENDER_REVISION.hash(&mut h);
    serde_json::to_vec(scene).unwrap_or_default().hash(&mut h);
    // Catalog data is not serialized with the scene.
    for e in &scene.elements {
        if let ResolvedKind::Device(d) = &e.props {
            format!("{:?}{:?}", d.device, d.palette).hash(&mut h);
        }
    }
    for a in scene_assets(scene) {
        ctx.assets().mtime(&a).hash(&mut h);
        a.hash(&mut h);
    }
    fonts.hash(&mut h);
    format.hash(&mut h);
    quality.hash(&mut h);
    h.finish()
}
