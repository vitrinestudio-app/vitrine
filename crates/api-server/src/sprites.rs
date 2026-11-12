//! Gallery sprites (owner decision 2026-11-12): per canvas one WebP with
//! all pages side by side and no gaps, 600 px page height plus a 2×
//! variant, rendered on the server from the stored package (its sample
//! screenshots if confirmed, else placeholders; parametric frames only).
//! The website slices them with `background-position`.
//!
//! Files: `<data>/templates/<id>/sprites/<canvas>.webp` and
//! `<canvas>@2x.webp`; metadata ([`SpriteMeta`]) in the `sprites` column.
//! Rendering is CPU heavy: it runs in a blocking task and at most
//! `VITRINE_API_RENDER_CONCURRENCY` renders run at once
//! ([`crate::AppState::render_slot`]).

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use vitrine_core::template::Package;
use vitrine_render::raster::to_rgb;
use vitrine_render::template::{SPRITE_PAGE_HEIGHT, Sprite, sprites};

/// WebP quality (0–100) of the sprites.
pub const WEBP_QUALITY: f32 = 80.0;

/// Metadata of one canvas's sprites.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpriteMeta {
    /// Canvas id.
    pub id: String,
    pub pages: u32,
    /// Size of the 1× image (the 2× image is exactly twice as large).
    pub width: u32,
    pub height: u32,
    pub page_width: u32,
    pub page_height: u32,
    /// Content hashes (cache busting, `ETag`).
    pub etag: String,
    pub etag_2x: String,
    /// File sizes in bytes.
    pub bytes: u64,
    pub bytes_2x: u64,
}

/// Encoded sprites of a canvas: metadata and the two WebP files.
#[derive(Debug, Clone)]
pub struct CanvasSprites {
    pub meta: SpriteMeta,
    pub webp: Vec<u8>,
    pub webp_2x: Vec<u8>,
}

/// Short content hash used as `ETag` and `?v=` value.
pub fn etag(data: &[u8]) -> String {
    use std::fmt::Write as _;
    Sha256::digest(data)
        .iter()
        .take(10)
        .fold(String::new(), |mut s, b| {
            let _ = write!(s, "{b:02x}");
            s
        })
}

fn encode(s: &Sprite) -> anyhow::Result<Vec<u8>> {
    let (w, h) = (s.pixmap.width(), s.pixmap.height());
    let rgb = to_rgb(&s.pixmap, [255, 255, 255]);
    let mem = webp::Encoder::from_rgb(&rgb, w, h).encode(WEBP_QUALITY);
    if mem.is_empty() {
        anyhow::bail!("WebP encoding failed ({w}x{h})");
    }
    Ok(mem.to_vec())
}

/// Renders and encodes the sprites of every canvas (blocking).
pub fn render(pkg: &Package) -> anyhow::Result<Vec<CanvasSprites>> {
    let mut out = Vec::new();
    for (canvas, list) in sprites(pkg, SPRITE_PAGE_HEIGHT, &[1, 2])? {
        let [one, two] = list.as_slice() else {
            continue;
        };
        let webp = encode(one)?;
        let webp_2x = encode(two)?;
        out.push(CanvasSprites {
            meta: SpriteMeta {
                id: canvas,
                pages: one.pages,
                width: one.pixmap.width(),
                height: one.pixmap.height(),
                page_width: one.page_width,
                page_height: one.page_height,
                etag: etag(&webp),
                etag_2x: etag(&webp_2x),
                bytes: webp.len() as u64,
                bytes_2x: webp_2x.len() as u64,
            },
            webp,
            webp_2x,
        });
    }
    Ok(out)
}

/// The metadata list as stored in the database.
pub fn metas_json(list: &[CanvasSprites]) -> Value {
    json!(list.iter().map(|c| &c.meta).collect::<Vec<_>>())
}

/// Metadata of `canvas` from the stored list.
pub fn meta_of(stored: &Value, canvas: &str) -> Option<SpriteMeta> {
    stored
        .as_array()?
        .iter()
        .filter_map(|v| serde_json::from_value::<SpriteMeta>(v.clone()).ok())
        .find(|m| m.id == canvas)
}

/// File name of a sprite (`phone.webp`, `phone@2x.webp`).
pub fn file_name(canvas: &str, double: bool) -> String {
    if double {
        format!("{canvas}@2x.webp")
    } else {
        format!("{canvas}.webp")
    }
}

/// Parses a requested sprite file name: `(canvas, 2x?)`.
pub fn parse_file_name(name: &str) -> Option<(&str, bool)> {
    let stem = name.strip_suffix(".webp")?;
    let (canvas, double) = match stem.strip_suffix("@2x") {
        Some(c) => (c, true),
        None => (stem, false),
    };
    vitrine_core::model::is_valid_id(canvas).then_some((canvas, double))
}

/// The `sprite` object of a canvas in API responses.
pub fn api_object(base: &str, m: &SpriteMeta) -> Value {
    json!({
        "url": format!("{base}/sprites/{}?v={}", file_name(&m.id, false), m.etag),
        "url_2x": format!("{base}/sprites/{}?v={}", file_name(&m.id, true), m.etag_2x),
        "width": m.width,
        "height": m.height,
        "page_width": m.page_width,
        "page_height": m.page_height,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_names_round_trip() {
        assert_eq!(parse_file_name("phone.webp"), Some(("phone", false)));
        assert_eq!(parse_file_name("phone@2x.webp"), Some(("phone", true)));
        assert_eq!(parse_file_name("../x.webp"), None);
        assert_eq!(parse_file_name("phone.png"), None);
        assert_eq!(file_name("phone", true), "phone@2x.webp");
    }

    #[test]
    fn builtin_sprites_are_small_webp_files() {
        let pkg = vitrine_core::template::builtin::package("phone").unwrap();
        let list = render(&pkg).unwrap();
        assert_eq!(list.len(), 1);
        let c = &list[0];
        assert!(c.webp.starts_with(b"RIFF") && &c.webp[8..12] == b"WEBP");
        assert_eq!(c.meta.page_height, SPRITE_PAGE_HEIGHT);
        assert_eq!(c.meta.width, 3 * c.meta.page_width);
        // Reasonable sizes (lossy).
        assert!(c.webp.len() < 150_000, "{}", c.webp.len());
        assert!(c.webp_2x.len() < 400_000, "{}", c.webp_2x.len());
        let back = meta_of(&metas_json(&list), "phone").unwrap();
        assert_eq!(back, c.meta);
    }
}
