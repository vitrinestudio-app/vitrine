#![allow(clippy::many_single_char_names, reason = "pixel geometry")]

//! Own frame packs (KONZEPT §4.2 item 3, ADR 0024): any PNG or SVG device
//! frame plus a small `frame.yaml`, in a project's `frames/<id>/` (travels
//! with the project) or app-wide in `<app data>/frames/packs/<id>/`.
//!
//! ```text
//! <pack>/frame.yaml          manifest (PackManifest)
//! <pack>/*.png | *.svg       frame images (screen transparent, or `screen:`)
//! compiled (cache)           bezel PNG + mask PNG per frame + index.yaml,
//!                            rebuilt when the pack changes (stamp):
//!   project: <project>/.cache/frames/<id>/
//!   app:     <app data>/frames/packs/<id>/.compiled/
//! ```
//!
//! A compiled pack is a frame source `pack:<id>` of the [`FrameIndex`]
//! with its own folder ([`FrameIndex::add_source`]); resolve, bezel
//! selection and rendering are the ones of imported vendor frames.
//! Devices use it with `frame: pack:<id>`.

use std::fs;
use std::hash::{Hash, Hasher};
use std::io::Read;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::analyze::{
    PxRect, RgbaImage, analyze, body_swatch, decode_png, encode_gray_png, encode_rgba_png, hex_rgb,
};
use super::rules::slug;
use super::store::{ANY_MODEL, FrameIndex, FrameSourceIndex, INDEX_FILE, InstalledFrame, today};
use crate::io::{from_yaml, to_yaml, write_atomic};
use crate::model::{Orientation, PixelSize};

/// Manifest file name of a frame pack.
pub const PACK_FILE: &str = "frame.yaml";
/// Manifest format id.
pub const PACK_FORMAT: &str = "vitrine-frame/1";
/// Prefix of frame pack sources in the frame index and in `frame:`.
pub const PACK_PREFIX: &str = "pack:";
/// App-wide packs: `<frames root>/packs/<id>/`.
pub const PACKS_DIR: &str = "packs";
/// Project packs: `<project>/frames/<id>/`.
pub const PROJECT_FRAMES_DIR: &str = "frames";
/// Compiled files of an app-wide pack (inside its folder).
pub const COMPILED_DIR: &str = ".compiled";
/// Default long side (px) SVG frames are rasterized to.
pub const SVG_LONG_SIDE: u32 = 3000;
/// Bumped when the compiled layout or the analysis changes.
const COMPILE_REVISION: u32 = 1;
const STAMP_FILE: &str = ".stamp";
/// Limits for imported archives / folders.
const MAX_PACK_FILES: usize = 500;
const MAX_PACK_BYTES: u64 = 512 << 20;

/// `frame.yaml`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PackManifest {
    /// `vitrine-frame/1`.
    pub format: String,
    /// Display name (`Clay phone`).
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    /// Licence of the images (free text or SPDX id, `CC0-1.0`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,
    /// Where the images come from (URL).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// Attribution line the licence asks for.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attribution: Option<String>,
    /// Catalog device models the frames stand for (`iphone-island`); empty
    /// = any device with `frame: pack:<id>` (a new device or size).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub models: Vec<String>,
    pub frames: Vec<PackFrame>,
}

/// One frame image of a pack.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PackFrame {
    /// PNG or SVG, relative to the pack folder.
    pub file: String,
    /// Default: from the image (wider than high = landscape).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub orientation: Option<Orientation>,
    /// Colour name (`Sand`); its id is the slug (`sand`). Default
    /// `Default`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    /// Size / variant name when a pack has several frames for one model ×
    /// colour × orientation (`Clay 6.3"`), picked like Apple's bezels
    /// (`bezel:`, target size, screenshot size, aspect). Default: the pack
    /// name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bezel: Option<String>,
    /// Overrides the pack's `models` for this frame.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub models: Option<Vec<String>>,
    /// Screen rectangle in image pixels (SVG: user units). Default:
    /// detected (the transparent area around the image centre).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub screen: Option<ScreenRect>,
    /// Screen corner radius (px / SVG units) with an explicit `screen`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub radius: Option<f64>,
    /// Screen mask image (PNG or SVG, same size as the frame; white /
    /// opaque = screen). Overrides `screen` / `radius` for the shape.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mask: Option<String>,
    /// Make the screen area of the frame transparent (for mock-ups whose
    /// screen is painted, e.g. black); needs `screen` or `mask`.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub cut_screen: bool,
    /// SVG only: long side of the rasterized frame in px (default 3000).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raster: Option<u32>,
}

