//! Preview images: the binary payload sent to the UI and an in-memory
//! cache of encoded previews (KONZEPT §9.3, ADR 0008).
//!
//! **Cache key**: target, language, what (output image / canvas / gallery),
//! size, quality, format and a hash of the resolved scene (which includes
//! the mtimes of every referenced asset). Ops therefore invalidate exactly
//! the previews whose scene changed: a lookup with a new scene hash also
//! drops the stale entries of the same target × language × view. External
//! asset changes drop all entries referencing that asset; reloads and font
//! changes clear the cache. Memory is capped; the least recently used
//! entries are evicted first.
//!
//! **Payload** ([`PreviewImage`]): PNG (fast compression) or raw RGBA
//! (straight alpha). Tauri gets [`PreviewImage::to_tauri_bytes`] (a
//! 40-byte header + data, see there); the HTTP bridge sends the bare data
//! with `X-Width`, `X-Height`, `X-Format`, `X-Origin-X/Y`, `X-Scale`
//! headers.

use std::collections::{BTreeSet, HashMap};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::{Arc, Mutex, PoisonError};

use serde::{Deserialize, Serialize};
use tiny_skia::Pixmap;
use vitrine_core::geometry::Viewport;
use vitrine_core::resolve::{ResolvedKind, ResolvedScene};
use vitrine_render::export::scene_assets;
use vitrine_render::scene::RenderContext;

use crate::error::{CmdResult, CommandError};

/// Default memory cap of the cache (encoded bytes).
pub const DEFAULT_CAP_BYTES: usize = 256 * 1024 * 1024;

/// Magic of the Tauri payload header.
pub const MAGIC: [u8; 4] = *b"ARPX";
/// Length of the Tauri payload header.
pub const HEADER_LEN: usize = 40;

/// Pixel encoding of a preview.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PixelFormat {
    /// PNG, RGBA 8 bit, fast compression (default; see ADR 0008).
    #[default]
    Png,
    /// Raw RGBA 8 bit, straight (not premultiplied) alpha, row-major.
    Rgba,
    /// JPEG (quality [`JPEG_QUALITY`], transparency flattened onto white):
    /// much smaller than PNG for photos, e.g. large gallery previews over
    /// MCP.
    Jpeg,
}

/// JPEG quality of [`PixelFormat::Jpeg`] previews.
pub const JPEG_QUALITY: u8 = 85;

impl PixelFormat {
    pub fn code(self) -> u32 {
        match self {
            Self::Rgba => 0,
            Self::Png => 1,
            Self::Jpeg => 2,
        }
    }

    pub fn content_type(self) -> &'static str {
        match self {
            Self::Png => "image/png",
            Self::Rgba => "application/octet-stream",
            Self::Jpeg => "image/jpeg",
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Rgba => "rgba",
            Self::Jpeg => "jpeg",
        }
    }
}

/// An encoded preview plus where it sits on the canvas.
#[derive(Debug, Clone, PartialEq)]
pub struct PreviewImage {
    pub width: u32,
    pub height: u32,
    pub format: PixelFormat,
    /// Canvas x/y (lu) of the top-left pixel.
    pub origin_x: f64,
    pub origin_y: f64,
    /// Pixels per lu (0 for the gallery, which is not a canvas view).
    pub scale: f64,
    pub data: Vec<u8>,
}

impl PreviewImage {
    /// Encodes a premultiplied pixmap.
    pub fn encode(
        pixmap: &Pixmap,
        viewport: Option<&Viewport>,
        format: PixelFormat,
    ) -> CmdResult<Self> {
        let data = match format {
            PixelFormat::Rgba => straight_rgba(pixmap),
            PixelFormat::Png => encode_png(&straight_rgba(pixmap), pixmap.width(), pixmap.height())
                .map_err(|e| CommandError::new(crate::ErrorCode::Render, e.to_string()))?,
            PixelFormat::Jpeg => {
                vitrine_render::raster::encode_jpeg(pixmap, [255, 255, 255], JPEG_QUALITY)
                    .map_err(|e| CommandError::new(crate::ErrorCode::Render, e.to_string()))?
            }
        };
        Ok(Self {
            width: pixmap.width(),
            height: pixmap.height(),
            format,
            origin_x: viewport.map_or(0.0, |v| v.origin_x),
            origin_y: viewport.map_or(0.0, |v| v.origin_y),
            scale: viewport.map_or(0.0, |v| v.scale),
            data,
        })
    }

