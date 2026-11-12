//! Templates (KONZEPT §9, phase 6.3, ADR 0023): the app-wide store
//! `<app data>/templates/`, saving canvases of the open project as a
//! template, importing/exporting `.vitrine-template` files, inserting
//! canvases from a template into the open project (one undo step) and
//! preview images. New projects from a template: `project_new`.

use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use vitrine_core::template::extract::{SaveOptions, SaveReport, extract};
use vitrine_core::template::insert::{InsertOptions, InsertReport, StyleConflict, plan_insert};
use vitrine_core::template::store::TemplateStore;
use vitrine_core::template::tags::{Taxonomy, taxonomy};
use vitrine_core::template::{EXTENSION, Limits, Package, TEMPLATES_DIR, TemplateSummary};
use vitrine_render::template::{PREVIEW_HEIGHT, add_previews};

use super::{AppState, NoArgs, absolute};
use crate::error::{CmdResult, CommandError, ErrorCode};
use crate::preview::{PixelFormat, PreviewImage};
use crate::session::ApplyResult;

impl AppState {
    /// The app-wide template store.
    pub fn templates(&self) -> TemplateStore {
        TemplateStore::new(self.paths().data_dir.join(TEMPLATES_DIR))
    }
}

fn generator() -> String {
    format!("Vitrine Studio {}", env!("CARGO_PKG_VERSION"))
}

/// Built-in and installed templates.
pub fn template_list(app: &AppState, _: NoArgs) -> CmdResult<Vec<TemplateSummary>> {
    Ok(app.templates().list())
}