/// `screen: {x, y, w, h}`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScreenRect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

/// Rasterizes SVG frames (implemented by `vitrine-render` with resvg; core
/// does not render).
pub trait SvgRasterizer: Sync {
    /// Renders `svg` so that its long side is `long_side` px; returns the
    /// image and the scale (px per SVG user unit).
    fn rasterize(&self, svg: &[u8], long_side: u32) -> Result<(RgbaImage, f64), String>;
}

/// Where a pack lives.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PackOrigin {
    /// `<project>/frames/<id>/` (travels with the project).
    Project,
    /// `<app data>/frames/packs/<id>/` (this computer).
    App,
}

/// A loaded (compiled) pack: what lists and the import report show.
#[derive(Debug, Clone, Serialize)]
pub struct PackInfo {
    pub id: String,
    /// `pack:<id>` (what `frame:` says).
    pub frame: String,
    pub origin: PackOrigin,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attribution: Option<String>,
    /// Models of the pack (empty = any device).
    pub models: Vec<String>,
    /// The pack folder.
    pub dir: PathBuf,
    /// Folder of the compiled files (`frames[].file` / `mask` are relative
    /// to it).
    pub compiled: PathBuf,
    pub frames: Vec<InstalledFrame>,
    /// Problems that dropped frames (or the whole pack).
    pub errors: Vec<String>,
    /// Sanity findings (unknown model, screen aspect …).
    pub warnings: Vec<String>,
    /// A project pack hides an app-wide pack with the same id.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub shadows_app_pack: bool,
}

impl PackInfo {
    pub fn index(&self) -> FrameSourceIndex {
        FrameSourceIndex {
            frames: self.frames.clone(),
        }
    }
}

/// Reads and checks `frame.yaml` of the pack in `dir`.
pub fn read_manifest(dir: &Path) -> Result<PackManifest, String> {
    let path = dir.join(PACK_FILE);
    let text = fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let m: PackManifest = from_yaml(&text).map_err(|e| format!("{PACK_FILE}: {e}"))?;
    if m.format != PACK_FORMAT {
        return Err(format!(
            "{PACK_FILE}: format `{}` is not supported (expected `{PACK_FORMAT}`)",
            m.format
        ));
    }
    if m.name.trim().is_empty() {
        return Err(format!("{PACK_FILE}: `name` is empty"));
    }
    if m.frames.is_empty() {
        return Err(format!("{PACK_FILE}: no `frames`"));
    }
    Ok(m)
}

/// A relative path inside the pack (no `..`, not absolute).
fn pack_file(dir: &Path, rel: &str) -> Result<PathBuf, String> {
    let p = Path::new(rel);
    if rel.is_empty()
        || p.components()
            .any(|c| !matches!(c, Component::Normal(_) | Component::CurDir))
    {
        return Err(format!("`{rel}`: not a file inside the pack"));
    }
    Ok(dir.join(p))
}

fn is_svg(rel: &str) -> bool {
    Path::new(rel)
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("svg"))
}

/// Stamp of the pack's inputs (manifest + referenced files' size/mtime).
fn stamp(dir: &Path, m: Option<&PackManifest>) -> String {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    COMPILE_REVISION.hash(&mut h);
    if let Ok(t) = fs::read(dir.join(PACK_FILE)) {
        t.hash(&mut h);
    }
    if let Some(m) = m {
        for f in &m.frames {
            for rel in std::iter::once(&f.file).chain(f.mask.as_ref()) {
                if let Ok(md) =
                    pack_file(dir, rel).and_then(|p| fs::metadata(p).map_err(|e| e.to_string()))
                {
                    md.len().hash(&mut h);
                    md.modified()
                        .ok()
                        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                        .map(|d| d.as_nanos())
                        .hash(&mut h);
                }
            }
        }
    }
    format!("{:016x}", h.finish())
}

/// What a compile keeps next to the files (`index.yaml` + messages).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CompiledState {
    stamp: String,
    #[serde(default)]
    errors: Vec<String>,
    #[serde(default)]
    warnings: Vec<String>,
}