    /// Tauri IPC payload (`tauri::ipc::Response` → `ArrayBuffer` in JS):
    ///
    /// | bytes | content |
    /// |---|---|
    /// | 0..4 | magic `ARPX` |
    /// | 4..8 | width, u32 LE |
    /// | 8..12 | height, u32 LE |
    /// | 12..16 | format, u32 LE: 0 = RGBA, 1 = PNG, 2 = JPEG |
    /// | 16..24 | origin x (canvas lu), f64 LE |
    /// | 24..32 | origin y (canvas lu), f64 LE |
    /// | 32..40 | scale (px per lu), f64 LE |
    /// | 40.. | data |
    pub fn to_tauri_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(HEADER_LEN + self.data.len());
        out.extend_from_slice(&MAGIC);
        out.extend_from_slice(&self.width.to_le_bytes());
        out.extend_from_slice(&self.height.to_le_bytes());
        out.extend_from_slice(&self.format.code().to_le_bytes());
        out.extend_from_slice(&self.origin_x.to_le_bytes());
        out.extend_from_slice(&self.origin_y.to_le_bytes());
        out.extend_from_slice(&self.scale.to_le_bytes());
        out.extend_from_slice(&self.data);
        out
    }

    /// HTTP headers of the bridge response (besides `Content-Type`).
    pub fn http_headers(&self) -> Vec<(&'static str, String)> {
        vec![
            ("X-Width", self.width.to_string()),
            ("X-Height", self.height.to_string()),
            ("X-Format", self.format.as_str().to_owned()),
            ("X-Origin-X", self.origin_x.to_string()),
            ("X-Origin-Y", self.origin_y.to_string()),
            ("X-Scale", self.scale.to_string()),
        ]
    }
}

/// Straight-alpha RGBA bytes of a premultiplied pixmap.
pub fn straight_rgba(pixmap: &Pixmap) -> Vec<u8> {
    let mut out = pixmap.data().to_vec();
    for px in out.as_chunks_mut::<4>().0 {
        let a = px[3];
        if a != 255 && a != 0 {
            for c in &mut px[..3] {
                *c = ((u16::from(*c) * 255 + u16::from(a) / 2) / u16::from(a)).min(255) as u8;
            }
        }
    }
    out
}

/// PNG with `Compression::Fast` and the `Up` filter: ~1 ms for 800 px
/// previews at ~1/6 of the raw size (ADR 0008).
pub fn encode_png(rgba: &[u8], width: u32, height: u32) -> Result<Vec<u8>, png::EncodingError> {
    let mut out = Vec::with_capacity(rgba.len() / 4);
    {
        let mut enc = png::Encoder::new(&mut out, width, height);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        enc.set_compression(png::Compression::Fast);
        enc.set_filter(png::Filter::Up);
        let mut w = enc.write_header()?;
        w.write_image_data(rgba)?;
        w.finish()?;
    }
    Ok(out)
}

/// What a preview shows.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum View {
    Image(u32),
    Canvas {
        overlay: bool,
        /// Region `x,y,w,h` (lu) or the whole canvas.
        region: Option<String>,
    },
    Gallery,
}

/// Cache key; `size` is a normalized description of the requested size.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PreviewKey {
    pub target: String,
    pub lang: String,
    pub view: View,
    pub size: String,
    pub quality: String,
    pub format: PixelFormat,
    pub scene_hash: u64,
}

impl PreviewKey {
    fn same_view(&self, other: &PreviewKey) -> bool {
        self.target == other.target && self.lang == other.lang && self.view == other.view
    }
}

struct Entry {
    image: Arc<PreviewImage>,
    assets: BTreeSet<String>,
    last_used: u64,
}

