//! Screenshot import (KONZEPT §5.2, ADR 0010): plan where the files of a
//! source folder go in `screenshots/<lang>/[<variant>/]<key>.<ext>`, then
//! copy them.
//!
//! Source layouts ([`SourceFormat`]):
//!
//! - **Vitrine Studio convention**: `<lang>/<variant>/<key>.png` and
//!   `<lang>/<key>.png` (optionally below a `screenshots/` folder).
//! - **fastlane snapshot**: `<locale>/<Device Name>-<screen>.png`;
//!   `screenshots.html` and frameit's `*_framed.png` are skipped. Locale →
//!   language: the `locales` map, the exact language, then shorter
//!   subtags (`de-DE` → `de`). Device → variant: the `devices` map, then
//!   the pixel size via the device catalog (exact sizes, then aspect),
//!   preferring the variants the project's targets use. Screen → key: the
//!   `keys` map, else the screen name without a numeric prefix (`01_`),
//!   slugified, matched against the keys the project's devices use
//!   (`01Home` → `home`). A whole `fastlane/` folder may be picked:
//!   `screenshots/` is read as snapshot output, `metadata/android/` as
//!   screengrab output (`<locale>/images/<type>Screenshots/<name>_<millis>.png`,
//!   variant from the folder via `data/fastlane.yaml`, timestamp dropped);
//!   deliver metadata, Play store graphics, `iMessage/`/`appleTV/`,
//!   frameit configs (`Framefile.json`, `*.strings`) and the backgrounds a
//!   `Framefile.json` names are skipped with a reason. Locale folders also
//!   match through the store locale maps (`zh-Hans`, `iw-IL` → `he`).
//! - **Flat** (a folder of files, drag & drop): language, variant and
//!   device from file-name tokens (and parent folder names), the variant
//!   otherwise from the pixel size; no language → the default language.
//!
//! Vitrine Studio's own exports are never captures: files an export
//! manifest ([`EXPORT_MANIFEST`], in the source tree or up to three
//! folders above it) lists are skipped, and in fastlane folders so are
//! files named like the fastlane export layout (`01_<target>.png`,
//! `01_<play-locale>.png`; ADR 0032).
//!
//! [`plan_screenshot_import`] only reads; the [`ImportPlan`] lists every
//! file with its destination, what was detected (with confidence and
//! notes), conflicts (overwrites, several files for one destination),
//! skipped files and the screenshots still missing per language and
//! variant afterwards. [`apply`] copies (never moves) the planned files,
//! each atomically (temp file + rename), skipping unchanged ones.
//!
//! The maps come from `import.fastlane` in `project.yaml`
//! ([`crate::model::ImportSettings`]) plus [`ImportRules::mapping`]; the
//! plan's [`ImportPlan::mapping`] is the effective mapping including what
//! was detected, ready to be saved back with `Op::SetImport`.

mod names;
#[cfg(test)]
mod tests;

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use serde::{Deserialize, Serialize};

pub use names::{
    DevicePrefixes, catalog_variants, slugify, split_fastlane, strip_numeric_prefix,
    variants_for_aspect, variants_for_size,
};

use crate::catalog;
use crate::io::write_atomic;
use crate::model::{ElementKind, ImportMapping, Lang, Project, StoreKind, is_valid_id};
use crate::resolve::{
    AssetLookup, FsAssets, SCREENSHOT_EXTENSIONS, ScreenshotSource, lang_chain, lookup_screenshot,
};

/// Layout of the source folder.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceFormat {
    /// Detect from the files (fastlane names, language folders, else flat).
    #[default]
    Auto,
    Vitrine,
    Fastlane,
    Flat,
}

impl SourceFormat {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Vitrine => "vitrine",
            Self::Fastlane => "fastlane",
            Self::Flat => "flat",
        }
    }
}

impl fmt::Display for SourceFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for SourceFormat {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, String> {
        Ok(match s {
            "auto" => Self::Auto,
            "vitrine" => Self::Vitrine,
            "fastlane" => Self::Fastlane,
            "flat" => Self::Flat,
            _ => {
                return Err(format!(
                    "unknown import format `{s}` (auto, vitrine, fastlane, flat)"
                ));
            }
        })
    }
}

/// How sure the detection is (lowest of language, variant and key).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Confidence {
    Low,
    Medium,
    High,
}

/// What [`apply`] does with a planned file.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    /// Copied to a new destination.
    New,
    /// Replaces an existing, different file.
    Overwrite,
    /// The destination already has the same content: nothing to do.
    Unchanged,
    /// Another file of the plan has the same destination: not copied.
    Duplicate,
    /// Language, variant or key could not be determined: not copied.
    Unresolved,
}

impl Action {
    /// Will [`apply`] copy the file?
    pub fn copies(self) -> bool {
        matches!(self, Self::New | Self::Overwrite)
    }
}

/// Per-file correction (GUI row edits), by source path.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ItemOverride {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lang: Option<String>,
    /// `""` = directly in the language folder (all variants).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub variant: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    /// Leave the file out.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub skip: bool,
}

/// Options of a plan.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImportRules {
    #[serde(default)]
    pub format: SourceFormat,
    /// Extra maps on top of the project's `import.fastlane` (these win).
    #[serde(default)]
    pub mapping: ImportMapping,
    /// Import frameit's `*_framed.png` instead of skipping them.
    #[serde(default)]
    pub include_framed: bool,
    /// Corrections by source path (as in [`PlannedFile::source`]).
    #[serde(default)]
    pub overrides: BTreeMap<String, ItemOverride>,
}

/// One importable file of the plan.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlannedFile {
    /// Path relative to the source folder (`/`-separated).
    pub source: String,
    /// Absolute source path.
    pub path: PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lang: Option<String>,
    /// `None`: directly in the language folder (serves all variants).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub variant: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    /// Project-relative destination (`None` when unresolved).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
    /// fastlane device name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device: Option<String>,
    /// Locale / language folder or token as found in the source.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub locale: Option<String>,
    /// Screen name as found in the source (before mapping).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub screen: Option<String>,
    /// Catalog variants matching the pixel size.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub candidates: Vec<String>,
    pub confidence: Confidence,
    pub action: Action,
    /// Why language, variant and key were chosen; conflicts.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
    /// Corrected by an [`ItemOverride`].
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub overridden: bool,
}

/// A file of the source folder that is not imported.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkippedFile {
    pub source: String,
    pub reason: String,
}