/// Loads the pack in `dir` (id `id`), compiling it into `compiled` when its
/// inputs changed. Never fails: problems end up in `errors` (a pack with a
/// broken manifest has no frames).
pub fn load_pack(
    dir: &Path,
    id: &str,
    origin: PackOrigin,
    compiled: &Path,
    svg: Option<&dyn SvgRasterizer>,
) -> PackInfo {
    let manifest = read_manifest(dir);
    let mut info = PackInfo {
        id: id.to_owned(),
        frame: format!("{PACK_PREFIX}{id}"),
        origin,
        name: id.to_owned(),
        author: None,
        license: None,
        source: None,
        attribution: None,
        models: Vec::new(),
        dir: dir.to_owned(),
        compiled: compiled.to_owned(),
        frames: Vec::new(),
        errors: Vec::new(),
        warnings: Vec::new(),
        shadows_app_pack: false,
    };
    let m = match manifest {
        Ok(m) => m,
        Err(e) => {
            info.errors.push(e);
            return info;
        }
    };
    info.name.clone_from(&m.name);
    info.author.clone_from(&m.author);
    info.license.clone_from(&m.license);
    info.source.clone_from(&m.source);
    info.attribution.clone_from(&m.attribution);
    info.models.clone_from(&m.models);
    let stamp = stamp(dir, Some(&m));
    // Up to date?
    if let Ok(t) = fs::read_to_string(compiled.join(STAMP_FILE))
        && let Ok(state) = from_yaml::<CompiledState>(&t)
        && state.stamp == stamp
        && let Ok(ix) = fs::read_to_string(compiled.join(INDEX_FILE))
        && let Ok(ix) = from_yaml::<FrameSourceIndex>(&ix)
    {
        info.frames = ix.frames;
        info.errors = state.errors;
        info.warnings = state.warnings;
        return info;
    }
    compile(&m, dir, compiled, svg, &mut info);
    let state = CompiledState {
        stamp,
        errors: info.errors.clone(),
        warnings: info.warnings.clone(),
    };
    let index = info.index();
    let saved = to_yaml(&index)
        .map_err(|e| e.to_string())
        .and_then(|y| {
            write_atomic(&compiled.join(INDEX_FILE), y.as_bytes()).map_err(|e| e.to_string())
        })
        .and_then(|()| to_yaml(&state).map_err(|e| e.to_string()))
        .and_then(|y| {
            write_atomic(&compiled.join(STAMP_FILE), y.as_bytes()).map_err(|e| e.to_string())
        });
    if let Err(e) = saved {
        info.warnings.push(format!("compiled files not saved: {e}"));
    }
    info
}

/// The result of compiling one frame entry.
struct Compiled {
    bezel: Vec<u8>,
    mask: Vec<u8>,
    size: PixelSize,
    screen: PxRect,
    radius: f64,
    swatch: [u8; 3],
}