#[derive(Default)]
struct Inner {
    map: HashMap<PreviewKey, Entry>,
    bytes: usize,
    tick: u64,
    hits: u64,
    misses: u64,
}

impl Inner {
    fn remove_where(&mut self, mut f: impl FnMut(&PreviewKey, &Entry) -> bool) -> usize {
        let keys: Vec<PreviewKey> = self
            .map
            .iter()
            .filter(|(k, e)| f(k, e))
            .map(|(k, _)| k.clone())
            .collect();
        for k in &keys {
            if let Some(e) = self.map.remove(k) {
                self.bytes -= e.image.data.len();
            }
        }
        keys.len()
    }
}

/// Cache statistics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct CacheStats {
    pub entries: usize,
    pub bytes: usize,
    pub hits: u64,
    pub misses: u64,
}

/// Encoded previews with an LRU memory cap. Thread-safe.
pub struct PreviewCache {
    inner: Mutex<Inner>,
    cap_bytes: usize,
}

impl std::fmt::Debug for PreviewCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PreviewCache")
            .field("stats", &self.stats())
            .finish_non_exhaustive()
    }
}

impl Default for PreviewCache {
    fn default() -> Self {
        Self::new(DEFAULT_CAP_BYTES)
    }
}

impl PreviewCache {
    pub fn new(cap_bytes: usize) -> Self {
        Self {
            inner: Mutex::new(Inner::default()),
            cap_bytes,
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Cached preview; on a miss, stale entries of the same view (other
    /// scene hash) are dropped.
    pub fn get(&self, key: &PreviewKey) -> Option<Arc<PreviewImage>> {
        let mut inner = self.lock();
        inner.tick += 1;
        let tick = inner.tick;
        if let Some(e) = inner.map.get_mut(key) {
            e.last_used = tick;
            let image = e.image.clone();
            inner.hits += 1;
            return Some(image);
        }
        inner.misses += 1;
        inner.remove_where(|k, _| k.same_view(key) && k.scene_hash != key.scene_hash);
        None
    }

    /// Stores a preview that references `assets`, evicting least recently
    /// used entries above the memory cap.
    pub fn insert(&self, key: PreviewKey, image: Arc<PreviewImage>, assets: BTreeSet<String>) {
        let mut inner = self.lock();
        inner.tick += 1;
        let size = image.data.len();
        if size > self.cap_bytes {
            return;
        }
        let tick = inner.tick;
        if let Some(old) = inner.map.insert(
            key,
            Entry {
                image,
                assets,
                last_used: tick,
            },
        ) {
            inner.bytes -= old.image.data.len();
        }
        inner.bytes += size;
        while inner.bytes > self.cap_bytes {
            let Some(oldest) = inner
                .map
                .iter()
                .min_by_key(|(_, e)| e.last_used)
                .map(|(k, _)| k.clone())
            else {
                break;
            };
            if let Some(e) = inner.map.remove(&oldest) {
                inner.bytes -= e.image.data.len();
            }
        }
    }

    /// Drops all previews that reference one of `assets` (project-relative
    /// paths). Returns the number of dropped entries.
    pub fn invalidate_assets(&self, assets: &[String]) -> usize {
        self.lock()
            .remove_where(|_, e| assets.iter().any(|a| e.assets.contains(a)))
    }

    /// Drops all previews of `target` (all languages), e.g. after an op.
    pub fn invalidate_target(&self, target: &str) -> usize {
        self.lock().remove_where(|k, _| k.target == target)
    }

    /// Drops everything.
    pub fn clear(&self) {
        let mut inner = self.lock();
        inner.map.clear();
        inner.bytes = 0;
    }

    pub fn stats(&self) -> CacheStats {
        let inner = self.lock();
        CacheStats {
            entries: inner.map.len(),
            bytes: inner.bytes,
            hits: inner.hits,
            misses: inner.misses,
        }
    }
}

/// Hash of everything that determines a scene's pixels besides the render
/// options: the resolved scene, catalog device data, asset paths + mtimes.
pub fn scene_hash(scene: &ResolvedScene, ctx: &RenderContext) -> u64 {
    let mut h = DefaultHasher::new();
    serde_json::to_vec(scene).unwrap_or_default().hash(&mut h);
    for e in &scene.elements {
        if let ResolvedKind::Device(d) = &e.props {
            format!("{:?}{:?}", d.device, d.palette).hash(&mut h);
        }
    }
    for a in scene_assets(scene) {
        ctx.assets().mtime(&a).hash(&mut h);
        a.hash(&mut h);
    }
    h.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image(bytes: usize) -> Arc<PreviewImage> {
        Arc::new(PreviewImage {
            width: 1,
            height: 1,
            format: PixelFormat::Png,
            origin_x: 0.0,
            origin_y: 0.0,
            scale: 1.0,
            data: vec![0; bytes],
        })
    }

    fn key(target: &str, image: u32, hash: u64) -> PreviewKey {
        PreviewKey {
            target: target.into(),
            lang: "en".into(),
            view: View::Image(image),
            size: "max_edge:400".into(),
            quality: "preview".into(),
            format: PixelFormat::Png,
            scene_hash: hash,
        }
    }

    #[test]
    fn hits_misses_and_stale_entries() {
        let c = PreviewCache::new(1000);
        c.insert(key("a", 0, 1), image(10), BTreeSet::new());
        c.insert(key("a", 1, 1), image(10), BTreeSet::new());
        assert!(c.get(&key("a", 0, 1)).is_some());
        // New scene hash for image 0: miss, the stale entry is dropped,
        // image 1 (another view) stays.
        assert!(c.get(&key("a", 0, 2)).is_none());
        assert_eq!(c.stats().entries, 1);
        assert!(c.get(&key("a", 1, 1)).is_some());
        assert_eq!(c.stats().hits, 2);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn invalidates_by_asset_and_target_and_evicts_lru() {
        let c = PreviewCache::new(25);
        let shot: BTreeSet<String> = ["screenshots/en/home.png".to_owned()].into();
        c.insert(key("a", 0, 1), image(10), shot);
        c.insert(key("b", 0, 1), image(10), BTreeSet::new());
        assert_eq!(c.invalidate_assets(&["screenshots/en/home.png".into()]), 1);
        assert_eq!(c.stats().entries, 1);
        c.insert(key("a", 0, 1), image(10), BTreeSet::new());
        assert!(c.get(&key("b", 0, 1)).is_some()); // b is now more recent than a
        c.insert(key("c", 0, 1), image(10), BTreeSet::new()); // 30 > 25: evict a
        assert!(c.get(&key("a", 0, 1)).is_none());
        assert!(c.get(&key("b", 0, 1)).is_some());
        assert_eq!(c.stats().bytes, 20);
        assert_eq!(c.invalidate_target("b"), 1);
        c.clear();
        assert_eq!(c.stats().entries, 0);
    }

    #[test]
    fn tauri_payload_header() {
        let img = PreviewImage {
            width: 3,
            height: 2,
            format: PixelFormat::Rgba,
            origin_x: 10.5,
            origin_y: -2.0,
            scale: 0.25,
            data: vec![7; 24],
        };
        let b = img.to_tauri_bytes();
        assert_eq!(b.len(), HEADER_LEN + 24);
        assert_eq!(&b[..4], b"ARPX");
        assert_eq!(u32::from_le_bytes(b[4..8].try_into().unwrap()), 3);
        assert_eq!(u32::from_le_bytes(b[12..16].try_into().unwrap()), 0);
        assert!((f64::from_le_bytes(b[16..24].try_into().unwrap()) - 10.5).abs() < 1e-12);
        assert!((f64::from_le_bytes(b[32..40].try_into().unwrap()) - 0.25).abs() < 1e-12);
    }

    #[test]
    fn straight_alpha() {
        let mut pm = Pixmap::new(1, 1).unwrap();
        pm.fill(tiny_skia::Color::from_rgba8(200, 100, 0, 128));
        let px = straight_rgba(&pm);
        assert_eq!(px[3], 128);
        assert!((i32::from(px[0]) - 200).abs() <= 2, "{px:?}");
    }
}
