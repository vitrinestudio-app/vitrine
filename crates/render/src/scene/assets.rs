//! Asset cache: decoded images (PNG/JPEG via `image`, converted to sRGB
//! once at decode – ADR 0021; SVG via `usvg`) and
//! pixmaps resampled to exact pixel sizes, keyed by (path, mtime, size,
//! mode). Shared between threads (`Mutex`-guarded maps, `Arc` values), so
//! export workers and repeated previews decode and resample each asset once.

use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::SystemTime;

use resvg::usvg;
use tiny_skia::{Color, IntSize, Pixmap, PixmapPaint, Transform};

use crate::raster::{Filter, SourceImage};

/// Largest side of a resampled pixmap (guards against absurd cover scales).
pub const MAX_PIXMAP_SIDE: u32 = 16_384;

/// Budget of the resampled-pixmap cache; exceeding it clears the cache.
const SCALED_BUDGET_BYTES: usize = 768 * 1024 * 1024;

/// A decoded asset.
#[derive(Debug)]
pub enum Decoded {
    Raster(SourceImage),
    Svg(Box<usvg::Tree>),
}

impl Decoded {
    /// Natural size: pixels for raster images, user units for SVG.
    pub fn size(&self) -> (f64, f64) {
        match self {
            Self::Raster(r) => (f64::from(r.width), f64::from(r.height)),
            Self::Svg(t) => (f64::from(t.size().width()), f64::from(t.size().height())),
        }
    }
}

/// How a decoded asset is mapped onto the requested pixel size.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Resample {
    /// Distort to exactly `w × h`.
    Stretch,
    /// Crop to the target aspect around the focus (fractions), then scale.
    Cover { focus: (f64, f64) },
    /// Scale to fit inside, centred, on an opaque background colour.
    ContainOn([u8; 4]),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct ScaledKey {
    path: String,
    mtime: Option<SystemTime>,
    w: u32,
    h: u32,
    mode: [u32; 3],
    filter: Filter,
}

fn mode_key(m: Resample) -> [u32; 3] {
    match m {
        Resample::Stretch => [0, 0, 0],
        Resample::Cover { focus } => [1, (focus.0 * 1e4) as u32, (focus.1 * 1e4) as u32],
        Resample::ContainOn(c) => [2, u32::from_le_bytes(c), 0],
    }
}

/// Why an asset could not be used.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum AssetError {
    #[error("asset path `{0}` must be relative to the project folder")]
    InvalidPath(String),
    #[error("cannot read `{path}`: {message}")]
    Read { path: String, message: String },
    #[error("cannot decode `{path}`: {message}")]
    Decode { path: String, message: String },
}

struct DecodedEntry {
    mtime: Option<SystemTime>,
    data: Arc<Decoded>,
    /// Raster image without any transparent pixel (SVGs never count).
    opaque: bool,
}

/// Decoded and resampled assets below one project folder.
pub struct AssetCache {
    root: PathBuf,
    decoded: Mutex<HashMap<String, DecodedEntry>>,
    scaled: Mutex<(HashMap<ScaledKey, Arc<Pixmap>>, usize)>,
}

impl std::fmt::Debug for AssetCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AssetCache")
            .field("root", &self.root)
            .finish_non_exhaustive()
    }
}

impl AssetCache {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            decoded: Mutex::new(HashMap::new()),
            scaled: Mutex::new((HashMap::new(), 0)),
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Absolute path of a project-relative asset path (no `..`, not absolute).
    pub fn resolve_path(&self, rel: &str) -> Result<PathBuf, AssetError> {
        let p = Path::new(rel);
        if rel.is_empty()
            || p.components()
                .any(|c| !matches!(c, Component::Normal(_) | Component::CurDir))
        {
            return Err(AssetError::InvalidPath(rel.to_owned()));
        }
        Ok(self.root.join(p))
    }

    /// Modification time of an asset (`None` if unavailable).
    pub fn mtime(&self, rel: &str) -> Option<SystemTime> {
        let path = self.resolve_path(rel).ok()?;
        std::fs::metadata(path).and_then(|m| m.modified()).ok()
    }