fn compile(
    m: &PackManifest,
    dir: &Path,
    out: &Path,
    svg: Option<&dyn SvgRasterizer>,
    info: &mut PackInfo,
) {
    // Fresh output folder (old frames of a changed pack must go).
    let _ = fs::remove_dir_all(out);
    if let Err(e) = fs::create_dir_all(out) {
        info.errors.push(format!("{}: {e}", out.display()));
        return;
    }
    let date = today();
    for (n, f) in m.frames.iter().enumerate() {
        let label = format!("frame {} (`{}`)", n + 1, f.file);
        let c = match compile_frame(f, dir, svg) {
            Ok(c) => c,
            Err(e) => {
                info.errors.push(format!("{label}: {e}"));
                continue;
            }
        };
        let orientation = f.orientation.unwrap_or(if c.size.w > c.size.h {
            Orientation::Landscape
        } else {
            Orientation::Portrait
        });
        let color_label = f.color.clone().unwrap_or_else(|| "Default".to_owned());
        let color = match slug(&color_label) {
            s if s.is_empty() => "default".to_owned(),
            s => s,
        };
        let bezel = f.bezel.clone().unwrap_or_else(|| m.name.clone());
        let stem = format!("{n:02}-{}", slug(&f.file));
        let (file, mask) = (format!("{stem}.png"), format!("{stem}.mask.png"));
        let written = write_atomic(&out.join(&file), &c.bezel)
            .and_then(|()| write_atomic(&out.join(&mask), &c.mask));
        if let Err(e) = written {
            info.errors.push(format!("{label}: {e}"));
            continue;
        }
        let models = f.models.clone().unwrap_or_else(|| m.models.clone());
        let models = if models.is_empty() {
            vec![ANY_MODEL.to_owned()]
        } else {
            models
        };
        for model in models {
            if model != ANY_MODEL {
                match crate::catalog::device(&model) {
                    None => {
                        info.errors
                            .push(format!("{label}: unknown device model `{model}`"));
                        continue;
                    }
                    Some(dev) => {
                        if let Some(w) = aspect_warning(dev, orientation, c.screen) {
                            info.warnings.push(format!("{label}: {w}"));
                        }
                    }
                }
            }
            info.frames.push(InstalledFrame {
                model,
                color: color.clone(),
                color_label: color_label.clone(),
                orientation,
                apple_model: bezel.clone(),
                file: file.clone(),
                mask: mask.clone(),
                size: c.size,
                screen: c.screen,
                radius: (c.radius * 10.0).round() / 10.0,
                priority: 0,
                source_file: f.file.clone(),
                package: m.name.clone(),
                license: m.license.clone(),
                imported: date.clone(),
                swatch: Some(hex_rgb(c.swatch)),
                origin: crate::frame_import::FrameOrigin::App,
            });
        }
    }
    // Two entries for the same slot: the later one wins (like a re-import).
    let mut seen: Vec<InstalledFrame> = Vec::new();
    for f in std::mem::take(&mut info.frames) {
        if let Some(i) = seen.iter().position(|s| s.same_slot(&f)) {
            info.warnings.push(format!(
                "`{}` and `{}` are the same model × colour × orientation × bezel; using `{}`",
                seen[i].source_file, f.source_file, f.source_file
            ));
            seen.remove(i);
        }
        seen.push(f);
    }
    info.frames = seen;
}

fn aspect_warning(
    dev: &crate::catalog::Device,
    orientation: Orientation,
    screen: PxRect,
) -> Option<String> {
    let aspect = dev.screenshot.aspect?;
    let mut expected = f64::from(aspect.w) / f64::from(aspect.h);
    if orientation != dev.natural {
        expected = 1.0 / expected;
    }
    let found = f64::from(screen.w) / f64::from(screen.h.max(1));
    let off = (found / expected - 1.0).abs();
    (off > super::ASPECT_TOLERANCE).then(|| {
        format!(
            "screen {}×{} px (aspect {found:.4}) differs by {:.1} % from `{}` ({expected:.4}); screenshots are cropped to fit",
            screen.w,
            screen.h,
            off * 100.0,
            dev.id
        )
    })
}

/// Reads one image of the pack (PNG, or SVG via `svg` at `long_side`);
/// returns it and the SVG scale (1 for PNGs).
fn read_image(
    dir: &Path,
    rel: &str,
    svg: Option<&dyn SvgRasterizer>,
    long_side: u32,
) -> Result<(RgbaImage, f64, Option<Vec<u8>>), String> {
    let path = pack_file(dir, rel)?;
    let bytes = fs::read(&path).map_err(|e| format!("{rel}: {e}"))?;
    if is_svg(rel) {
        let svg = svg.ok_or("SVG frames need the renderer (not available here)")?;
        let (img, k) = svg.rasterize(&bytes, long_side)?;
        Ok((img, k, None))
    } else {
        let img = decode_png(&bytes).map_err(|e| format!("{rel}: {e}"))?;
        Ok((img, 1.0, Some(bytes)))
    }
}