/// State of a used screenshot after the import.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MissingStatus {
    /// No file in any language: a placeholder is rendered.
    Missing,
    /// Taken from a fallback language.
    Fallback,
}

/// A screenshot key a target needs that has no own file afterwards.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MissingScreenshot {
    pub lang: String,
    pub variant: String,
    pub key: String,
    pub status: MissingStatus,
    /// Fallback language used.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
}

/// Result of [`plan_screenshot_import`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ImportPlan {
    pub source_dir: PathBuf,
    /// Detected (or given) layout.
    pub format: SourceFormat,
    pub items: Vec<PlannedFile>,
    pub skipped: Vec<SkippedFile>,
    /// Used screenshots without an own file per language × variant after
    /// applying the plan.
    pub missing: Vec<MissingScreenshot>,
    /// Effective mapping (project + rules + detected), to save with
    /// `Op::SetImport`.
    pub mapping: ImportMapping,
}

impl ImportPlan {
    pub fn count(&self, action: Action) -> usize {
        self.items.iter().filter(|i| i.action == action).count()
    }

    /// Files [`apply`] would copy.
    pub fn to_copy(&self) -> usize {
        self.items.iter().filter(|i| i.action.copies()).count()
    }

    /// Unresolved or duplicate files.
    pub fn has_conflicts(&self) -> bool {
        self.items
            .iter()
            .any(|i| matches!(i.action, Action::Unresolved | Action::Duplicate))
    }
}

/// Result of [`apply`].
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApplyReport {
    /// Destinations written (project-relative).
    pub copied: Vec<String>,
    /// Destinations that already had the same content.
    pub unchanged: Vec<String>,
    /// Files not copied (unresolved, duplicate).
    pub skipped: usize,
    /// `(destination, error)`.
    pub failed: Vec<(String, String)>,
}

/// Errors of [`plan_screenshot_import`].
#[derive(Debug, thiserror::Error)]
pub enum ImportError {
    #[error("`{}` is not a folder", .0.display())]
    NotADir(PathBuf),
    #[error("{}: {source}", path.display())]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

// ----- planning ---------------------------------------------------------------------

/// One source file: absolute path and path parts relative to the source.
struct Source {
    abs: PathBuf,
    parts: Vec<String>,
}

impl Source {
    fn rel(&self) -> String {
        self.parts.join("/")
    }
    fn stem(&self) -> String {
        let name = self.parts.last().map_or("", String::as_str);
        match name.rfind('.') {
            Some(i) if i > 0 => name[..i].to_owned(),
            _ => name.to_owned(),
        }
    }
    fn ext(&self) -> Option<String> {
        let name = self.parts.last()?;
        let i = name.rfind('.')?;
        Some(name[i + 1..].to_ascii_lowercase())
    }
}

/// Plans importing the screenshots below `source_dir` into the project in
/// `project_dir`. Reads file headers (pixel sizes) and compares existing
/// destinations, writes nothing.
pub fn plan_screenshot_import(
    project: &Project,
    project_dir: &Path,
    source_dir: &Path,
    rules: &ImportRules,
) -> Result<ImportPlan, ImportError> {
    if !source_dir.is_dir() {
        return Err(ImportError::NotADir(source_dir.to_owned()));
    }
    let mut sources = Vec::new();
    let mut manifests = Vec::new();
    list(source_dir, &mut Vec::new(), &mut sources, &mut manifests)?;
    // An export manifest in the folder or up to `MANIFEST_LEVELS_UP`
    // levels above it (`fastlane/` when `fastlane/screenshots` is picked).
    for dir in source_dir.ancestors().take(MANIFEST_LEVELS_UP + 1) {
        let m = dir.join(EXPORT_MANIFEST);
        if m.is_file() && !manifests.contains(&m) {
            manifests.push(m);
        }
    }
    let own = exported_files(&manifests);
    Ok(plan_sources(
        project,
        project_dir,
        source_dir,
        sources,
        rules,
        &own,
    ))
}

/// File name of the export manifest Vitrine Studio writes into every
/// export folder (`vitrine_render::export::manifest`): a JSON object whose
/// keys are the exported files, relative to the manifest's folder.
pub const EXPORT_MANIFEST: &str = ".vitrine-export.json";

/// How many folders above the source folder the import looks for an
/// export manifest.
const MANIFEST_LEVELS_UP: usize = 3;

/// The files the given export manifests list that exist (canonical paths):
/// Vitrine Studio's own exports, never imported as captures.
fn exported_files(manifests: &[PathBuf]) -> BTreeSet<PathBuf> {
    let mut out = BTreeSet::new();
    for m in manifests {
        let Some(dir) = m.parent() else { continue };
        let Some(map) = std::fs::read(m)
            .ok()
            .and_then(|b| serde_json::from_slice::<BTreeMap<String, serde_json::Value>>(&b).ok())
        else {
            continue;
        };
        for rel in map.keys() {
            // Manifest paths are relative and `/`-separated; never follow
            // `..` or absolute entries out of the manifest's folder.
            if rel
                .split('/')
                .any(|c| c.is_empty() || c == "." || c == "..")
            {
                continue;
            }
            let path = rel.split('/').fold(dir.to_path_buf(), |p, c| p.join(c));
            if let Ok(c) = std::fs::canonicalize(&path) {
                out.insert(c);
            }
        }
    }
    out
}

/// Plans importing single files (drag & drop) as a flat source: names are
/// the file names, `format` is ignored.
pub fn plan_files(
    project: &Project,
    project_dir: &Path,
    files: &[PathBuf],
    rules: &ImportRules,
) -> ImportPlan {
    let sources = files
        .iter()
        .map(|f| Source {
            abs: f.clone(),
            parts: vec![
                f.file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default(),
            ],
        })
        .collect();
    let rules = ImportRules {
        format: SourceFormat::Flat,
        ..rules.clone()
    };
    let dir = files
        .first()
        .and_then(|f| f.parent())
        .map(Path::to_path_buf)
        .unwrap_or_default();
    plan_sources(
        project,
        project_dir,
        &dir,
        sources,
        &rules,
        &BTreeSet::new(),
    )
}

/// Files below `dir`, recursively, sorted, hidden entries skipped; export
/// manifests ([`EXPORT_MANIFEST`]) found on the way go to `manifests`.
fn list(
    dir: &Path,
    prefix: &mut Vec<String>,
    out: &mut Vec<Source>,
    manifests: &mut Vec<PathBuf>,
) -> Result<(), ImportError> {
    let rd = std::fs::read_dir(dir).map_err(|source| ImportError::Io {
        path: dir.to_owned(),
        source,
    })?;
    let mut entries: Vec<_> = rd.flatten().collect();
    entries.sort_by_key(std::fs::DirEntry::file_name);
    for e in entries {
        let name = e.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') {
            if name == EXPORT_MANIFEST && e.path().is_file() {
                manifests.push(e.path());
            }
            continue;
        }
        let p = e.path();
        prefix.push(name);
        if p.is_dir() {
            // Depth limit against symlink loops; real layouts are ≤ 3 deep.
            if prefix.len() < 8 {
                list(&p, prefix, out, manifests)?;
            }
        } else {
            out.push(Source {
                abs: p,
                parts: prefix.clone(),
            });
        }
        prefix.pop();
    }
    Ok(())
}

