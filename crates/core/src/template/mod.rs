//! Templates (KONZEPT §9 "Vorlagen", phase 6.3, ADR 0023): a package
//! `.vitrine-template` (zip) holding a subset of a project – one or more
//! canvases with their targets and variants, the styles, the texts of the
//! canvases in every language, generated placeholder screenshots, image
//! assets and OFL fonts with their licence files – plus `template.yaml`
//! (the [`Manifest`]).
//!
//! ```text
//! template.yaml                  manifest (format vitrine-template/1)
//! project.yaml                   a vitrine/1 project (no store/import settings)
//! i18n/<lang>.yaml               texts of the canvases
//! screenshots/<lang>/…           generated placeholders, or the author's own
//!                                sample screenshots listed in `samples`
//! assets/…                       PNG/JPEG/SVG images used by the canvases
//! fonts/<file>.ttf|otf           OFL fonts only, each with fonts/<stem>-OFL.txt
//! previews/<canvas>.png          preview images (rendered by Vitrine Studio)
//! ```
//!
//! Everything a package contains is checked when it is read
//! ([`Package::from_zip_bytes`], [`Package::from_dir`]): size limits, safe
//! relative paths (no traversal, no symlinks, no hidden files), only the
//! file types above (checked by content), fonts that declare the OFL in
//! their name table, and the project subset loading and validating with
//! `core`. Templates are CC0 ([`LICENSE`]); fonts keep their OFL.
//!
//! What a template is used for: [`instantiate::create_project`] (new
//! project from a template), [`insert::plan_insert`] (canvases into an
//! open project, merge rules in ADR 0023), [`extract::extract`] (save
//! canvases or a whole project as template), [`store::TemplateStore`]
//! (app-wide `<app data>/templates/`) and [`builtin`] (`phone`, `minimal`).

mod archive;
pub mod builtin;
pub mod extract;
pub mod fonts;
pub mod insert;
pub mod instantiate;
pub mod placeholder;
pub mod store;
pub mod tags;
mod walk;

#[cfg(test)]
mod tests;

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::io::{I18N_DIR, PROJECT_FILE, from_yaml, project_from_yaml, strings_from_yaml};
use crate::model::{Lang, Project};

pub use archive::Limits;
pub use tags::TemplateTags;

/// `format:` of `template.yaml`.
pub const TEMPLATE_FORMAT: &str = "vitrine-template/1";
/// Manifest file name inside a package.
pub const MANIFEST_FILE: &str = "template.yaml";
/// File extension of a packed template.
pub const EXTENSION: &str = "vitrine-template";
/// Folder of the app-wide template store inside the app data folder.
pub const TEMPLATES_DIR: &str = "templates";
/// Licence of every template (user decision 2026-11-10: no choice).
pub const LICENSE: &str = "CC0-1.0";
/// Licence of the fonts inside a template.
pub const FONT_LICENSE: &str = "OFL-1.1";
/// Largest sample screenshot (bytes).
pub const SAMPLE_MAX_BYTES: usize = 10 * 1024 * 1024;
/// Longest side of a sample screenshot (px).
pub const SAMPLE_MAX_EDGE: u32 = 6000;

/// Why a template could not be read, saved or used.
#[derive(Debug, thiserror::Error)]
pub enum TemplateError {
    /// The package breaks a rule (path, type, size, licence, content).
    #[error("{0}")]
    Invalid(String),
    /// A font may not be redistributed (no OFL declaration).
    #[error(
        "font `{file}` may not be put into a template: {reason} (only fonts under the SIL Open Font License are allowed; save without it to list the family as required but not included: `--skip-unlicensed-fonts`)"
    )]
    FontLicense { file: String, reason: String },
    /// The project subset does not load or validate.
    #[error("the template's project is invalid: {0}")]
    Project(String),
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    Conflict(String),
    #[error("{path}: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },
}

impl TemplateError {
    pub(crate) fn invalid(msg: impl Into<String>) -> Self {
        Self::Invalid(msg.into())
    }

    pub(crate) fn io(path: &std::path::Path) -> impl FnOnce(std::io::Error) -> Self + '_ {
        move |source| Self::Io {
            path: path.display().to_string(),
            source,
        }
    }
}