fn compile_frame(
    f: &PackFrame,
    dir: &Path,
    svg: Option<&dyn SvgRasterizer>,
) -> Result<Compiled, String> {
    let long_side = f.raster.unwrap_or(SVG_LONG_SIDE).clamp(256, 8192);
    let (mut img, k, original) = read_image(dir, &f.file, svg, long_side)?;
    let (w, h) = (img.width, img.height);
    if w < 8 || h < 8 {
        return Err(format!("image too small ({w}×{h})"));
    }
    let size = PixelSize::new(w, h);
    let (screen, radius, mask) = if let Some(mask_rel) = &f.mask {
        let (m, _, _) = read_image(dir, mask_rel, svg, w.max(h))?;
        if (m.width, m.height) != (w, h) {
            return Err(format!(
                "mask `{mask_rel}` is {}×{}, the frame {w}×{h}",
                m.width, m.height
            ));
        }
        let mask: Vec<u8> = m
            .data
            .as_chunks::<4>()
            .0
            .iter()
            .map(|p| {
                let luma =
                    (299 * u32::from(p[0]) + 587 * u32::from(p[1]) + 114 * u32::from(p[2])) / 1000;
                #[allow(clippy::cast_possible_truncation, reason = "≤ 255")]
                let v = (luma * u32::from(p[3]) / 255) as u8;
                v
            })
            .collect();
        let screen = bbox(&mask, w, h).ok_or_else(|| format!("mask `{mask_rel}` is empty"))?;
        (screen, f.radius.map_or(0.0, |r| r * k), mask)
    } else if let Some(r) = f.screen {
        let rect = (r.x * k, r.y * k, r.w * k, r.h * k);
        if rect.2 < 4.0 || rect.3 < 4.0 || rect.0 < 0.0 || rect.1 < 0.0 {
            return Err("`screen` is empty or negative".into());
        }
        if rect.0 + rect.2 > f64::from(w) + 0.5 || rect.1 + rect.3 > f64::from(h) + 0.5 {
            return Err(format!("`screen` reaches outside the {w}×{h} image"));
        }
        let radius = f.radius.unwrap_or(0.0).max(0.0) * k;
        let mask = rounded_rect_mask(w, h, rect, radius);
        #[allow(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "checked"
        )]
        let screen = PxRect {
            x: rect.0.round() as u32,
            y: rect.1.round() as u32,
            w: rect.2.round() as u32,
            h: rect.3.round() as u32,
        };
        (screen, radius, mask)
    } else {
        if f.cut_screen {
            return Err("`cut_screen` needs `screen` or `mask`".into());
        }
        let a = analyze(&img).map_err(|e| {
            format!("{e} (give the screen with `screen: {{x, y, w, h}}`, and `cut_screen: true` if it is painted)")
        })?;
        let radius = f.radius.map_or(a.radius, |r| r * k);
        (a.screen, radius, a.mask)
    };
    if f.cut_screen {
        for (p, &m) in img.data.as_chunks_mut::<4>().0.iter_mut().zip(&mask) {
            #[allow(clippy::cast_possible_truncation, reason = "≤ 255")]
            let a = (u32::from(p[3]) * (255 - u32::from(m)) / 255) as u8;
            p[3] = a;
        }
    }
    let swatch = body_swatch(&img, screen);
    // PNGs are kept byte for byte unless the screen was cut.
    let bezel = match original {
        Some(bytes) if !f.cut_screen => bytes,
        _ => encode_rgba_png(&img)?,
    };
    Ok(Compiled {
        bezel,
        mask: encode_gray_png(w, h, &mask)?,
        size,
        screen,
        radius,
        swatch,
    })
}

/// Bounding box of mask values ≥ 128.
fn bbox(mask: &[u8], w: u32, h: u32) -> Option<PxRect> {
    let (mut x0, mut y0, mut x1, mut y1) = (u32::MAX, u32::MAX, 0, 0);
    for y in 0..h {
        for x in 0..w {
            if mask[(y * w + x) as usize] >= 128 {
                x0 = x0.min(x);
                y0 = y0.min(y);
                x1 = x1.max(x);
                y1 = y1.max(y);
            }
        }
    }
    (x0 != u32::MAX).then(|| PxRect {
        x: x0,
        y: y0,
        w: x1 - x0 + 1,
        h: y1 - y0 + 1,
    })
}