/// Shared context of the planning steps.
struct Ctx<'a> {
    project: &'a Project,
    mapping: ImportMapping,
    /// Variants named by the project's targets (target order).
    target_variants: Vec<String>,
    /// Catalog + target variants.
    known_variants: Vec<String>,
    /// Keys used by device elements.
    keys: BTreeSet<String>,
    /// Device-name prefixes of all source files (fastlane splitting).
    prefixes: DevicePrefixes,
}

/// A detected value with its confidence and an explanation.
type Found = (String, Confidence, String);

impl Ctx<'_> {
    /// Locale or folder name → project language.
    fn lang(&self, raw: &str) -> Option<Found> {
        if let Some(l) = self.mapping.locales.get(raw) {
            return Some((
                l.to_string(),
                Confidence::High,
                format!("language `{raw}` → {l} (locale map)"),
            ));
        }
        let langs = &self.project.languages.available;
        if let Some(l) = langs.iter().find(|l| l.as_str() == raw) {
            return Some((l.to_string(), Confidence::High, format!("language {l}")));
        }
        let norm = raw.replace('_', "-");
        if let Some(l) = langs
            .iter()
            .find(|l| l.as_str().eq_ignore_ascii_case(&norm))
        {
            return Some((
                l.to_string(),
                Confidence::High,
                format!("language `{raw}` → {l}"),
            ));
        }
        // Store locale folders (`zh-Hans`, `no`, Play's `iw-IL`): the project
        // language whose App Store / Play locale (project map or default)
        // this is.
        for store in StoreKind::ALL {
            let explicit = self.project.store.locales.get(store);
            let codes = catalog::store_locales().store(store);
            if let Some(l) = langs.iter().find(|l| {
                codes
                    .resolve(explicit, l.as_str())
                    .is_some_and(|(c, _)| c.eq_ignore_ascii_case(&norm))
            }) {
                return Some((
                    l.to_string(),
                    Confidence::High,
                    format!("locale `{raw}` → {l} ({store} store locale)"),
                ));
            }
        }
        let mut cur = norm.as_str();
        while let Some(i) = cur.rfind('-') {
            cur = &cur[..i];
            if let Some(l) = langs.iter().find(|l| l.as_str().eq_ignore_ascii_case(cur)) {
                return Some((
                    l.to_string(),
                    Confidence::High,
                    format!("language `{raw}` → {l} (language subtag)"),
                ));
            }
        }
        None
    }

    /// Variant of a device name in the device map (exact, then any case).
    fn mapped_device(&self, device: &str) -> Option<String> {
        self.mapping
            .devices
            .get(device)
            .or_else(|| {
                self.mapping
                    .devices
                    .iter()
                    .find(|(k, _)| k.eq_ignore_ascii_case(device))
                    .map(|(_, v)| v)
            })
            .cloned()
    }

    /// Variant by pixel size: `(variant, confidence, candidates, note)`.
    fn variant_by_size(
        &self,
        size: Option<(u32, u32)>,
    ) -> (Option<String>, Confidence, Vec<String>, String) {
        let Some((w, h)) = size else {
            return (
                None,
                Confidence::Low,
                Vec::new(),
                "pixel size unreadable".to_owned(),
            );
        };
        let exact = variants_for_size(w, h);
        let (candidates, base, how) = if exact.is_empty() {
            (variants_for_aspect(w, h), Confidence::Medium, "aspect")
        } else {
            (exact, Confidence::High, "pixel size")
        };
        if candidates.is_empty() {
            return (
                None,
                Confidence::Low,
                candidates,
                format!("no catalog device matches {w}×{h}"),
            );
        }
        let used: Vec<&String> = candidates
            .iter()
            .filter(|c| self.target_variants.contains(c))
            .collect();
        let (pick, conf, extra) = match used.as_slice() {
            [one] => ((*one).clone(), base, String::new()),
            [first, ..] => (
                (*first).clone(),
                Confidence::Low,
                format!(", ambiguous: {}", candidates.join(", ")),
            ),
            [] => (
                candidates[0].clone(),
                Confidence::Low,
                ", not used by any target".to_owned(),
            ),
        };
        let note = format!("variant {pick} ({how} {w}×{h}{extra})");
        (Some(pick), conf, candidates, note)
    }

    /// Screen name → key.
    fn key(&self, screen: &str, strip_number: bool) -> Found {
        if let Some(k) = self.mapping.keys.get(screen) {
            return (
                k.clone(),
                Confidence::High,
                format!("key `{screen}` → {k} (key map)"),
            );
        }
        let stripped = if strip_number {
            strip_numeric_prefix(screen)
        } else {
            screen
        };
        if let Some(k) = self.mapping.keys.get(stripped) {
            return (
                k.clone(),
                Confidence::High,
                format!("key `{stripped}` → {k} (key map)"),
            );
        }
        let slug = slugify(stripped);
        if self.keys.contains(&slug) {
            let note = if slug == screen {
                format!("key {slug}")
            } else {
                format!("key `{screen}` → {slug}")
            };
            return (slug, Confidence::High, note);
        }
        let n = names::normalized(&slug);
        if let Some(k) = self.keys.iter().find(|k| names::normalized(k) == n) {
            return (
                k.clone(),
                Confidence::Medium,
                format!("key `{screen}` → {k} (similar project key)"),
            );
        }
        let conf = if self.keys.is_empty() {
            Confidence::Medium
        } else {
            Confidence::Low
        };
        (
            slug.clone(),
            conf,
            format!("key {slug} is not used by any device"),
        )
    }
}

/// Partial detection of one file.
#[derive(Default)]
struct Detection {
    lang: Option<Found>,
    lang_problem: Option<String>,
    variant: Option<String>,
    variant_conf: Option<Confidence>,
    variant_problem: Option<String>,
    key: Option<Found>,
    device: Option<String>,
    locale: Option<String>,
    screen: Option<String>,
    candidates: Vec<String>,
    notes: Vec<String>,
}