/// `template.yaml`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    /// Always [`TEMPLATE_FORMAT`].
    pub format: String,
    /// Identifier (`[a-z0-9][a-z0-9-]*`, at most 64 characters); unique in
    /// a store and in the gallery.
    pub id: String,
    /// Display name.
    pub name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub author: String,
    /// Tags from the fixed taxonomy (`style`, `color`, `use`; [`tags`]).
    #[serde(default, skip_serializing_if = "TemplateTags::is_empty")]
    pub tags: TemplateTags,
    /// Version of the template (free form, e.g. `1.0.0`).
    pub version: String,
    /// Always [`LICENSE`] (`CC0-1.0`).
    pub license: String,
    /// Format of `project.yaml` (`vitrine/1`).
    pub project_format: String,
    /// The canvases, in project order.
    pub canvases: Vec<CanvasInfo>,
    /// Fonts shipped with the template (all OFL).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fonts: Vec<FontInfo>,
    /// Families the styles use that are **not** included (licence does not
    /// allow sharing, or not in the project: a system font). A project made
    /// from the template uses them when the user has them installed (system
    /// or project fonts), else falls back to Inter (ADR 0028).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fonts_required: Vec<RequiredFont>,
    /// Sample screenshots: package paths (`screenshots/en/home.png`) of the
    /// author's own app screenshots (CC0), copied into new projects and
    /// marked as samples there; all other screenshots are placeholders.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub samples: Vec<String>,
    /// Program that wrote the package (`Vitrine Studio 0.1.0`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generator: Option<String>,
}

/// One canvas of a template.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CanvasInfo {
    /// Canvas id in `project.yaml`.
    pub id: String,
    /// Page aspect `w:h`.
    pub aspect: String,
    pub pages: u32,
    /// Targets showing this canvas.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub targets: Vec<String>,
    /// Preview image (`previews/<id>.png`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preview: Option<String>,
}

/// A font file of a template.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FontInfo {
    /// `fonts/<file>`.
    pub file: String,
    pub family: String,
    /// Always [`FONT_LICENSE`].
    pub license: String,
    /// `fonts/<stem>-OFL.txt`.
    pub license_file: String,
    /// Copyright line from the font (name ID 0).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub copyright: String,
}

/// Why a font family a template uses is not in the package.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RequiredReason {
    /// The font's licence does not allow sharing it (not OFL).
    License,
    /// Not in the project's `fonts/` (a system font, or missing).
    NotInProject,
}

/// A font family a template uses but does not include.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RequiredFont {
    pub family: String,
    pub reason: RequiredReason,
}

/// Families of `required` that neither `fonts` (the project's library)
/// nor the installed system fonts (`index`) have: texts in them fall back
/// to Inter.
pub fn unavailable_fonts(
    required: &[RequiredFont],
    fonts: &crate::layout::FontLibrary,
    index: &crate::layout::system::SystemFontIndex,
) -> Vec<String> {
    required
        .iter()
        .filter(|r| !fonts.has_family(&r.family) && index.family(&r.family).is_none())
        .map(|r| r.family.clone())
        .collect()
}

impl Manifest {
    /// Checks the fields on their own.
    pub fn check(&self) -> Result<(), TemplateError> {
        let bad = |m: String| Err(TemplateError::Invalid(format!("template.yaml: {m}")));
        if self.format != TEMPLATE_FORMAT {
            return bad(format!(
                "unsupported format `{}` (expected `{TEMPLATE_FORMAT}`)",
                self.format
            ));
        }
        if !is_template_id(&self.id) {
            return bad(format!(
                "invalid id `{}` (lowercase letters, digits and `-`, at most 64 characters)",
                self.id
            ));
        }
        if self.name.trim().is_empty() || self.name.chars().count() > 80 {
            return bad("`name` must have 1 to 80 characters".into());
        }
        if self.description.chars().count() > 2000 || self.author.chars().count() > 120 {
            return bad("`description` (2000) or `author` (120) is too long".into());
        }
        if let Err(e) = self.tags.check(false) {
            return bad(format!("tags: {e}"));
        }
        if self.version.trim().is_empty()
            || self.version.len() > 32
            || self.version.chars().any(char::is_whitespace)
        {
            return bad(format!("invalid `version` `{}`", self.version));
        }
        if self.license != LICENSE {
            return bad(format!(
                "`license` must be `{LICENSE}` (templates are always CC0), not `{}`",
                self.license
            ));
        }
        if self.project_format != crate::PROJECT_FORMAT {
            return bad(format!(
                "`project_format` `{}` is not supported (expected `{}`)",
                self.project_format,
                crate::PROJECT_FORMAT
            ));
        }
        if self.canvases.is_empty() {
            return bad("a template needs at least one canvas".into());
        }
        for text in [&self.name, &self.description, &self.author] {
            if text.chars().any(|c| c.is_control() && c != '\n') {
                return bad("control characters in name, description or author".into());
            }
        }
        for f in &self.fonts {
            if f.license != FONT_LICENSE {
                return bad(format!(
                    "font `{}`: licence must be `{FONT_LICENSE}`",
                    f.file
                ));
            }
        }
        Ok(())
    }
}