/// Anti-aliased rounded rectangle (x, y, w, h, radius in px), 255 inside.
fn rounded_rect_mask(w: u32, h: u32, (rx, ry, rw, rh): (f64, f64, f64, f64), r: f64) -> Vec<u8> {
    let r = r.min(rw / 2.0).min(rh / 2.0).max(0.0);
    let (cx, cy) = (rx + rw / 2.0, ry + rh / 2.0);
    let (hx, hy) = (rw / 2.0 - r, rh / 2.0 - r);
    let mut out = vec![0u8; w as usize * h as usize];
    let y0 = (ry.floor().max(0.0)) as u32;
    let y1 = ((ry + rh).ceil() as u32).min(h);
    let x0 = (rx.floor().max(0.0)) as u32;
    let x1 = ((rx + rw).ceil() as u32).min(w);
    for y in y0..y1 {
        for x in x0..x1 {
            // Signed distance of the pixel centre to the rounded rect.
            let px = (f64::from(x) + 0.5 - cx).abs() - hx;
            let py = (f64::from(y) + 0.5 - cy).abs() - hy;
            let outside = px.max(0.0).hypot(py.max(0.0));
            let d = outside + px.max(py).min(0.0) - r;
            let cover = (0.5 - d).clamp(0.0, 1.0);
            #[allow(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "0..=255"
            )]
            let v = (cover * 255.0).round() as u8;
            out[(y * w + x) as usize] = v;
        }
    }
    out
}

/// Pack ids in `dir` (sub-folders with a `frame.yaml` and a valid id).
fn pack_dirs(dir: &Path) -> Vec<(String, PathBuf)> {
    let mut out: Vec<(String, PathBuf)> = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().join(PACK_FILE).is_file())
        .filter_map(|e| {
            let id = e.file_name().to_string_lossy().into_owned();
            (crate::model::is_valid_id(&id) && id != super::embed::EMBED_DIR)
                .then(|| (id, e.path()))
        })
        .collect();
    out.sort();
    out
}

/// Compiled folder of a project pack.
pub fn project_compiled_dir(project: &Path, id: &str) -> PathBuf {
    project.join(".cache").join(PROJECT_FRAMES_DIR).join(id)
}

/// All packs: app-wide ones (`<frames_root>/packs/`) and the project's
/// (`<project>/frames/`); a project pack hides an app pack with the same
/// id. Compiles what changed.
pub fn load_packs(
    frames_root: Option<&Path>,
    project: Option<&Path>,
    svg: Option<&dyn SvgRasterizer>,
) -> Vec<PackInfo> {
    let mut app: Vec<PackInfo> = Vec::new();
    if let Some(root) = frames_root {
        for (id, dir) in pack_dirs(&root.join(PACKS_DIR)) {
            let compiled = dir.join(COMPILED_DIR);
            app.push(load_pack(&dir, &id, PackOrigin::App, &compiled, svg));
        }
    }
    let mut out: Vec<PackInfo> = Vec::new();
    if let Some(project) = project {
        for (id, dir) in pack_dirs(&project.join(PROJECT_FRAMES_DIR)) {
            let compiled = project_compiled_dir(project, &id);
            let mut info = load_pack(&dir, &id, PackOrigin::Project, &compiled, svg);
            info.shadows_app_pack = app.iter().any(|a| a.id == id);
            out.push(info);
        }
    }
    out.extend(app);
    out
}

/// Adds the packs' frames to `index` as sources `pack:<id>` (the project
/// pack wins over an app pack with the same id).
pub fn add_packs(index: &mut FrameIndex, packs: &[PackInfo]) {
    for p in packs.iter().filter(|p| p.origin == PackOrigin::App) {
        index.add_source(&p.frame, &p.compiled, p.index());
    }
    for p in packs.iter().filter(|p| p.origin == PackOrigin::Project) {
        index.add_source(&p.frame, &p.compiled, p.index());
    }
}

// ----- import ----------------------------------------------------------------------------

/// Where [`import_pack`] puts a pack.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PackTarget {
    /// App-wide (`<frames root>/packs/<id>/`).
    App,
    /// Into the project (`<project>/frames/<id>/`).
    Project,
}

/// Result of [`import_pack`].
#[derive(Debug, Clone, Serialize)]
pub struct PackImport {
    pub id: String,
    pub target: PackTarget,
    /// A pack with this id is there already (it is replaced).
    pub replaces: bool,
    /// Files copied (or that would be).
    pub files: usize,
    /// The pack as compiled from the package (frames, errors, warnings).
    pub pack: PackInfo,
    /// `false` = dry run.
    pub applied: bool,
}