#[allow(clippy::too_many_lines)]
fn plan_sources(
    project: &Project,
    project_dir: &Path,
    source_dir: &Path,
    sources: Vec<Source>,
    rules: &ImportRules,
    own_exports: &BTreeSet<PathBuf>,
) -> ImportPlan {
    let mut mapping = project.import.fastlane.clone();
    mapping.merge(&rules.mapping);
    let mut target_variants: Vec<String> = Vec::new();
    for t in &project.targets {
        if let Some(v) = &t.screenshots
            && !target_variants.contains(v)
        {
            target_variants.push(v.clone());
        }
    }
    let mut known_variants = catalog_variants();
    for v in &target_variants {
        if !known_variants.contains(v) {
            known_variants.push(v.clone());
        }
    }
    let mut ctx = Ctx {
        project,
        mapping,
        target_variants,
        known_variants,
        keys: used_keys(project).into_values().flatten().collect(),
        prefixes: DevicePrefixes::default(),
    };

    // Strip a leading `fastlane/` (a fastlane folder picked as source) and
    // `screenshots/` (a project folder, or deliver's screenshots folder).
    let sources: Vec<Source> = sources
        .into_iter()
        .map(|mut s| {
            if s.parts.len() > 1 && s.parts[0].eq_ignore_ascii_case("fastlane") {
                s.parts.remove(0);
            }
            if s.parts.len() > 1 && s.parts[0] == "screenshots" {
                s.parts.remove(0);
            }
            s
        })
        .collect();
    // Vitrine Studio's own exports (an export manifest lists them) are left
    // out before the format is detected.
    let mut skipped: Vec<SkippedFile> = Vec::new();
    let sources: Vec<Source> = sources
        .into_iter()
        .filter(|s| {
            let own = !own_exports.is_empty()
                && std::fs::canonicalize(&s.abs).is_ok_and(|c| own_exports.contains(&c));
            if own {
                skipped.push(SkippedFile {
                    source: s.rel(),
                    reason: SKIP_OWN_EXPORT.to_owned(),
                });
            }
            !own
        })
        .collect();
    let backgrounds = frameit_backgrounds(&sources);

    let format = match rules.format {
        SourceFormat::Auto => detect_format(&ctx, &sources),
        f => f,
    };
    let stems: Vec<String> = sources.iter().map(Source::stem).collect();
    ctx.prefixes = DevicePrefixes::new(stems.iter().map(String::as_str));

    let fl = catalog::fastlane();
    let mut items: Vec<PlannedFile> = Vec::new();
    for src in sources {
        let rel = src.rel();
        let skip = |reason: &str| SkippedFile {
            source: rel.clone(),
            reason: reason.to_owned(),
        };
        let name = src.parts.last().cloned().unwrap_or_default();
        if name.eq_ignore_ascii_case(&fl.snapshot.report) {
            skipped.push(skip("fastlane report"));
            continue;
        }
        if fl
            .frameit
            .config_files
            .iter()
            .any(|f| f.eq_ignore_ascii_case(&name))
        {
            skipped.push(skip("frameit configuration"));
            continue;
        }
        if backgrounds.contains(&rel) {
            skipped.push(skip("frameit background (Framefile.json)"));
            continue;
        }
        let route = if format == SourceFormat::Fastlane {
            fastlane_route(&src.parts)
        } else {
            Route::Snapshot
        };
        if let Route::Skip(reason) = &route {
            skipped.push(skip(reason));
            continue;
        }
        let Some(ext) = src
            .ext()
            .filter(|e| SCREENSHOT_EXTENSIONS.contains(&e.as_str()))
        else {
            skipped.push(skip("not a screenshot image (png, jpg)"));
            continue;
        };
        let ext = if ext == "jpeg" { "jpg".to_owned() } else { ext };
        let mut stem = src.stem();
        let framed = &fl.frameit.framed_suffix;
        if stem.ends_with(framed.as_str()) {
            if !rules.include_framed {
                skipped.push(skip("framed screenshot (frameit)"));
                continue;
            }
            stem.truncate(stem.len() - framed.len());
        }
        if format == SourceFormat::Fastlane && looks_exported(project, &route, &src.parts, &stem) {
            skipped.push(skip(SKIP_EXPORT_NAME));
            continue;
        }
        let ov = rules.overrides.get(&rel);
        if ov.is_some_and(|o| o.skip) {
            skipped.push(skip("skipped by you"));
            continue;
        }
        let size = imagesize::size(&src.abs)
            .ok()
            .and_then(|s| Some((u32::try_from(s.width).ok()?, u32::try_from(s.height).ok()?)));
        let det = match format {
            SourceFormat::Vitrine => detect_vitrine(&ctx, &src, &stem, size),
            SourceFormat::Fastlane => match &route {
                Route::Screengrab { locale, kind } => {
                    Ok(detect_screengrab(&ctx, locale, kind, &stem, size))
                }
                _ => detect_fastlane(&ctx, &src, &stem, size),
            },
            SourceFormat::Flat | SourceFormat::Auto => Ok(detect_flat(&ctx, &src, &stem, size)),
        };
        let det = match det {
            Ok(d) => d,
            Err(reason) => {
                skipped.push(skip(&reason));
                continue;
            }
        };
        items.push(finish(&ctx, rel, src.abs, det, ov, &ext, size));
    }

    // Destinations: existing files, duplicates.
    let mut by_dest: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (i, it) in items.iter().enumerate() {
        if let Some(d) = &it.dest {
            by_dest.entry(d.clone()).or_default().push(i);
        }
    }
    for (dest, idx) in by_dest {
        // Keep the most confident file; on a tie the one already in the
        // project (a duplicate never overwrites it), then the first by
        // source path.
        let abs = project_dir.join(&dest);
        let exists = abs.is_file();
        let keep = *idx
            .iter()
            .max_by_key(|&&i| {
                let same = idx.len() > 1 && exists && same_content(&items[i].path, &abs);
                (items[i].confidence, same, std::cmp::Reverse(i))
            })
            .unwrap_or(&idx[0]);
        let action = if exists {
            if same_content(&items[keep].path, &abs) {
                Action::Unchanged
            } else {
                items[keep]
                    .notes
                    .push("replaces the existing file".to_owned());
                Action::Overwrite
            }
        } else {
            Action::New
        };
        items[keep].action = action;
        let kept = items[keep].source.clone();
        for &i in &idx {
            if i != keep {
                items[i].action = Action::Duplicate;
                items[i].notes.push(format!("same destination as `{kept}`"));
            }
        }
    }

    let learned = learned_mapping(&ctx, &items);
    let mut mapping = ctx.mapping.clone();
    mapping.merge(&learned);
    let missing = missing_after(project, project_dir, &items);
    ImportPlan {
        source_dir: source_dir.to_owned(),
        format,
        items,
        skipped,
        missing,
        mapping,
    }
}