/// Is `s` a valid template id or tag (`[a-z0-9][a-z0-9-]*`, ≤ 64)?
pub fn is_template_id(s: &str) -> bool {
    s.len() <= 64
        && s.bytes()
            .next()
            .is_some_and(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
        && s.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// A template id derived from a name (`My Bold Set!` → `my-bold-set`).
pub fn slug(name: &str) -> String {
    let mut out = String::new();
    for c in name.chars().flat_map(char::to_lowercase) {
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if !out.is_empty() && !out.ends_with('-') {
            out.push('-');
        }
    }
    let out: String = out.trim_end_matches('-').chars().take(64).collect();
    let out = out.trim_end_matches('-').to_owned();
    if out.is_empty() {
        "template".to_owned()
    } else {
        out
    }
}

/// A checked template package held in memory: the manifest and every other
/// file by its package path (`project.yaml`, `fonts/x.ttf` …).
#[derive(Clone, Debug, PartialEq)]
pub struct Package {
    pub manifest: Manifest,
    files: BTreeMap<String, Vec<u8>>,
}

/// What kind of file a package path may hold.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FileKind {
    Project,
    Strings,
    Screenshot,
    Asset,
    Font,
    FontLicense,
    Preview,
}

impl FileKind {
    /// The kind a package path must have, or why it is not allowed.
    pub(crate) fn of(path: &str) -> Result<Self, String> {
        let ext = path
            .rsplit_once('.')
            .map(|(_, e)| e.to_ascii_lowercase())
            .unwrap_or_default();
        let depth = path.split('/').count();
        let top = path.split('/').next().unwrap_or_default();
        let kind = match (top, ext.as_str()) {
            _ if path == PROJECT_FILE => Self::Project,
            ("i18n", "yaml") if depth == 2 => Self::Strings,
            ("screenshots", "png" | "jpg" | "jpeg") if depth >= 3 => Self::Screenshot,
            ("assets", "png" | "jpg" | "jpeg" | "svg") => Self::Asset,
            ("fonts", "ttf" | "otf") if depth == 2 => Self::Font,
            ("fonts", "txt") if depth == 2 => Self::FontLicense,
            ("previews", "png") if depth == 2 => Self::Preview,
            _ => {
                return Err(format!(
                    "`{path}`: not allowed in a template (expected project.yaml, i18n/*.yaml, \
                     screenshots/<lang>/…png|jpg, assets/…png|jpg|svg, fonts/*.ttf|otf + *.txt, previews/*.png)"
                ));
            }
        };
        Ok(kind)
    }
}

impl Package {
    /// A package from a manifest and files, fully checked.
    pub fn new(
        manifest: Manifest,
        files: BTreeMap<String, Vec<u8>>,
    ) -> Result<Self, TemplateError> {
        let pkg = Self { manifest, files };
        pkg.check()?;
        Ok(pkg)
    }

    /// Without checks (the caller checks later).
    pub(crate) fn unchecked(manifest: Manifest, files: BTreeMap<String, Vec<u8>>) -> Self {
        Self { manifest, files }
    }

    /// Package paths (without `template.yaml`), sorted.
    pub fn paths(&self) -> impl Iterator<Item = &str> {
        self.files.keys().map(String::as_str)
    }

    pub fn file(&self, path: &str) -> Option<&[u8]> {
        self.files.get(path).map(Vec::as_slice)
    }

    /// Adds or replaces a file (preview images after rendering). The kind
    /// rules apply; call [`Self::check`] when done.
    pub fn put(&mut self, path: &str, data: Vec<u8>) -> Result<(), TemplateError> {
        archive::check_path(path).map_err(TemplateError::Invalid)?;
        FileKind::of(path).map_err(TemplateError::Invalid)?;
        self.files.insert(path.to_owned(), data);
        Ok(())
    }

    /// Removes a file (`true` if it existed).
    pub fn remove(&mut self, path: &str) -> bool {
        self.files.remove(path).is_some()
    }

    /// Sum of all file sizes.
    pub fn size(&self) -> u64 {
        self.files.values().map(|v| v.len() as u64).sum()
    }

    /// The project subset with its strings (unchecked against the
    /// manifest; see [`Self::check`]).
    pub fn project(&self) -> Result<Project, TemplateError> {
        let text = self
            .text(PROJECT_FILE)
            .ok_or_else(|| TemplateError::invalid("project.yaml is missing"))?;
        let mut project = project_from_yaml(text).map_err(TemplateError::Project)?;
        project.dropped_override_nulls.clear();
        let prefix = format!("{I18N_DIR}/");
        for (path, data) in &self.files {
            let Some(name) = path.strip_prefix(&prefix) else {
                continue;
            };
            let stem = name.trim_end_matches(".yaml");
            let lang =
                Lang::parse(stem).map_err(|e| TemplateError::Project(format!("{path}: {e}")))?;
            let text = std::str::from_utf8(data)
                .map_err(|_| TemplateError::invalid(format!("{path}: not UTF-8")))?;
            let map = strings_from_yaml(text)
                .map_err(|e| TemplateError::Project(format!("{path}: {e}")))?;
            project.strings.insert(lang, map);
        }
        Ok(project)
    }

    fn text(&self, path: &str) -> Option<&str> {
        self.files
            .get(path)
            .and_then(|d| std::str::from_utf8(d).ok())
    }

    /// Preview image of a canvas, if the package has one.
    pub fn preview(&self, canvas: &str) -> Option<&[u8]> {
        let c = self.manifest.canvases.iter().find(|c| c.id == canvas)?;
        self.file(c.preview.as_deref()?)
    }

    /// Paths of the generated placeholder screenshots.
    pub fn screenshots(&self) -> impl Iterator<Item = &str> {
        self.paths().filter(|p| p.starts_with("screenshots/"))
    }

    /// Checks everything: manifest, paths and kinds, file contents (by
    /// magic bytes / parsing), fonts (OFL), the project subset (loads,
    /// validates without errors, no store/import settings, matches the
    /// manifest).
    pub fn check(&self) -> Result<(), TemplateError> {
        self.manifest.check()?;
        let mut lower = BTreeSet::new();
        for (path, data) in &self.files {
            archive::check_path(path).map_err(TemplateError::Invalid)?;
            if !lower.insert(path.to_lowercase()) {
                return Err(TemplateError::invalid(format!(
                    "`{path}` exists twice (names differing only in case)"
                )));
            }
            let kind = FileKind::of(path).map_err(TemplateError::Invalid)?;
            check_content(path, kind, data)?;
        }
        let project = self.project()?;
        self.check_project(&project)?;
        self.check_fonts()?;
        self.check_samples()?;
        Ok(())
    }

    /// Sample screenshots: listed once, present, under `screenshots/`,
    /// PNG/JPEG within [`SAMPLE_MAX_BYTES`] and [`SAMPLE_MAX_EDGE`].
    fn check_samples(&self) -> Result<(), TemplateError> {
        let mut seen = BTreeSet::new();
        for path in &self.manifest.samples {
            if !seen.insert(path.as_str()) {
                return Err(TemplateError::invalid(format!(
                    "template.yaml lists the sample `{path}` twice"
                )));
            }
            if FileKind::of(path) != Ok(FileKind::Screenshot) {
                return Err(TemplateError::invalid(format!(
                    "sample `{path}` is not a PNG/JPEG under screenshots/"
                )));
            }
            let data = self.file(path).ok_or_else(|| {
                TemplateError::invalid(format!("sample screenshot `{path}` is missing"))
            })?;
            if data.len() > SAMPLE_MAX_BYTES {
                return Err(TemplateError::invalid(format!(
                    "sample screenshot `{path}` is larger than {} MB",
                    SAMPLE_MAX_BYTES / (1024 * 1024)
                )));
            }
            let size = imagesize::blob_size(data).map_err(|_| {
                TemplateError::invalid(format!(
                    "sample screenshot `{path}` is not a readable image"
                ))
            })?;
            if size.width.max(size.height) > SAMPLE_MAX_EDGE as usize
                || size.width.min(size.height) == 0
            {
                return Err(TemplateError::invalid(format!(
                    "sample screenshot `{path}` is {}x{} px (at most {SAMPLE_MAX_EDGE} px per side)",
                    size.width, size.height
                )));
            }
        }
        Ok(())
    }

    /// Has the package sample screenshots?
    pub fn has_samples(&self) -> bool {
        !self.manifest.samples.is_empty()
    }

    fn check_project(&self, project: &Project) -> Result<(), TemplateError> {
        if !project.store.is_empty() || !project.import.is_empty() {
            return Err(TemplateError::Project(
                "templates must not contain store or import settings (`store:`, `import:`)".into(),
            ));
        }
        let ids: Vec<&str> = project.canvases.iter().map(|c| c.id.as_str()).collect();
        let listed: Vec<&str> = self
            .manifest
            .canvases
            .iter()
            .map(|c| c.id.as_str())
            .collect();
        if ids != listed {
            return Err(TemplateError::invalid(format!(
                "template.yaml lists the canvases {listed:?}, project.yaml has {ids:?}"
            )));
        }
        for c in &self.manifest.canvases {
            if let Some(p) = &c.preview
                && !self.files.contains_key(p)
            {
                return Err(TemplateError::invalid(format!(
                    "preview `{p}` of canvas `{}` is missing",
                    c.id
                )));
            }
        }
        // Every referenced asset is in the package.
        for canvas in &project.canvases {
            for asset in walk::assets(canvas) {
                if !self.files.contains_key(&asset) {
                    return Err(TemplateError::Project(format!(
                        "canvas `{}` uses `{asset}`, which is not in the template",
                        canvas.id
                    )));
                }
            }
        }
        validate_project(project, self)
    }

    fn check_fonts(&self) -> Result<(), TemplateError> {
        let listed: BTreeSet<&str> = self
            .manifest
            .fonts
            .iter()
            .map(|f| f.file.as_str())
            .collect();
        for path in self
            .paths()
            .filter(|p| FileKind::of(p) == Ok(FileKind::Font))
        {
            if !listed.contains(path) {
                return Err(TemplateError::invalid(format!(
                    "font `{path}` is not listed in template.yaml"
                )));
            }
        }
        for f in &self.manifest.fonts {
            let data = self
                .file(&f.file)
                .ok_or_else(|| TemplateError::invalid(format!("font `{}` is missing", f.file)))?;
            fonts::check_ofl(&f.file, data)?;
            let licence = self.text(&f.license_file).ok_or_else(|| {
                TemplateError::invalid(format!(
                    "licence file `{}` of font `{}` is missing",
                    f.license_file, f.file
                ))
            })?;
            if !fonts::is_ofl_text(licence) {
                return Err(TemplateError::FontLicense {
                    file: f.file.clone(),
                    reason: format!("`{}` is not the OFL 1.1 text", f.license_file),
                });
            }
        }
        Ok(())
    }

    /// Replaces every screenshot with a freshly generated placeholder (the
    /// gallery server does this so no uploaded app screenshot is ever
    /// published). With `keep_samples` the listed sample screenshots stay
    /// (the uploader confirmed they are their own and CC0) and only the
    /// other screenshots are regenerated; without it the samples are
    /// dropped too.
    pub fn regenerate_placeholders(&mut self, keep_samples: bool) -> Result<(), TemplateError> {
        if !keep_samples {
            self.manifest.samples.clear();
        }
        let samples: BTreeSet<String> = self.manifest.samples.iter().cloned().collect();
        self.files
            .retain(|p, _| !p.starts_with("screenshots/") || samples.contains(p));
        let project = self.project()?;
        for (path, png) in placeholder::for_project(&project) {
            if !placeholder::shadowed(&path, &samples) {
                self.files.insert(path, png);
            }
        }
        Ok(())
    }
}

/// Lowercase file extension of a package path.
fn extension(path: &str) -> String {
    path.rsplit_once('.')
        .map(|(_, e)| e.to_ascii_lowercase())
        .unwrap_or_default()
}

/// Content check of one file by kind.
fn check_content(path: &str, kind: FileKind, data: &[u8]) -> Result<(), TemplateError> {
    let bad = |what: &str| Err(TemplateError::invalid(format!("`{path}`: {what}")));
    let is_png = data.starts_with(b"\x89PNG\r\n\x1a\n");
    let is_jpeg = data.starts_with(&[0xFF, 0xD8, 0xFF]);
    match kind {
        FileKind::Project | FileKind::Strings | FileKind::FontLicense => {
            if std::str::from_utf8(data).is_err() {
                return bad("not UTF-8 text");
            }
            if data.len() > 2 * 1024 * 1024 {
                return bad("text file larger than 2 MB");
            }
        }
        FileKind::Preview => {
            if !is_png {
                return bad("not a PNG image");
            }
        }
        FileKind::Screenshot => {
            let ok = if extension(path) == "png" {
                is_png
            } else {
                is_jpeg
            };
            if !ok {
                return bad("content does not match the file type");
            }
        }
        FileKind::Asset => match extension(path).as_str() {
            "svg" => check_svg(path, data)?,
            "png" if !is_png => return bad("content does not match the file type"),
            "jpg" | "jpeg" if !is_jpeg => return bad("content does not match the file type"),
            _ => {}
        },
        FileKind::Font => {
            if skrifa::FontRef::new(data).is_err() {
                return bad("not a usable TrueType/OpenType font");
            }
        }
    }
    Ok(())
}

/// SVG assets: UTF-8 text without scripts, event handlers, entity
/// declarations or references to outside files (resvg runs no scripts, but
/// the gallery serves these files to others).
fn check_svg(path: &str, data: &[u8]) -> Result<(), TemplateError> {
    let Ok(text) = std::str::from_utf8(data) else {
        return Err(TemplateError::invalid(format!(
            "`{path}`: SVG is not UTF-8"
        )));
    };
    let lower = text.to_ascii_lowercase();
    if !lower.contains("<svg") {
        return Err(TemplateError::invalid(format!(
            "`{path}`: not an SVG image"
        )));
    }
    let forbidden = [
        "<script",
        "<!entity",
        "<foreignobject",
        "javascript:",
        "href=\"http",
        "href='http",
        "href=\"file",
        "href='file",
        "href=\"//",
        "href='//",
    ];
    if forbidden.iter().any(|n| lower.contains(n)) || has_event_handler(&lower) {
        return Err(TemplateError::invalid(format!(
            "`{path}`: SVG with scripts, event handlers, entities or external references is not allowed"
        )));
    }
    Ok(())
}

/// Does the (lowercase) markup contain an attribute like ` onload=`?
fn has_event_handler(lower: &str) -> bool {
    let b = lower.as_bytes();
    (1..b.len().saturating_sub(2)).any(|i| {
        b[i - 1].is_ascii_whitespace() && b[i] == b'o' && b[i + 1] == b'n' && {
            let rest = &lower[i + 2..];
            let name = rest.bytes().take_while(u8::is_ascii_alphabetic).count();
            name > 0 && rest[name..].trim_start().starts_with('=')
        }
    })
}

/// Findings that do not make a template invalid: store rules, screenshot
/// state and text fitting depend on the user's later content.
const TOLERATED: &[crate::validate::FindingCode] = {
    use crate::validate::FindingCode as F;
    &[
        F::StoreSize,
        F::StoreCount,
        F::StoreFormat,
        F::StoreDecorations,
        F::StoreLocaleUnknown,
        F::StoreLocaleUnused,
        F::StoreLocaleMissing,
        F::StoreLocaleDuplicate,
        F::StoreNoSlot,
        F::StoreSlotDuplicate,
        F::ScreenshotMissing,
        F::ScreenshotAspect,
        F::ScreenshotUpscale,
        F::ScreenshotOnlyEmpty,
        F::TextOverflow,
        F::TextClipped,
        F::TextMissing,
        F::EmergencyBreak,
        F::UnknownFont,
        F::MissingGlyphs,
        F::ImportedFrameMissing,
    ]
};

/// Validates the project subset with `core::validate` (fonts: bundled +
/// the package's; assets: the package's files). Errors other than
/// [`TOLERATED`] ones make the template invalid.
fn validate_project(project: &Project, pkg: &Package) -> Result<(), TemplateError> {
    use crate::validate::{Severity, ValidateOptions, validate};
    let mut assets: BTreeMap<String, (u32, u32)> = BTreeMap::new();
    let mut fonts = crate::layout::FontLibrary::bundled();
    for (path, data) in &pkg.files {
        match FileKind::of(path) {
            Ok(FileKind::Font) => {
                let _ = fonts.add_font(data.clone(), &crate::layout::FontSource::File(path.into()));
            }
            Ok(FileKind::Screenshot | FileKind::Asset) => {
                let dims = imagesize::blob_size(data)
                    .ok()
                    .and_then(|s| {
                        Some((u32::try_from(s.width).ok()?, u32::try_from(s.height).ok()?))
                    })
                    .unwrap_or((1, 1));
                assets.insert(path.clone(), dims);
            }
            _ => {}
        }
    }
    let mut options = ValidateOptions::new(&assets);
    options.fonts = fonts;
    let report = validate(project, &options);
    let errors: Vec<String> = report
        .findings
        .iter()
        .filter(|f| f.level == Severity::Error && !TOLERATED.contains(&f.code))
        .map(|f| format!("{} ({})", f.message, f.code))
        .take(5)
        .collect();
    if errors.is_empty() {
        Ok(())
    } else {
        Err(TemplateError::Project(errors.join("; ")))
    }
}

/// Parses `template.yaml`.
pub fn manifest_from_yaml(text: &str) -> Result<Manifest, TemplateError> {
    from_yaml::<Manifest>(text).map_err(|e| TemplateError::invalid(format!("template.yaml: {e}")))
}

/// `template.yaml` text.
pub fn manifest_to_yaml(m: &Manifest) -> Result<String, TemplateError> {
    crate::io::to_yaml(m).map_err(|e| TemplateError::invalid(e.to_string()))
}

/// Summary of a template for lists (store, gallery, GUI).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TemplateSummary {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub author: String,
    #[serde(default, skip_serializing_if = "TemplateTags::is_empty")]
    pub tags: TemplateTags,
    /// Derived device tags (`phone`, `tablet`, … ; [`tags::devices`]).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub devices: Vec<String>,
    /// Number of sample screenshots (0 = placeholders only).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub samples: usize,
    pub version: String,
    pub license: String,
    pub canvases: Vec<CanvasInfo>,
    /// Font families shipped with the template.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fonts: Vec<String>,
    /// `builtin` or `installed`.
    pub source: TemplateSource,
}

/// Where a template comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TemplateSource {
    /// Shipped with Vitrine Studio.
    Builtin,
    /// In the app-wide store (saved, imported or downloaded).
    Installed,
}

#[allow(clippy::trivially_copy_pass_by_ref)] // serde passes references
fn is_zero(n: &usize) -> bool {
    *n == 0
}

impl TemplateSummary {
    /// Summary of a package (with the derived device tags).
    pub fn of_package(pkg: &Package, source: TemplateSource) -> Self {
        let mut s = Self::of(&pkg.manifest, source);
        if let Ok(project) = pkg.project() {
            s.devices = tags::devices(&project);
        }
        s
    }

    /// Summary from the manifest alone (no device tags).
    pub fn of(m: &Manifest, source: TemplateSource) -> Self {
        let mut fonts: Vec<String> = m.fonts.iter().map(|f| f.family.clone()).collect();
        fonts.dedup();
        Self {
            id: m.id.clone(),
            name: m.name.clone(),
            description: m.description.clone(),
            author: m.author.clone(),
            tags: m.tags.clone(),
            devices: Vec::new(),
            samples: m.samples.len(),
            version: m.version.clone(),
            license: m.license.clone(),
            canvases: m.canvases.clone(),
            fonts,
            source,
        }
    }
}