/// Is `path` a frame pack (a folder with `frame.yaml`, or a `.zip` holding
/// one)? Used to tell packs from vendor packages on import.
pub fn is_pack(path: &Path) -> bool {
    if path.is_dir() {
        return find_manifest_dir(path).is_some();
    }
    if path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("zip"))
        && let Ok(file) = fs::File::open(path)
        && let Ok(archive) = zip::ZipArchive::new(file)
    {
        return archive.file_names().any(|n| {
            n.is_ok_and(|n| {
                !n.starts_with("__MACOSX")
                    && n.rsplit('/').next() == Some(PACK_FILE)
                    && n.matches('/').count() <= 1
            })
        });
    }
    false
}

/// The folder holding `frame.yaml`: `dir` itself or its only sub-folder.
fn find_manifest_dir(dir: &Path) -> Option<PathBuf> {
    if dir.join(PACK_FILE).is_file() {
        return Some(dir.to_owned());
    }
    let subs: Vec<PathBuf> = fs::read_dir(dir)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.is_dir()
                && !p
                    .file_name()
                    .is_some_and(|n| n.to_string_lossy().starts_with(['.', '_']))
        })
        .collect();
    match subs.as_slice() {
        [one] if one.join(PACK_FILE).is_file() => Some(one.clone()),
        _ => None,
    }
}

/// Extracts a zip safely (enclosed names only, size and count limits)
/// into `to`.
fn extract_zip(path: &Path, to: &Path) -> Result<(), String> {
    let file = fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| format!("{}: {e}", path.display()))?;
    if archive.len() > MAX_PACK_FILES {
        return Err(format!(
            "{}: more than {MAX_PACK_FILES} files",
            path.display()
        ));
    }
    let mut total = 0u64;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
        let Some(rel) = entry.enclosed_name() else {
            return Err(format!(
                "unsafe path in the archive: `{}`",
                entry
                    .name()
                    .map(std::borrow::Cow::into_owned)
                    .unwrap_or_default()
            ));
        };
        if entry.is_dir() || rel.starts_with("__MACOSX") {
            continue;
        }
        total = total.saturating_add(entry.size());
        if total > MAX_PACK_BYTES {
            return Err(format!(
                "{}: larger than {} MB",
                path.display(),
                MAX_PACK_BYTES >> 20
            ));
        }
        let out = to.join(&rel);
        if let Some(parent) = out.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let mut buf = Vec::with_capacity(usize::try_from(entry.size()).unwrap_or(0));
        entry
            .by_ref()
            .take(MAX_PACK_BYTES)
            .read_to_end(&mut buf)
            .map_err(|e| e.to_string())?;
        fs::write(&out, buf).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Files of a pack folder to copy (relative, no hidden files), with limits.
fn pack_files(dir: &Path) -> Result<Vec<PathBuf>, String> {
    fn walk(
        base: &Path,
        dir: &Path,
        out: &mut Vec<PathBuf>,
        total: &mut u64,
    ) -> Result<(), String> {
        let mut entries: Vec<_> = fs::read_dir(dir)
            .map_err(|e| format!("{}: {e}", dir.display()))?
            .flatten()
            .collect();
        entries.sort_by_key(fs::DirEntry::file_name);
        for e in entries {
            let name = e.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') || name == "__MACOSX" {
                continue;
            }
            let ft = e.file_type().map_err(|e| e.to_string())?;
            if ft.is_dir() {
                walk(base, &e.path(), out, total)?;
            } else if ft.is_file() {
                *total += e.metadata().map_or(0, |m| m.len());
                if out.len() >= MAX_PACK_FILES || *total > MAX_PACK_BYTES {
                    return Err(format!(
                        "the pack is too large (at most {MAX_PACK_FILES} files, {} MB)",
                        MAX_PACK_BYTES >> 20
                    ));
                }
                if let Ok(rel) = e.path().strip_prefix(base) {
                    out.push(rel.to_owned());
                }
            }
        }
        Ok(())
    }
    let mut out = Vec::new();
    walk(dir, dir, &mut out, &mut 0)?;
    Ok(out)
}