/// Builds the planned file from a detection and an optional override.
#[allow(clippy::too_many_lines)]
fn finish(
    ctx: &Ctx<'_>,
    source: String,
    path: PathBuf,
    det: Detection,
    ov: Option<&ItemOverride>,
    ext: &str,
    size: Option<(u32, u32)>,
) -> PlannedFile {
    let mut notes = det.notes;
    let mut conf = Confidence::High;
    let mut problems: Vec<String> = Vec::new();
    let mut overridden = false;

    let mut lang = det.lang.map(|(l, c, n)| {
        conf = conf.min(c);
        notes.push(n);
        l
    });
    if lang.is_none() {
        problems.push(
            det.lang_problem
                .unwrap_or_else(|| "no language detected".to_owned()),
        );
    }
    let mut variant = det.variant;
    if let Some(c) = det.variant_conf {
        conf = conf.min(c);
    }
    let mut variant_problem = det.variant_problem;
    let mut key = det.key.map(|(k, c, n)| {
        conf = conf.min(c);
        notes.push(n);
        k
    });

    if let Some(o) = ov {
        if let Some(l) = &o.lang {
            overridden = true;
            problems.retain(|p| !p.contains("language"));
            lang = Some(l.clone());
            notes.push(format!("language {l} (set by you)"));
        }
        if let Some(v) = &o.variant {
            overridden = true;
            variant_problem = None;
            variant = (!v.is_empty()).then(|| v.clone());
            notes.push(format!(
                "variant {} (set by you)",
                if v.is_empty() { "none" } else { v }
            ));
        }
        if let Some(k) = &o.key {
            overridden = true;
            key = Some(k.clone());
            notes.push(format!("key {k} (set by you)"));
        }
        if overridden {
            // Corrected rows count as certain.
            conf = Confidence::High;
        }
    }
    if let Some(p) = variant_problem {
        problems.push(p);
    }
    if let Some(l) = &lang
        && !ctx.project.languages.is_available(l)
    {
        problems.push(format!("language `{l}` is not a project language"));
    }
    if let Some(v) = &variant
        && !is_valid_id(v)
    {
        problems.push(format!("invalid variant `{v}`"));
    }
    match &key {
        Some(k) if !is_valid_id(k) => problems.push(format!("invalid key `{k}`")),
        None => problems.push("no screenshot key".to_owned()),
        _ => {}
    }
    let dest = if problems.is_empty() {
        lang.as_ref()
            .zip(key.as_ref())
            .map(|(l, k)| match &variant {
                Some(v) => format!("screenshots/{l}/{v}/{k}.{ext}"),
                None => format!("screenshots/{l}/{k}.{ext}"),
            })
    } else {
        None
    };
    let action = if dest.is_some() {
        Action::New // refined after duplicate/existing checks
    } else {
        conf = Confidence::Low;
        notes.extend(problems);
        Action::Unresolved
    };
    PlannedFile {
        source,
        path,
        lang,
        variant,
        key,
        dest,
        width: size.map(|s| s.0),
        height: size.map(|s| s.1),
        device: det.device,
        locale: det.locale,
        screen: det.screen,
        candidates: det.candidates,
        confidence: conf,
        action,
        notes,
        overridden,
    }
}

/// Auto-detects the layout.
fn detect_format(ctx: &Ctx<'_>, sources: &[Source]) -> SourceFormat {
    let mut fastlane = 0usize;
    let mut vitrine = 0usize;
    let fl = catalog::fastlane();
    for s in sources {
        if s.parts.len() == 1
            && (s.parts[0].eq_ignore_ascii_case(&fl.snapshot.report)
                || fl
                    .frameit
                    .config_files
                    .iter()
                    .any(|f| f.eq_ignore_ascii_case(&s.parts[0])))
        {
            return SourceFormat::Fastlane;
        }
        // screengrab / supply: `<locale>/images/<type>/<file>`.
        if s.parts.windows(2).any(|w| {
            w[0].eq_ignore_ascii_case(&fl.supply.images_dir)
                && fl.supply.screenshot_type(&w[1]).is_some()
        }) {
            return SourceFormat::Fastlane;
        }
        let stem = s.stem();
        if s.parts.len() == 2 && names::looks_like_fastlane(&stem) {
            fastlane += 1;
        }
        if (2..=3).contains(&s.parts.len())
            && ctx.lang(&s.parts[0]).is_some()
            && slugify(&stem) == stem
        {
            vitrine += 1;
        }
    }
    if fastlane > 0 && fastlane >= vitrine {
        SourceFormat::Fastlane
    } else if vitrine > 0 {
        SourceFormat::Vitrine
    } else {
        SourceFormat::Flat
    }
}

fn detect_vitrine(
    ctx: &Ctx<'_>,
    src: &Source,
    stem: &str,
    size: Option<(u32, u32)>,
) -> Result<Detection, String> {
    let (lang_dir, variant_dir) = match src.parts.as_slice() {
        [l, _] => (l, None),
        [l, v, _] => (l, Some(v)),
        _ => return Err("not in <lang>/[<variant>/]<key>.png".to_owned()),
    };
    let mut d = Detection {
        locale: Some(lang_dir.clone()),
        screen: Some(stem.to_owned()),
        ..Detection::default()
    };
    d.lang = ctx.lang(lang_dir);
    if d.lang.is_none() {
        d.lang_problem = Some(format!("folder `{lang_dir}` is not a project language"));
    }
    if let Some(v) = variant_dir {
        let (_, _, candidates, _) = ctx.variant_by_size(size);
        d.candidates = candidates;
        d.variant = Some(v.clone());
        if ctx.known_variants.contains(v) {
            d.variant_conf = Some(Confidence::High);
            d.notes.push(format!("variant {v} (folder)"));
        } else {
            d.variant_conf = Some(Confidence::Medium);
            d.notes
                .push(format!("variant {v} (folder; not a known variant)"));
        }
    } else {
        d.notes
            .push("no variant folder: serves all variants".to_owned());
    }
    d.key = Some(ctx.key(stem, false));
    Ok(d)
}