    /// Decoded asset, re-read when its mtime changed.
    pub fn decoded(&self, rel: &str) -> Result<(Arc<Decoded>, Option<SystemTime>), AssetError> {
        let path = self.resolve_path(rel)?;
        let mtime = std::fs::metadata(&path)
            .map_err(|e| AssetError::Read {
                path: rel.to_owned(),
                message: e.to_string(),
            })?
            .modified()
            .ok();
        if let Some(e) = lock(&self.decoded).get(rel)
            && e.mtime == mtime
        {
            return Ok((e.data.clone(), mtime));
        }
        let data = Arc::new(decode(&path, rel)?);
        let opaque = match &*data {
            Decoded::Raster(r) => r.rgba.as_chunks::<4>().0.iter().all(|p| p[3] == 255),
            Decoded::Svg(_) => false,
        };
        lock(&self.decoded).insert(
            rel.to_owned(),
            DecodedEntry {
                mtime,
                data: data.clone(),
                opaque,
            },
        );
        Ok((data, mtime))
    }

    /// Is the asset a raster image without transparent pixels? Its shadow
    /// can then be drawn from its rectangle (`false` for SVGs, images with
    /// alpha and assets that cannot be decoded).
    pub fn is_opaque(&self, rel: &str) -> bool {
        if self.decoded(rel).is_err() {
            return false;
        }
        lock(&self.decoded).get(rel).is_some_and(|e| e.opaque)
    }

    /// Natural size of an asset (px or SVG user units).
    pub fn natural_size(&self, rel: &str) -> Result<(f64, f64), AssetError> {
        Ok(self.decoded(rel)?.0.size())
    }

    /// The asset at exactly `w × h` px (premultiplied), cached.
    pub fn pixmap(
        &self,
        rel: &str,
        w: u32,
        h: u32,
        mode: Resample,
        filter: Filter,
    ) -> Result<Arc<Pixmap>, AssetError> {
        let (w, h) = (w.clamp(1, MAX_PIXMAP_SIDE), h.clamp(1, MAX_PIXMAP_SIDE));
        let (data, mtime) = self.decoded(rel)?;
        let key = ScaledKey {
            path: rel.to_owned(),
            mtime,
            w,
            h,
            mode: mode_key(mode),
            filter,
        };
        if let Some(p) = lock(&self.scaled).0.get(&key) {
            return Ok(p.clone());
        }
        let pm = Arc::new(
            scale(&data, w, h, mode, filter).map_err(|e| AssetError::Decode {
                path: rel.to_owned(),
                message: e.to_string(),
            })?,
        );
        let mut cache = lock(&self.scaled);
        let bytes = pm.data().len();
        if cache.1 + bytes > SCALED_BUDGET_BYTES {
            cache.0.clear();
            cache.1 = 0;
        }
        cache.1 += bytes;
        cache.0.insert(key, pm.clone());
        Ok(pm)
    }

    /// Drops all cached data (e.g. after a project switch).
    pub fn clear(&self) {
        lock(&self.decoded).clear();
        let mut s = lock(&self.scaled);
        s.0.clear();
        s.1 = 0;
    }
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

fn decode(path: &Path, rel: &str) -> Result<Decoded, AssetError> {
    let read_err = |e: std::io::Error| AssetError::Read {
        path: rel.to_owned(),
        message: e.to_string(),
    };
    let dec_err = |message: String| AssetError::Decode {
        path: rel.to_owned(),
        message,
    };
    let is_svg = path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("svg"));
    if is_svg {
        let bytes = std::fs::read(path).map_err(read_err)?;
        let tree = usvg::Tree::from_data(&bytes, &usvg::Options::default())
            .map_err(|e| dec_err(e.to_string()))?;
        Ok(Decoded::Svg(Box::new(tree)))
    } else {
        let bytes = std::fs::read(path).map_err(read_err)?;
        SourceImage::decode(&bytes)
            .map(Decoded::Raster)
            .map_err(|e| dec_err(e.to_string()))
    }
}

/// Size of `(sw, sh)` scaled to fit inside `(w, h)`, at least 1 px.
fn contain_size(sw: f64, sh: f64, w: u32, h: u32) -> (u32, u32) {
    let k = (f64::from(w) / sw).min(f64::from(h) / sh);
    (
        (sw * k).round().clamp(1.0, f64::from(w)) as u32,
        (sh * k).round().clamp(1.0, f64::from(h)) as u32,
    )
}

