//! Project files: screenshots, assets and fonts (list, import, remove).
//!
//! Imports copy files into the project folder with sanitized names
//! (lowercase ASCII letters, digits, `-`, `_`; everything else becomes
//! `-`). Screenshots go to `screenshots/<lang>/<variant>/<key>.<ext>`; the
//! variant is detected from the pixel size via the device catalog when not
//! given (preferring variants the project's targets use).

use std::path::{Component, Path, PathBuf};

use arrangia_core::catalog;
use arrangia_core::model::{Lang, PixelSize};
use arrangia_core::resolve::SCREENSHOT_EXTENSIONS;
use serde::{Deserialize, Serialize};

use super::{AppState, NoArgs};
use crate::error::{CmdResult, CommandError};

/// Image extensions accepted in `assets/`.
pub const ASSET_EXTENSIONS: [&str; 4] = ["png", "jpg", "jpeg", "svg"];
/// Font extensions accepted in `fonts/`.
pub const FONT_EXTENSIONS: [&str; 3] = ["ttf", "otf", "ttc"];

/// Kind of an imported file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetKind {
    Screenshot,
    Asset,
    Font,
}

impl AssetKind {
    pub fn extensions(self) -> &'static [&'static str] {
        match self {
            Self::Screenshot => &SCREENSHOT_EXTENSIONS,
            Self::Asset => &ASSET_EXTENSIONS,
            Self::Font => &FONT_EXTENSIONS,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ScreenshotFile {
    /// Project-relative path (`/`-separated).
    pub path: String,
    pub lang: String,
    /// `None`: directly in the language folder (serves all variants).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variant: Option<String>,
    pub key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AssetFile {
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AssetsListing {
    pub screenshots: Vec<ScreenshotFile>,
    pub assets: Vec<AssetFile>,
    pub fonts: Vec<AssetFile>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssetsImportArgs {
    /// Absolute source files.
    pub paths: Vec<PathBuf>,
    pub kind: AssetKind,
    /// Screenshots: language folder (default: the project's default language).
    #[serde(default)]
    pub lang: Option<String>,
    /// Screenshots: variant folder (default: detected from the pixel size;
    /// `""` = directly in the language folder).
    #[serde(default)]
    pub variant: Option<String>,
    /// Screenshots: key (file name without extension) when importing one
    /// file (default: the sanitized file name).
    #[serde(default)]
    pub key: Option<String>,
    /// Replace an existing file with the same name instead of adding `-2`.
    #[serde(default)]
    pub replace: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImportedFile {
    pub source: PathBuf,
    /// Project-relative destination.
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variant: Option<String>,
    /// The variant was detected from the pixel size.
    pub detected: bool,
    /// All catalog variants matching the pixel size.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub candidates: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssetsRemoveArgs {
    /// Project-relative paths below `screenshots/`, `assets/` or `fonts/`.
    pub paths: Vec<String>,
}

/// Sanitized file-name stem: lowercase ASCII letters, digits, `-`, `_`.
pub fn sanitize(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for c in name.chars() {
        let c = c.to_ascii_lowercase();
        if c.is_ascii_alphanumeric() || c == '_' {
            out.push(c);
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    let out = out.trim_matches(['-', '_']).to_owned();
    if out.is_empty() {
        "file".to_owned()
    } else {
        out
    }
}

/// Screenshot variants (device catalog) whose typical sizes include
/// `w × h` in either orientation, in catalog order.
pub fn variants_for_size(w: u32, h: u32) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for d in catalog::devices() {
        let Some(v) = &d.screenshot.variant else {
            continue;
        };
        let hit = d
            .screenshot
            .sizes
            .iter()
            .any(|s| *s == PixelSize::new(w, h) || *s == PixelSize::new(h, w));
        if hit && !out.contains(v) {
            out.push(v.clone());
        }
    }
    out
}

fn dims(path: &Path) -> Option<(u32, u32)> {
    let s = imagesize::size(path).ok()?;
    Some((u32::try_from(s.width).ok()?, u32::try_from(s.height).ok()?))
}

fn rel(root: &Path, path: &Path) -> Option<String> {
    let r = path.strip_prefix(root).ok()?;
    let parts: Vec<String> = r
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect();
    Some(parts.join("/"))
}

fn ext_of(path: &Path) -> Option<String> {
    path.extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
}

/// Files below `dir` (recursive, sorted, hidden files skipped).
fn files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(rd) = std::fs::read_dir(dir) else {
        return out;
    };
    for e in rd.flatten() {
        let p = e.path();
        if e.file_name().to_string_lossy().starts_with('.') {
            continue;
        }
        if p.is_dir() {
            out.extend(files(&p));
        } else {
            out.push(p);
        }
    }
    out.sort();
    out
}

pub fn assets_list(app: &AppState, _: NoArgs) -> CmdResult<AssetsListing> {
    let session = app.session()?;
    let root = session.dir();
    let mut screenshots = Vec::new();
    for p in files(&root.join("screenshots")) {
        let Some(r) = rel(root, &p) else { continue };
        if !ext_of(&p).is_some_and(|e| SCREENSHOT_EXTENSIONS.contains(&e.as_str())) {
            continue;
        }
        let parts: Vec<&str> = r.split('/').collect();
        let (lang, variant) = match parts.as_slice() {
            [_, lang, _] => (*lang, None),
            [_, lang, variant, _] => (*lang, Some((*variant).to_owned())),
            _ => continue,
        };
        let key = p
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let d = dims(&p);
        screenshots.push(ScreenshotFile {
            lang: lang.to_owned(),
            variant,
            key,
            width: d.map(|d| d.0),
            height: d.map(|d| d.1),
            path: r,
        });
    }
    let list = |sub: &str, exts: &[&str]| -> Vec<AssetFile> {
        files(&root.join(sub))
            .into_iter()
            .filter(|p| ext_of(p).is_some_and(|e| exts.contains(&e.as_str())))
            .filter_map(|p| {
                let d = dims(&p);
                Some(AssetFile {
                    path: rel(root, &p)?,
                    width: d.map(|d| d.0),
                    height: d.map(|d| d.1),
                })
            })
            .collect()
    };
    Ok(AssetsListing {
        screenshots,
        assets: list("assets", &ASSET_EXTENSIONS),
        fonts: list("fonts", &FONT_EXTENSIONS)
            .into_iter()
            .map(|mut f| {
                f.width = None;
                f.height = None;
                f
            })
            .collect(),
    })
}

/// Free destination `dir/stem.ext` (`stem-2.ext` … unless `replace`).
fn destination(dir: &Path, stem: &str, ext: &str, replace: bool) -> PathBuf {
    let first = dir.join(format!("{stem}.{ext}"));
    if replace || !first.exists() {
        return first;
    }
    let mut n = 2u32;
    loop {
        let p = dir.join(format!("{stem}-{n}.{ext}"));
        if !p.exists() {
            return p;
        }
        n += 1;
    }
}

pub fn assets_import(app: &AppState, args: AssetsImportArgs) -> CmdResult<Vec<ImportedFile>> {
    let session = app.session()?;
    let root = session.dir().to_owned();
    if args.key.is_some() && args.paths.len() != 1 {
        return Err(CommandError::invalid("`key` needs exactly one path"));
    }
    let (lang, target_variants) = session.with_project(|p| {
        (
            args.lang
                .clone()
                .unwrap_or_else(|| p.languages.default.to_string()),
            p.targets
                .iter()
                .filter_map(|t| t.screenshots.clone())
                .collect::<Vec<_>>(),
        )
    });
    Lang::parse(&lang).map_err(CommandError::invalid)?;
    if let Some(v) = &args.variant
        && !v.is_empty()
        && sanitize(v) != *v
    {
        return Err(CommandError::invalid(format!("invalid variant `{v}`")));
    }
    let mut out = Vec::new();
    for src in &args.paths {
        if !src.is_file() {
            return Err(CommandError::not_found(format!(
                "`{}` is not a file",
                src.display()
            )));
        }
        let ext = ext_of(src).unwrap_or_default();
        let ext = if ext == "jpeg" { "jpg".to_owned() } else { ext };
        if !args.kind.extensions().contains(&ext.as_str()) {
            return Err(CommandError::invalid(format!(
                "`{}`: unsupported file type for {:?} (allowed: {})",
                src.display(),
                args.kind,
                args.kind.extensions().join(", ")
            )));
        }
        let stem = match &args.key {
            Some(k) => {
                if sanitize(k) != *k {
                    return Err(CommandError::invalid(format!(
                        "invalid key `{k}` (use lowercase letters, digits, - and _)"
                    )));
                }
                k.clone()
            }
            None => sanitize(&src.file_stem().unwrap_or_default().to_string_lossy()),
        };
        let size = dims(src);
        let (dir, variant, detected, candidates) = match args.kind {
            AssetKind::Screenshot => {
                let candidates = size.map_or_else(Vec::new, |(w, h)| variants_for_size(w, h));
                let (variant, detected) = match &args.variant {
                    Some(v) if v.is_empty() => (None, false),
                    Some(v) => (Some(v.clone()), false),
                    None => {
                        let pick = candidates
                            .iter()
                            .find(|c| target_variants.contains(c))
                            .or_else(|| candidates.first())
                            .cloned();
                        let detected = pick.is_some();
                        (pick, detected)
                    }
                };
                let mut dir = root.join("screenshots").join(&lang);
                if let Some(v) = &variant {
                    dir = dir.join(v);
                }
                (dir, variant, detected, candidates)
            }
            AssetKind::Asset => (root.join("assets"), None, false, Vec::new()),
            AssetKind::Font => (root.join("fonts"), None, false, Vec::new()),
        };
        std::fs::create_dir_all(&dir)
            .map_err(|e| CommandError::io(format!("{}: {e}", dir.display())))?;
        let dest = destination(&dir, &stem, &ext, args.replace);
        std::fs::copy(src, &dest).map_err(|e| {
            CommandError::io(format!(
                "cannot copy `{}` to `{}`: {e}",
                src.display(),
                dest.display()
            ))
        })?;
        out.push(ImportedFile {
            source: src.clone(),
            path: rel(&root, &dest).unwrap_or_default(),
            variant,
            detected,
            candidates,
            width: size.map(|s| s.0),
            height: size.map(|s| s.1),
        });
    }
    Ok(out)
}

/// Removed files (project-relative).
pub fn assets_remove(app: &AppState, args: AssetsRemoveArgs) -> CmdResult<Vec<String>> {
    let session = app.session()?;
    let root = session.dir();
    let mut removed = Vec::new();
    for r in &args.paths {
        let p = Path::new(r);
        let ok = p.components().all(|c| matches!(c, Component::Normal(_)))
            && matches!(
                p.components().next(),
                Some(Component::Normal(first)) if ["screenshots", "assets", "fonts"].iter().any(|d| first == *d)
            )
            && p.components().count() > 1;
        if !ok {
            return Err(CommandError::invalid(format!(
                "`{r}` is not a file below screenshots/, assets/ or fonts/"
            )));
        }
        let abs = root.join(p);
        if !abs.is_file() {
            return Err(CommandError::not_found(format!("`{r}` does not exist")));
        }
        std::fs::remove_file(&abs)
            .map_err(|e| CommandError::io(format!("cannot remove `{r}`: {e}")))?;
        removed.push(r.clone());
    }
    Ok(removed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitizes_names() {
        assert_eq!(sanitize("Home Screen (1)"), "home-screen-1");
        assert_eq!(sanitize("Über_Uns"), "ber_uns");
        assert_eq!(sanitize("  "), "file");
        assert_eq!(sanitize("plan--v2"), "plan-v2");
    }

    #[test]
    fn detects_variants_by_size() {
        let v = variants_for_size(1320, 2868);
        assert_eq!(v.first().map(String::as_str), Some("ios-phone"), "{v:?}");
        // Landscape screenshots of the same device match too.
        assert_eq!(variants_for_size(2868, 1320), v);
        assert_eq!(variants_for_size(17, 23), Vec::<String>::new());
    }
}