fn detect_fastlane(
    ctx: &Ctx<'_>,
    src: &Source,
    stem: &str,
    size: Option<(u32, u32)>,
) -> Result<Detection, String> {
    let [locale, _] = src.parts.as_slice() else {
        return Err(if src.parts.len() == 1 {
            "not in a locale folder".to_owned()
        } else {
            "not in <locale>/<Device>-<screen>.png".to_owned()
        });
    };
    let known: Vec<&str> = ctx.mapping.devices.keys().map(String::as_str).collect();
    let Some((device, screen)) = split_fastlane(stem, &known, &ctx.prefixes) else {
        return Err("name is not <Device Name>-<screen>".to_owned());
    };
    let mut d = Detection {
        locale: Some(locale.clone()),
        device: Some(device.to_owned()),
        screen: Some(screen.to_owned()),
        ..Detection::default()
    };
    d.lang = ctx.lang(locale);
    if d.lang.is_none() {
        d.lang_problem = Some(format!(
            "locale `{locale}` matches no project language (map it)"
        ));
    }
    let mapped = ctx.mapped_device(device);
    let (_, _, candidates, _) = ctx.variant_by_size(size);
    if let Some(v) = mapped {
        d.notes
            .push(format!("device `{device}` → {v} (device map)"));
        d.variant = Some(v);
        d.variant_conf = Some(Confidence::High);
        d.candidates = candidates;
    } else {
        let (v, conf, candidates, note) = ctx.variant_by_size(size);
        d.candidates = candidates;
        match v {
            Some(v) => {
                d.notes.push(format!("device `{device}` → {note}"));
                d.variant = Some(v);
                d.variant_conf = Some(conf);
            }
            None => {
                d.variant_problem = Some(format!(
                    "unknown device `{device}` ({note}); map it to a variant"
                ));
            }
        }
    }
    d.key = Some(ctx.key(screen, true));
    Ok(d)
}

/// Skip reason of a file an export manifest lists.
pub const SKIP_OWN_EXPORT: &str =
    "exported by Vitrine Studio (listed in .vitrine-export.json), not a capture";
/// Skip reason of a file named like a fastlane-layout export.
pub const SKIP_EXPORT_NAME: &str =
    "named like a Vitrine Studio export (<nn>_<target> / <nn>_<locale>), not a capture";

/// A file of a fastlane folder named the way the fastlane export layout
/// names its images (`crate::fastlane::Placement::path`): deliver
/// `<nn>_<target>[_<marker>]` with a target of this project, supply
/// `<nn>_<locale>` with the file's own locale folder. Catches exports
/// whose manifest is gone; snapshot (`<Device>-<screen>`) and screengrab
/// (`<name>_<millis>`) do not name files like that.
fn looks_exported(project: &Project, route: &Route, parts: &[String], stem: &str) -> bool {
    let Some((nn, rest)) = stem.split_once('_') else {
        return false;
    };
    if nn.len() < 2 || !nn.chars().all(|c| c.is_ascii_digit()) || rest.is_empty() {
        return false;
    }
    match route {
        Route::Screengrab { locale, .. } => rest.eq_ignore_ascii_case(locale),
        Route::Snapshot => {
            parts.len() >= 2
                && project.targets.iter().any(|t| {
                    rest == t.id.as_str()
                        || rest
                            .strip_prefix(t.id.as_str())
                            .is_some_and(|m| m.starts_with('_'))
                })
        }
        Route::Skip(_) => false,
    }
}

/// Where a file of a fastlane folder belongs.
enum Route {
    /// snapshot / deliver: `<locale>/<Device Name>-<screen>.png`.
    Snapshot,
    /// screengrab / supply: `<locale>/images/<kind>/<name>.png`.
    Screengrab { locale: String, kind: String },
    /// Not a screenshot (reason).
    Skip(String),
}

/// Sorts a file of a fastlane folder (parts relative to the source, a
/// leading `fastlane/` and `screenshots/` already removed): screengrab's
/// `metadata/android/<locale>/images/<type>/…` (also below a picked
/// `metadata` or `android` folder), deliver metadata, iMessage / Apple TV
/// folders, Play store graphics.
fn fastlane_route(parts: &[String]) -> Route {
    let fl = catalog::fastlane();
    let images = fl.supply.images_dir.as_str();
    let meta: Vec<&str> = fl.supply.metadata_dir.split('/').collect();
    let eq = |a: &str, b: &str| a.eq_ignore_ascii_case(b);
    let mut p = parts;
    let mut android = false;
    if p.len() > meta.len() && p.iter().zip(&meta).all(|(a, b)| eq(a, b)) {
        p = &p[meta.len()..];
        android = true;
    } else if p.len() > 1 && meta.first().is_some_and(|m| eq(&p[0], m)) {
        return Route::Skip("fastlane metadata (not a screenshot)".to_owned());
    } else if p.len() > 3 && meta.last().is_some_and(|m| eq(&p[0], m)) && eq(&p[2], images) {
        p = &p[1..];
        android = true;
    }
    if p.len() >= 3 && eq(&p[1], images) {
        if p.len() == 3 {
            let stem = p[2].rsplit_once('.').map_or(p[2].as_str(), |(s, _)| s);
            return Route::Skip(match fl.supply.image_type(stem) {
                Some(t) => format!("Play store graphic `{t}` (not a screenshot)"),
                None => "not in images/<type>/".to_owned(),
            });
        }
        if p.len() == 4 {
            return match fl.supply.screenshot_type(&p[2]) {
                Some(kind) => Route::Screengrab {
                    locale: p[0].clone(),
                    kind: kind.to_owned(),
                },
                None => Route::Skip(format!("unknown Play image folder `{}`", p[2])),
            };
        }
        return Route::Skip("not in <locale>/images/<type>/".to_owned());
    }
    if android {
        return Route::Skip("fastlane metadata (not a screenshot)".to_owned());
    }
    // iMessage/<locale>/, appleTV/<locale>/ (deliver's `default/` folder is
    // a locale folder for the import: map it with the locale map).
    if p.len() > 1 && !eq(&p[0], "default") && fl.deliver.special_dirs.iter().any(|d| eq(d, &p[0]))
    {
        return Route::Skip(format!("`{}` screenshots are not imported", p[0]));
    }
    Route::Snapshot
}

/// The part of frameit's `Framefile.json` the import needs.
#[derive(Deserialize)]
struct Framefile {
    #[serde(default)]
    default: Option<FramefileEntry>,
    #[serde(default)]
    data: Option<Vec<FramefileEntry>>,
}