fn scale(data: &Decoded, w: u32, h: u32, mode: Resample, filter: Filter) -> anyhow::Result<Pixmap> {
    match (data, mode) {
        (Decoded::Raster(r), Resample::Stretch) => r.resample(w, h, None, filter),
        (Decoded::Raster(r), Resample::Cover { focus }) => r.resample(w, h, Some(focus), filter),
        (Decoded::Raster(r), Resample::ContainOn(bg)) => {
            let (cw, ch) = contain_size(f64::from(r.width), f64::from(r.height), w, h);
            let inner = r.resample(cw, ch, None, filter)?;
            Ok(pad(&inner, w, h, bg))
        }
        (Decoded::Svg(tree), mode) => {
            let (sw, sh) = (
                f64::from(tree.size().width()),
                f64::from(tree.size().height()),
            );
            let mut pm = Pixmap::new(w, h).ok_or_else(|| anyhow::anyhow!("bad size"))?;
            let (fw, fh) = (f64::from(w), f64::from(h));
            let ts = match mode {
                Resample::Stretch => Transform::from_scale((fw / sw) as f32, (fh / sh) as f32),
                Resample::Cover { focus } => {
                    let k = (fw / sw).max(fh / sh);
                    Transform::from_row(
                        k as f32,
                        0.0,
                        0.0,
                        k as f32,
                        ((fw - sw * k) * focus.0) as f32,
                        ((fh - sh * k) * focus.1) as f32,
                    )
                }
                Resample::ContainOn(bg) => {
                    pm.fill(Color::from_rgba8(bg[0], bg[1], bg[2], bg[3]));
                    let k = (fw / sw).min(fh / sh);
                    Transform::from_row(
                        k as f32,
                        0.0,
                        0.0,
                        k as f32,
                        ((fw - sw * k) / 2.0) as f32,
                        ((fh - sh * k) / 2.0) as f32,
                    )
                }
            };
            resvg::render(tree, ts, &mut pm.as_mut());
            Ok(pm)
        }
    }
}

/// `inner` centred on a `w × h` pixmap filled with `bg`.
fn pad(inner: &Pixmap, w: u32, h: u32, bg: [u8; 4]) -> Pixmap {
    let mut pm = Pixmap::new(w, h).unwrap_or_else(|| {
        Pixmap::from_vec(vec![0; 4], IntSize::from_wh(1, 1).expect("1x1")).expect("1x1")
    });
    pm.fill(Color::from_rgba8(bg[0], bg[1], bg[2], bg[3]));
    let x = (w.saturating_sub(inner.width()) / 2).cast_signed();
    let y = (h.saturating_sub(inner.height()) / 2).cast_signed();
    pm.draw_pixmap(
        x,
        y,
        inner.as_ref(),
        &PixmapPaint::default(),
        Transform::identity(),
        None,
    );
    pm
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_escaping_paths() {
        let c = AssetCache::new("/tmp/x");
        assert!(c.resolve_path("../etc/passwd").is_err());
        assert!(c.resolve_path("/etc/passwd").is_err());
        assert!(c.resolve_path("").is_err());
        assert!(c.resolve_path("assets/a.png").is_ok());
    }

    #[test]
    fn caches_and_decodes_svg_and_png() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("a.svg"),
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10"><rect width="20" height="10" fill="#ff0000"/></svg>"##,
        )
        .unwrap();
        let mut pm = Pixmap::new(8, 4).unwrap();
        pm.fill(Color::from_rgba8(0, 0, 255, 255));
        pm.save_png(dir.path().join("b.png")).unwrap();
        let c = AssetCache::new(dir.path());
        assert_eq!(c.natural_size("a.svg").unwrap(), (20.0, 10.0));
        assert_eq!(c.natural_size("b.png").unwrap(), (8.0, 4.0));
        let a = c
            .pixmap(
                "a.svg",
                40,
                40,
                Resample::ContainOn([0, 0, 0, 255]),
                Filter::Bilinear,
            )
            .unwrap();
        // Letterboxed: red in the middle, black at the top.
        assert_eq!(a.pixel(20, 20).unwrap().red(), 255);
        assert_eq!(a.pixel(20, 2).unwrap().red(), 0);
        let b1 = c
            .pixmap(
                "b.png",
                4,
                4,
                Resample::Cover { focus: (0.5, 0.5) },
                Filter::Lanczos3,
            )
            .unwrap();
        let b2 = c
            .pixmap(
                "b.png",
                4,
                4,
                Resample::Cover { focus: (0.5, 0.5) },
                Filter::Lanczos3,
            )
            .unwrap();
        assert!(Arc::ptr_eq(&b1, &b2));
        assert!(matches!(
            c.decoded("missing.png"),
            Err(AssetError::Read { .. })
        ));
    }
}