/// The template tag taxonomy (`crates/core/data/template-tags.yaml`):
/// facets `style`, `color`, `use` (set by the author, with min/max) and the
/// derived `devices`, each value with English and German labels.
pub fn template_tags(_: &AppState, _: NoArgs) -> CmdResult<Taxonomy> {
    Ok(taxonomy().clone())
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TemplateSaveArgs {
    #[serde(flatten)]
    pub options: SaveOptions,
    /// Replace an installed template with the same id.
    #[serde(default)]
    pub replace: bool,
    /// Also write the package to this `.vitrine-template` file.
    #[serde(default)]
    pub file: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TemplateSaveResult {
    pub template: TemplateSummary,
    pub report: SaveReport,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<PathBuf>,
}

/// Saves canvases of the open project (default: all) as a template in the
/// store, with rendered previews; optionally also as a file.
pub fn template_save(app: &AppState, args: TemplateSaveArgs) -> CmdResult<TemplateSaveResult> {
    let session = app.session()?;
    if args.options.name.trim().is_empty() {
        return Err(CommandError::invalid("a template needs a name"));
    }
    let project = session.snapshot();
    let (mut pkg, report) = extract(&project, session.dir(), &args.options)?;
    pkg.manifest.generator = Some(generator());
    add_previews(&mut pkg, PREVIEW_HEIGHT)
        .map_err(|e| CommandError::new(ErrorCode::Render, format!("{e:#}")))?;
    let template = app.templates().install(&pkg, args.replace)?;
    let file = match args.file {
        Some(f) => {
            let f = with_extension(absolute(&f)?);
            pkg.write_zip(&f)?;
            Some(f)
        }
        None => None,
    };
    Ok(TemplateSaveResult {
        template,
        report,
        file,
    })
}

fn with_extension(p: PathBuf) -> PathBuf {
    if p.extension().is_some_and(|e| e == EXTENSION) {
        p
    } else {
        let mut s = p.into_os_string();
        s.push(".");
        s.push(EXTENSION);
        PathBuf::from(s)
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TemplateImportArgs {
    /// `.vitrine-template` file (or an extracted template folder).
    pub path: PathBuf,
    #[serde(default)]
    pub replace: bool,
}

/// Checks a template file and installs it into the store.
pub fn template_import(app: &AppState, args: TemplateImportArgs) -> CmdResult<TemplateSummary> {
    let pkg = Package::open(&absolute(&args.path)?, Limits::default())?;
    Ok(app.templates().install(&pkg, args.replace)?)
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TemplateExportArgs {
    /// Template id (installed or built in).
    pub id: String,
    /// Target file (`.vitrine-template` is appended when missing).
    pub path: PathBuf,
}

#[derive(Debug, Clone, Serialize)]
pub struct TemplateExportResult {
    pub path: PathBuf,
    pub bytes: u64,
}

/// Writes a template as a `.vitrine-template` file (e.g. for the gallery).
pub fn template_export(
    app: &AppState,
    args: TemplateExportArgs,
) -> CmdResult<TemplateExportResult> {
    let mut pkg = app.templates().find(&args.id)?;
    if pkg.manifest.canvases.iter().any(|c| c.preview.is_none()) {
        add_previews(&mut pkg, PREVIEW_HEIGHT)
            .map_err(|e| CommandError::new(ErrorCode::Render, format!("{e:#}")))?;
    }
    let path = with_extension(absolute(&args.path)?);
    pkg.write_zip(&path)?;
    let bytes = std::fs::metadata(&path).map_or(0, |m| m.len());
    Ok(TemplateExportResult { path, bytes })
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TemplateIdArgs {
    pub id: String,
}

pub fn template_remove(app: &AppState, args: TemplateIdArgs) -> CmdResult<()> {
    Ok(app.templates().remove(&args.id)?)
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TemplateInsertArgs {
    /// Template id (built in or installed) or a template file.
    pub template: String,
    /// Canvases of the template (default: all).
    #[serde(default)]
    pub canvases: Option<Vec<String>>,
    /// Same style name, different definition: `rename` (default) or
    /// `use_project`.
    #[serde(default)]
    pub styles: StyleConflict,
}

impl TemplateInsertArgs {
    fn options(&self) -> InsertOptions {
        InsertOptions {
            canvases: self.canvases.clone(),
            styles: self.styles,
        }
    }
}

/// What inserting would do (nothing changes).
pub fn template_insert_plan(app: &AppState, args: TemplateInsertArgs) -> CmdResult<InsertReport> {
    let session = app.session()?;
    let pkg = app.templates().find(&args.template)?;
    let plan = plan_insert(&session.snapshot(), session.dir(), &pkg, &args.options())?;
    Ok(plan.report)
}

#[derive(Debug, Clone, Serialize)]
pub struct TemplateInsertResult {
    #[serde(flatten)]
    pub result: ApplyResult,
    pub report: InsertReport,
    /// Families of `report.fonts_required` this computer does not have
    /// (bundled, project or installed): their texts use Inter.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub fonts_unavailable: Vec<String>,
}

/// Inserts canvases of a template into the open project: copies assets
/// and fonts, then applies styles, texts, canvases and targets as one undo
/// step.
pub fn template_insert(
    app: &AppState,
    args: TemplateInsertArgs,
) -> CmdResult<TemplateInsertResult> {
    let session = app.session()?;
    let pkg = app.templates().find(&args.template)?;
    let root = session.dir().to_owned();
    let plan = plan_insert(&session.snapshot(), &root, &pkg, &args.options())?;
    let mut written = Vec::new();
    for f in &plan.files {
        let dest = f.to.split('/').fold(root.clone(), |p, c| p.join(c));
        let data = pkg.file(&f.from).unwrap_or_default();
        let res = dest
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|()| std::fs::write(&dest, data));
        if let Err(e) = res {
            remove_all(&written);
            return Err(CommandError::io(format!("{}: {e}", dest.display())));
        }
        written.push(dest);
    }
    let names: Vec<&str> = plan.report.canvases.iter().map(|c| c.to.as_str()).collect();
    let label = format!(
        "Insert {} from template {}",
        names.join(", "),
        pkg.manifest.name
    );
    let marked = vitrine_core::template::insert::mark_samples(&root, &pkg, &plan);
    match session.apply_ops(plan.ops.clone(), Some(label)) {
        Ok(result) => {
            marked?;
            let index = if session.system_fonts() {
                vitrine_core::layout::system::index()
            } else {
                Arc::new(vitrine_core::layout::system::SystemFontIndex::default())
            };
            let fonts_unavailable = vitrine_core::template::unavailable_fonts(
                &plan.report.fonts_required,
                session.render_context().fonts(),
                &index,
            );
            Ok(TemplateInsertResult {
                result,
                report: plan.report,
                fonts_unavailable,
            })
        }
        Err(e) => {
            remove_all(&written);
            Err(e)
        }
    }
}

fn remove_all(files: &[PathBuf]) {
    for f in files {
        let _ = std::fs::remove_file(f);
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TemplatePreviewArgs {
    pub id: String,
    /// Canvas (default: the first).
    #[serde(default)]
    pub canvas: Option<String>,
}

/// Preview image (PNG) of a template canvas: the stored preview, else
/// rendered now (built-in templates have none stored).
pub fn template_preview(app: &AppState, args: TemplatePreviewArgs) -> CmdResult<Arc<PreviewImage>> {
    let pkg = app.templates().find(&args.id)?;
    let canvas = match args.canvas {
        Some(c) => c,
        None => pkg
            .manifest
            .canvases
            .first()
            .map(|c| c.id.clone())
            .ok_or_else(|| CommandError::not_found("the template has no canvas"))?,
    };
    let mut pkg = pkg;
    if pkg.preview(&canvas).is_none() {
        add_previews(&mut pkg, PREVIEW_HEIGHT)
            .map_err(|e| CommandError::new(ErrorCode::Render, format!("{e:#}")))?;
    }
    let png = pkg
        .preview(&canvas)
        .ok_or_else(|| CommandError::not_found(format!("no preview for canvas `{canvas}`")))?
        .to_vec();
    let size = image_size(&png)?;
    Ok(Arc::new(PreviewImage {
        width: size.0,
        height: size.1,
        format: PixelFormat::Png,
        origin_x: 0.0,
        origin_y: 0.0,
        scale: 0.0,
        data: png,
    }))
}

pub(super) fn image_size(png: &[u8]) -> CmdResult<(u32, u32)> {
    let d = png::Decoder::new(std::io::Cursor::new(png));
    let r = d
        .read_info()
        .map_err(|e| CommandError::new(ErrorCode::Render, e.to_string()))?;
    let info = r.info();
    Ok((info.width, info.height))
}