#[derive(Deserialize)]
struct FramefileEntry {
    #[serde(default)]
    background: Option<String>,
}

/// Files a `Framefile.json` of the source names as `background`
/// (relative to its folder), as source-relative paths.
fn frameit_backgrounds(sources: &[Source]) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for s in sources {
        if !s
            .parts
            .last()
            .is_some_and(|n| n.eq_ignore_ascii_case("Framefile.json"))
        {
            continue;
        }
        // JSON is YAML: read it with the YAML parser (no JSON dependency).
        let Some(file) = std::fs::read_to_string(&s.abs)
            .ok()
            .and_then(|t| crate::io::from_yaml::<Framefile>(&t).ok())
        else {
            continue;
        };
        let found = file
            .default
            .iter()
            .chain(file.data.iter().flatten())
            .filter_map(|e| e.background.as_deref());
        for b in found {
            let mut parts: Vec<&str> = s.parts[..s.parts.len() - 1]
                .iter()
                .map(String::as_str)
                .collect();
            for seg in b.split(['/', '\\']) {
                match seg {
                    "" | "." => {}
                    ".." => {
                        parts.pop();
                    }
                    seg => parts.push(seg),
                }
            }
            out.insert(parts.join("/"));
        }
    }
    out
}

/// `home_1731234567890` → `home` (screengrab's timestamp suffix).
fn strip_timestamp(stem: &str, min_digits: usize) -> &str {
    match stem.rsplit_once('_') {
        Some((head, tail))
            if !head.is_empty()
                && tail.len() >= min_digits
                && tail.chars().all(|c| c.is_ascii_digit()) =>
        {
            head
        }
        _ => stem,
    }
}

/// screengrab / supply file `<locale>/images/<kind>/<name>.png`.
fn detect_screengrab(
    ctx: &Ctx<'_>,
    locale: &str,
    kind: &str,
    stem: &str,
    size: Option<(u32, u32)>,
) -> Detection {
    let fl = catalog::fastlane();
    let screen = strip_timestamp(stem, fl.screengrab.timestamp_min_digits);
    let mut d = Detection {
        locale: Some(locale.to_owned()),
        screen: Some(screen.to_owned()),
        ..Detection::default()
    };
    d.lang = ctx.lang(locale);
    if d.lang.is_none() {
        d.lang_problem = Some(format!(
            "locale `{locale}` matches no project language (map it)"
        ));
    }
    let (sv, sconf, candidates, snote) = ctx.variant_by_size(size);
    d.candidates = candidates;
    if let Some(v) = ctx.mapped_device(kind) {
        d.device = Some(kind.to_owned());
        d.notes.push(format!("`{kind}` → {v} (device map)"));
        d.variant = Some(v);
        d.variant_conf = Some(Confidence::High);
    } else if let Some(v) = fl.supply.variant(kind) {
        d.notes.push(format!("variant {v} (Play folder {kind})"));
        d.variant = Some(v.to_owned());
        d.variant_conf = Some(Confidence::High);
    } else if let Some(v) = sv {
        d.device = Some(kind.to_owned());
        d.notes.push(format!("`{kind}` → {snote}"));
        d.variant = Some(v);
        d.variant_conf = Some(sconf);
    } else {
        d.variant_problem = Some(format!("`{kind}` ({snote}); map it to a variant"));
    }
    d.key = Some(ctx.key(screen, true));
    d
}

fn detect_flat(ctx: &Ctx<'_>, src: &Source, stem: &str, size: Option<(u32, u32)>) -> Detection {
    let mut d = Detection {
        screen: Some(stem.to_owned()),
        ..Detection::default()
    };
    let mut rest = stem.to_owned();

    // A mapped device name at the start (`iPhone 16 Pro Max-home`).
    let known: Vec<&str> = ctx.mapping.devices.keys().map(String::as_str).collect();
    let mut variant_from_name: Option<(String, String)> = None;
    if let Some((device, screen)) = split_fastlane(stem, &known, &DevicePrefixes::default())
        && let Some(v) = ctx.mapping.devices.get(device)
    {
        variant_from_name = Some((v.clone(), format!("device `{device}` → {v} (device map)")));
        d.device = Some(device.to_owned());
        screen.clone_into(&mut rest);
    }

    let mut toks = names::tokens(&rest);
    // Variant: a known variant id spelled in the name (`…_ios-phone_…`).
    if variant_from_name.is_none() {
        'outer: for v in &ctx.known_variants {
            let vt = names::tokens(v);
            if vt.is_empty() || vt.len() > toks.len() {
                continue;
            }
            for i in 0..=toks.len() - vt.len() {
                if toks[i..i + vt.len()]
                    .iter()
                    .zip(&vt)
                    .all(|(a, b)| a.eq_ignore_ascii_case(b))
                {
                    toks.drain(i..i + vt.len());
                    variant_from_name = Some((v.clone(), format!("variant {v} (file name)")));
                    break 'outer;
                }
            }
        }
    }
    // Language: a token pair (`de-DE`) or token, else a parent folder.
    'lang: for i in 0..toks.len() {
        if i + 1 < toks.len() {
            let pair = format!("{}-{}", toks[i], toks[i + 1]);
            if let Some(f) = ctx.lang(&pair) {
                d.locale = Some(pair);
                d.lang = Some(f);
                toks.drain(i..i + 2);
                break 'lang;
            }
        }
        if let Some(f) = ctx.lang(&toks[i]) {
            d.locale = Some(toks[i].clone());
            d.lang = Some(f);
            toks.remove(i);
            break 'lang;
        }
    }
    // Parent folders (`de/ios-phone/x.png` dropped as a tree).
    for folder in src.parts[..src.parts.len() - 1].iter().rev() {
        if d.lang.is_none()
            && let Some(f) = ctx.lang(folder)
        {
            d.locale = Some(folder.clone());
            d.lang = Some(f);
        } else if variant_from_name.is_none() && ctx.known_variants.contains(folder) {
            variant_from_name = Some((folder.clone(), format!("variant {folder} (folder)")));
        }
    }
    if d.lang.is_none() {
        let def = ctx.project.languages.default.to_string();
        d.lang = Some((
            def.clone(),
            Confidence::Low,
            format!("no language in the name: default language {def}"),
        ));
    }
    let (sv, sconf, candidates, snote) = ctx.variant_by_size(size);
    d.candidates = candidates;
    if let Some((v, note)) = variant_from_name {
        d.variant = Some(v);
        d.variant_conf = Some(Confidence::High);
        d.notes.push(note);
    } else if let Some(v) = sv {
        d.variant = Some(v);
        d.variant_conf = Some(sconf);
        d.notes.push(snote);
    } else {
        d.variant_conf = Some(Confidence::Low);
        d.notes.push(format!(
            "{snote}: stored in the language folder (all variants)"
        ));
    }
    let screen = if toks.is_empty() {
        rest.clone()
    } else {
        toks.join("-")
    };
    d.key = Some(ctx.key(&screen, true));
    d
}