/// Imports the pack at `path` (folder or `.zip`) as `id` (default: the
/// slug of its `name`) into `dest` (the app packs folder
/// `<frames root>/packs`, or `<project>/frames`). Dry run with
/// `apply == false` (compiled into a temporary folder only). The pack must
/// compile with at least one frame.
pub fn import_pack(
    path: &Path,
    dest: &Path,
    target: PackTarget,
    id: Option<&str>,
    apply: bool,
    svg: Option<&dyn SvgRasterizer>,
) -> Result<PackImport, String> {
    let tmp = std::env::temp_dir().join(format!(
        "vitrine-pack-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos())
    ));
    let result = import_in(path, dest, target, id, apply, svg, &tmp);
    let _ = fs::remove_dir_all(&tmp);
    result
}

#[allow(clippy::too_many_arguments)]
fn import_in(
    path: &Path,
    dest: &Path,
    target: PackTarget,
    id: Option<&str>,
    apply: bool,
    svg: Option<&dyn SvgRasterizer>,
    tmp: &Path,
) -> Result<PackImport, String> {
    let src_root = if path.is_dir() {
        path.to_owned()
    } else if path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("zip"))
    {
        let x = tmp.join("unzipped");
        extract_zip(path, &x)?;
        x
    } else {
        return Err(format!(
            "{}: a frame pack is a folder or a .zip with `{PACK_FILE}`",
            path.display()
        ));
    };
    let src = find_manifest_dir(&src_root)
        .ok_or_else(|| format!("{}: no `{PACK_FILE}` found", path.display()))?;
    let manifest = read_manifest(&src)?;
    let id = match id {
        Some(id) => id.to_owned(),
        None => slug(&manifest.name),
    };
    if !crate::model::is_valid_id(&id) {
        return Err(format!(
            "`{id}` is not a valid pack id (letters, digits, `-`)"
        ));
    }
    if id == super::embed::EMBED_DIR {
        return Err(format!(
            "`{id}` is reserved (stored vendor frames live in `frames/{id}/`); choose another id"
        ));
    }
    let files = pack_files(&src)?;
    let check = load_pack(&src, &id, PackOrigin::App, &tmp.join("compiled"), svg);
    if check.frames.is_empty() {
        return Err(format!(
            "the pack has no usable frame: {}",
            if check.errors.is_empty() {
                "no frames".to_owned()
            } else {
                check.errors.join("; ")
            }
        ));
    }
    let final_dir = dest.join(&id);
    let replaces = final_dir.join(PACK_FILE).is_file();
    let mut pack = check;
    if apply {
        let staging = dest.join(format!(".{id}.importing"));
        let _ = fs::remove_dir_all(&staging);
        for rel in &files {
            let to = staging.join(rel);
            if let Some(parent) = to.parent() {
                fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
            }
            fs::copy(src.join(rel), &to).map_err(|e| format!("{}: {e}", rel.display()))?;
        }
        if final_dir.exists() {
            fs::remove_dir_all(&final_dir).map_err(|e| format!("{}: {e}", final_dir.display()))?;
        }
        fs::rename(&staging, &final_dir).map_err(|e| format!("{}: {e}", final_dir.display()))?;
        pack = match target {
            PackTarget::App => load_pack(
                &final_dir,
                &id,
                PackOrigin::App,
                &final_dir.join(COMPILED_DIR),
                svg,
            ),
            PackTarget::Project => {
                // dest = <project>/frames
                let project = dest.parent().unwrap_or(dest);
                load_pack(
                    &final_dir,
                    &id,
                    PackOrigin::Project,
                    &project_compiled_dir(project, &id),
                    svg,
                )
            }
        };
    } else {
        pack.dir = final_dir;
        pack.origin = match target {
            PackTarget::App => PackOrigin::App,
            PackTarget::Project => PackOrigin::Project,
        };
    }
    Ok(PackImport {
        id,
        target,
        replaces,
        files: files.len(),
        pack,
        applied: apply,
    })
}

/// Removes the pack folder `<dir>/<id>` (and, for a project, its compiled
/// cache). Returns whether it existed.
pub fn remove_pack(packs_dir: &Path, id: &str, project: Option<&Path>) -> Result<bool, String> {
    if !crate::model::is_valid_id(id) {
        return Err(format!("`{id}` is not a pack id"));
    }
    let dir = packs_dir.join(id);
    if !dir.join(PACK_FILE).is_file() {
        return Ok(false);
    }
    fs::remove_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    if let Some(p) = project {
        let _ = fs::remove_dir_all(project_compiled_dir(p, id));
    }
    Ok(true)
}

#[cfg(test)]
mod tests;
