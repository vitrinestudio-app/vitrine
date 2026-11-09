//! Project files: screenshots, assets and fonts (list, import, remove).
//!
//! Imports copy files into the project folder with sanitized names
//! (lowercase ASCII letters, digits, `-`, `_`; everything else becomes
//! `-`). Screenshots go to `screenshots/<lang>/<variant>/<key>.<ext>`; the
//! variant is detected from the pixel size via the device catalog when not
//! given (preferring variants the project's targets use).

use std::collections::{BTreeMap, HashMap};
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::SystemTime;

use arrangia_core::Project;
use arrangia_core::catalog;
use arrangia_core::model::{ElementKind, Fill, Lang, PixelSize};
use arrangia_core::resolve::SCREENSHOT_EXTENSIONS;
use arrangia_render::raster::{Filter, SourceImage};
use arrangia_render::scene::Resample;
use serde::{Deserialize, Serialize};

use super::{AppState, NoArgs};
use crate::error::{CmdResult, CommandError, ErrorCode};
use crate::preview::{PixelFormat, PreviewImage};

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
    /// Screenshot key → device elements using it (base elements, all strips).
    pub screenshot_keys: BTreeMap<String, Vec<String>>,
    /// Asset path → elements (image, background) using it; strip-wide
    /// backgrounds are listed as `<strip>:background`.
    pub asset_usage: BTreeMap<String, Vec<String>>,
    /// Screenshot variants named by targets (in target order, unique).
    pub variants: Vec<String>,
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
    let (screenshot_keys, asset_usage, variants) = session.with_project(usage);
    Ok(AssetsListing {
        screenshot_keys,
        asset_usage,
        variants,
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

type Usage = (
    BTreeMap<String, Vec<String>>,
    BTreeMap<String, Vec<String>>,
    Vec<String>,
);

/// Which elements use which screenshot keys and assets, and the screenshot
/// variants of the targets.
fn usage(p: &Project) -> Usage {
    let mut keys: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut assets: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let fill = |f: &Fill, who: String, assets: &mut BTreeMap<String, Vec<String>>| {
        if let Fill::Image { asset, .. } = f {
            assets.entry(asset.clone()).or_default().push(who);
        }
    };
    for s in &p.strips {
        for b in &s.backgrounds {
            fill(b, format!("{}:background", s.id), &mut assets);
        }
        s.walk_elements(&mut |e| match &e.kind {
            ElementKind::Device(d) => {
                if let Some(k) = &d.screenshot {
                    keys.entry(k.clone()).or_default().push(e.id.to_string());
                }
            }
            ElementKind::Image(i) => {
                assets
                    .entry(i.asset.clone())
                    .or_default()
                    .push(e.id.to_string());
            }
            ElementKind::Background(b) => fill(&b.fill, e.id.to_string(), &mut assets),
            _ => {}
        });
    }
    let mut variants: Vec<String> = Vec::new();
    for t in &p.targets {
        if let Some(v) = &t.screenshots
            && !variants.contains(v)
        {
            variants.push(v.clone());
        }
    }
    (keys, assets, variants)
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

// ----- probe (variant detection before importing) ------------------------------

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssetsProbeArgs {
    /// Absolute files to inspect.
    #[serde(default)]
    pub paths: Vec<PathBuf>,
    /// Pixel sizes `[w, h]` (files the UI only has as bytes).
    #[serde(default)]
    pub sizes: Vec<[u32; 2]>,
}

/// What an import would detect for one file or size.
#[derive(Debug, Clone, Serialize)]
pub struct ProbedFile {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
    /// The variant `assets_import` would pick (targets' variants first).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variant: Option<String>,
    /// All catalog variants matching the pixel size.
    pub candidates: Vec<String>,
}

/// Variant detection for files (`paths`) or pixel sizes (`sizes`), in
/// that order, without copying anything.
pub fn assets_probe(app: &AppState, args: AssetsProbeArgs) -> CmdResult<Vec<ProbedFile>> {
    let session = app.session()?;
    let target_variants: Vec<String> = session.with_project(|p| {
        p.targets
            .iter()
            .filter_map(|t| t.screenshots.clone())
            .collect()
    });
    let probe = |path: Option<PathBuf>, size: Option<(u32, u32)>| {
        let candidates = size.map_or_else(Vec::new, |(w, h)| variants_for_size(w, h));
        let variant = candidates
            .iter()
            .find(|c| target_variants.contains(c))
            .or_else(|| candidates.first())
            .cloned();
        ProbedFile {
            path,
            width: size.map(|s| s.0),
            height: size.map(|s| s.1),
            variant,
            candidates,
        }
    };
    let mut out: Vec<ProbedFile> = args
        .paths
        .iter()
        .map(|p| probe(Some(p.clone()), dims(p)))
        .collect();
    out.extend(args.sizes.iter().map(|[w, h]| probe(None, Some((*w, *h)))));
    Ok(out)
}

// ----- upload (files the UI only has as bytes, e.g. browser drag & drop) ------

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssetsUploadArgs {
    /// Original file name (its stem is the default key, its extension the type).
    pub name: String,
    /// File content, base64 (standard alphabet, padding optional).
    pub data: String,
    pub kind: AssetKind,
    #[serde(default)]
    pub lang: Option<String>,
    #[serde(default)]
    pub variant: Option<String>,
    #[serde(default)]
    pub key: Option<String>,
    #[serde(default)]
    pub replace: bool,
}

/// Decodes standard base64 (whitespace and padding tolerated).
pub fn decode_base64(text: &str) -> Option<Vec<u8>> {
    fn value(c: u8) -> Option<u32> {
        Some(u32::from(match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            _ => return None,
        }))
    }
    let mut out = Vec::with_capacity(text.len() / 4 * 3);
    let (mut acc, mut bits) = (0u32, 0u32);
    for &c in text.as_bytes() {
        if c == b'=' || c.is_ascii_whitespace() {
            continue;
        }
        acc = (acc << 6) | value(c)?;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    Some(out)
}

/// Like `assets_import`, for one file given as bytes (the dev bridge in a
/// browser has no file paths). The bytes are written to a temporary file
/// first; `source` in the result is the given `name`.
pub fn assets_upload(app: &AppState, args: AssetsUploadArgs) -> CmdResult<Vec<ImportedFile>> {
    let bytes =
        decode_base64(&args.data).ok_or_else(|| CommandError::invalid("`data` is not base64"))?;
    let name = Path::new(&args.name)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .filter(|n| !n.starts_with('.'))
        .ok_or_else(|| CommandError::invalid(format!("invalid file name `{}`", args.name)))?;
    let dir = std::env::temp_dir().join(format!(
        "arrangia-upload-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos())
    ));
    std::fs::create_dir_all(&dir)
        .map_err(|e| CommandError::io(format!("{}: {e}", dir.display())))?;
    let tmp = dir.join(&name);
    let result = std::fs::write(&tmp, &bytes)
        .map_err(|e| CommandError::io(format!("{}: {e}", tmp.display())))
        .and_then(|()| {
            assets_import(
                app,
                AssetsImportArgs {
                    paths: vec![tmp.clone()],
                    kind: args.kind,
                    lang: args.lang,
                    variant: args.variant,
                    key: args.key,
                    replace: args.replace,
                },
            )
        });
    let _ = std::fs::remove_dir_all(&dir);
    let mut out = result?;
    for f in &mut out {
        f.source = PathBuf::from(&name);
    }
    Ok(out)
}

// ----- thumbnails ------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssetThumbnailArgs {
    /// Project-relative path below `screenshots/` or `assets/`.
    pub path: String,
    /// Longer side in px (default 160, at most 1024).
    #[serde(default)]
    pub max_edge: Option<u32>,
}

/// Default and largest thumbnail edge (px).
pub const THUMBNAIL_EDGE: u32 = 160;
const THUMBNAIL_MAX_EDGE: u32 = 1024;
/// Thumbnails kept in memory (oldest dropped first).
const THUMBNAIL_CACHE_LEN: usize = 512;

type ThumbKey = (PathBuf, Option<SystemTime>, u64, u32);

/// Encoded thumbnails plus their insertion order (oldest first).
type ThumbCache = Mutex<(HashMap<ThumbKey, Arc<PreviewImage>>, Vec<ThumbKey>)>;

/// Encoded thumbnails keyed by (absolute path, mtime, file length, edge).
fn thumbnail_cache() -> &'static ThumbCache {
    static CACHE: std::sync::OnceLock<ThumbCache> = std::sync::OnceLock::new();
    CACHE.get_or_init(|| Mutex::new((HashMap::new(), Vec::new())))
}

/// A small PNG of a screenshot or image asset (aspect kept), cached by
/// path, modification time and size. Raster files are decoded on their
/// own (not kept in the render cache); SVGs go through the render
/// context's asset cache.
pub fn asset_thumbnail(app: &AppState, args: AssetThumbnailArgs) -> CmdResult<Arc<PreviewImage>> {
    let session = app.session()?;
    let p = Path::new(&args.path);
    let ok = p.components().all(|c| matches!(c, Component::Normal(_)))
        && matches!(
            p.components().next(),
            Some(Component::Normal(first)) if first == "screenshots" || first == "assets"
        );
    if !ok {
        return Err(CommandError::invalid(format!(
            "`{}` is not a file below screenshots/ or assets/",
            args.path
        )));
    }
    let abs = session.dir().join(p);
    let meta = std::fs::metadata(&abs)
        .map_err(|e| CommandError::not_found(format!("`{}`: {e}", args.path)))?;
    let edge = args
        .max_edge
        .unwrap_or(THUMBNAIL_EDGE)
        .clamp(16, THUMBNAIL_MAX_EDGE);
    let key: ThumbKey = (abs.clone(), meta.modified().ok(), meta.len(), edge);
    let cache = thumbnail_cache();
    if let Some(hit) = cache
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .0
        .get(&key)
    {
        return Ok(hit.clone());
    }
    let fit = |w: f64, h: f64| -> (u32, u32) {
        let k = (f64::from(edge) / w.max(h)).min(1.0);
        (
            ((w * k).round() as u32).max(1),
            ((h * k).round() as u32).max(1),
        )
    };
    let render_err = |e: String| CommandError::new(ErrorCode::Render, e);
    let pixmap = if ext_of(&abs).as_deref() == Some("svg") {
        let assets = session.render_context();
        let assets = assets.assets();
        let (w, h) = assets
            .natural_size(&args.path)
            .map_err(|e| render_err(e.to_string()))?;
        let (tw, th) = fit(w.max(1.0), h.max(1.0));
        let pm = assets
            .pixmap(&args.path, tw, th, Resample::Stretch, Filter::Bilinear)
            .map_err(|e| render_err(e.to_string()))?;
        (*pm).clone()
    } else {
        let img = SourceImage::load(&abs).map_err(|e| render_err(format!("{}: {e}", args.path)))?;
        let (tw, th) = fit(f64::from(img.width), f64::from(img.height));
        img.resample(tw, th, None, Filter::Bilinear)
            .map_err(|e| render_err(e.to_string()))?
    };
    let image = Arc::new(PreviewImage::encode(&pixmap, None, PixelFormat::Png)?);
    let mut guard = cache.lock().unwrap_or_else(PoisonError::into_inner);
    let (map, order) = &mut *guard;
    map.insert(key.clone(), image.clone());
    order.push(key);
    if order.len() > THUMBNAIL_CACHE_LEN {
        let old = order.remove(0);
        map.remove(&old);
    }
    Ok(image)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_base64() {
        assert_eq!(decode_base64("aGVsbG8=").unwrap(), b"hello");
        assert_eq!(decode_base64("aGVsbG8").unwrap(), b"hello");
        assert_eq!(decode_base64("AAEC\n/w==").unwrap(), [0, 1, 2, 255]);
        assert!(decode_base64("a*b").is_none());
    }

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