/// Device/locale/key entries the plan relied on that are not the default
/// behaviour (so a later import maps the same way without detection).
fn learned_mapping(ctx: &Ctx<'_>, items: &[PlannedFile]) -> ImportMapping {
    let mut m = ImportMapping::default();
    let mut conflicting: BTreeSet<String> = BTreeSet::new();
    // Keys per screen name over all files: a row correction becomes a key
    // mapping only when every file of that screen name gets the same key.
    let mut screen_keys: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for it in items {
        if let (Some(screen), Some(key)) = (&it.screen, &it.key) {
            screen_keys.entry(screen).or_default().insert(key);
        }
    }
    for it in items {
        if it.action == Action::Unresolved {
            continue;
        }
        if let (Some(device), Some(v)) = (&it.device, &it.variant) {
            match m.devices.get(device) {
                Some(old) if old != v => {
                    conflicting.insert(device.clone());
                }
                _ => {
                    m.devices.insert(device.clone(), v.clone());
                }
            }
        }
        if let (Some(locale), Some(lang)) = (&it.locale, &it.lang)
            && locale != lang
            && let Ok(l) = Lang::parse(lang)
        {
            m.locales.insert(locale.clone(), l);
        }
        if let (Some(screen), Some(key)) = (&it.screen, &it.key)
            && (it.overridden || ctx.mapping.keys.contains_key(screen))
            && slugify(strip_numeric_prefix(screen)) != *key
            && screen_keys
                .get(screen.as_str())
                .is_some_and(|k| k.len() == 1)
        {
            m.keys.insert(screen.clone(), key.clone());
        }
    }
    for d in conflicting {
        m.devices.remove(&d);
    }
    m
}

/// Screenshot keys used by the device elements of each canvas (base
/// elements).
fn used_keys(project: &Project) -> BTreeMap<String, BTreeSet<String>> {
    let mut out: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for s in &project.canvases {
        let keys = out.entry(s.id.to_string()).or_default();
        s.walk_elements(&mut |e| {
            if let ElementKind::Device(d) = &e.kind
                && let Some(k) = &d.screenshot
            {
                keys.insert(k.clone());
            }
        });
    }
    out
}

/// Files on disk plus the destinations the plan writes.
struct PlannedAssets<'a> {
    fs: FsAssets,
    planned: &'a BTreeSet<String>,
}

impl AssetLookup for PlannedAssets<'_> {
    fn exists(&self, path: &str) -> bool {
        self.planned.contains(path) || self.fs.exists(path)
    }

    fn dir_exists(&self, path: &str) -> bool {
        self.planned.dir_exists(path) || self.fs.dir_exists(path)
    }
}

/// Used screenshots without an own file per language × target variant
/// after the import.
fn missing_after(
    project: &Project,
    project_dir: &Path,
    items: &[PlannedFile],
) -> Vec<MissingScreenshot> {
    let planned: BTreeSet<String> = items
        .iter()
        .filter(|i| i.action.copies())
        .filter_map(|i| i.dest.clone())
        .collect();
    let assets = PlannedAssets {
        fs: FsAssets::new(project_dir),
        planned: &planned,
    };
    let used = used_keys(project);
    // variant → keys of the canvases its targets show.
    let mut by_variant: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for t in &project.targets {
        if let Some(v) = &t.screenshots
            && let Some(keys) = used.get(t.canvas.as_str())
        {
            by_variant
                .entry(v.clone())
                .or_default()
                .extend(keys.iter().cloned());
        }
    }
    let mut out = Vec::new();
    for lang in &project.languages.available {
        let chain = lang_chain(project, lang.as_str());
        for (variant, keys) in &by_variant {
            for key in keys {
                let r = lookup_screenshot(&assets, &chain, Some(variant), key);
                let status = match r.source {
                    ScreenshotSource::Missing => MissingStatus::Missing,
                    ScreenshotSource::Fallback | ScreenshotSource::FallbackVariant => {
                        MissingStatus::Fallback
                    }
                    _ => continue,
                };
                out.push(MissingScreenshot {
                    lang: lang.to_string(),
                    variant: variant.clone(),
                    key: key.clone(),
                    status,
                    from: r.lang.map(|l| l.to_string()),
                });
            }
        }
    }
    out
}

fn same_content(a: &Path, b: &Path) -> bool {
    let (Ok(ma), Ok(mb)) = (std::fs::metadata(a), std::fs::metadata(b)) else {
        return false;
    };
    if ma.len() != mb.len() {
        return false;
    }
    matches!((std::fs::read(a), std::fs::read(b)), (Ok(x), Ok(y)) if x == y)
}

// ----- apply ---------------------------------------------------------------------------

/// Copies the files of `plan` that are [`Action::New`] or
/// [`Action::Overwrite`] into `project_dir` (temp file + rename per file;
/// sources are never modified). Files whose destination meanwhile has the
/// same content are reported as unchanged.
pub fn apply(plan: &ImportPlan, project_dir: &Path) -> ApplyReport {
    let mut report = ApplyReport::default();
    for it in &plan.items {
        let Some(dest) = it.dest.as_ref().filter(|_| it.action.copies()) else {
            if it.action == Action::Unchanged {
                if let Some(d) = &it.dest {
                    report.unchanged.push(d.clone());
                }
            } else {
                report.skipped += 1;
            }
            continue;
        };
        let abs = project_dir.join(dest);
        if abs.is_file() && same_content(&it.path, &abs) {
            report.unchanged.push(dest.clone());
            continue;
        }
        let result = (|| -> Result<(), String> {
            let bytes = std::fs::read(&it.path).map_err(|e| format!("{}: {e}", it.source))?;
            if let Some(dir) = abs.parent() {
                std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
            }
            write_atomic(&abs, &bytes).map_err(|e| e.to_string())
        })();
        match result {
            Ok(()) => report.copied.push(dest.clone()),
            Err(e) => report.failed.push((dest.clone(), e)),
        }
    }
    report
}
