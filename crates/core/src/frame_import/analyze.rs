//! Screen detection in a bezel PNG: the transparent interior around the
//! image centre (flood fill), its bounding box, corner radius and mask.

use std::io::Cursor;

use serde::{Deserialize, Serialize};

use crate::model::PixelSize;

/// Pixels with alpha below this count as "screen" (transparent).
pub const ALPHA_THRESHOLD: u8 = 128;

/// Largest decoded bezel (bytes of RGBA); Apple's biggest bezels (iMac,
/// Pro Display) are far below.
const MAX_DECODED_BYTES: usize = 1 << 30;

/// A rectangle in image pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PxRect {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

/// What the analysis found.
#[derive(Debug, Clone, PartialEq)]
pub struct ScreenAnalysis {
    pub size: PixelSize,
    /// Bounding box of the transparent interior.
    pub screen: PxRect,
    /// Estimated corner radius of the screen (px, mean of the four corners).
    pub radius: f64,
    /// Screen mask, one byte per pixel of the whole image: 255 inside the
    /// screen (grown by [`MASK_GROW`] px so the screenshot reaches under the
    /// anti-aliased bezel edge), a 1 px feather of 128 around it, 0 outside.
    /// Opaque parts inside the screen (island, notch) stay 0.
    pub mask: Vec<u8>,
    /// Housing colour measured on the outer rim ([`body_swatch`]).
    pub swatch: [u8; 3],
}

/// Pixels the mask is grown beyond the transparent region.
pub const MASK_GROW: u32 = 2;

/// An RGBA8 image (straight alpha).
#[derive(Debug, Clone)]
pub struct RgbaImage {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}

impl RgbaImage {
    fn alpha(&self, x: u32, y: u32) -> u8 {
        self.data[(y as usize * self.width as usize + x as usize) * 4 + 3]
    }
}

/// Decodes a PNG to RGBA8.
pub fn decode_png(bytes: &[u8]) -> Result<RgbaImage, String> {
    let mut decoder = png::Decoder::new_with_limits(
        Cursor::new(bytes),
        png::Limits {
            bytes: MAX_DECODED_BYTES,
        },
    );
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().map_err(|e| e.to_string())?;
    let mut buf = vec![0; reader.output_buffer_size().ok_or("image too large")?];
    let info = reader.next_frame(&mut buf).map_err(|e| e.to_string())?;
    buf.truncate(info.buffer_size());
    let (w, h) = (info.width, info.height);
    let n = w as usize * h as usize;
    let data = match info.color_type {
        png::ColorType::Rgba => buf,
        png::ColorType::Rgb => buf
            .as_chunks::<3>()
            .0
            .iter()
            .flat_map(|p| [p[0], p[1], p[2], 255])
            .collect(),
        png::ColorType::GrayscaleAlpha => buf
            .as_chunks::<2>()
            .0
            .iter()
            .flat_map(|p| [p[0], p[0], p[0], p[1]])
            .collect(),
        png::ColorType::Grayscale => buf.iter().flat_map(|&g| [g, g, g, 255]).collect(),
        png::ColorType::Indexed => return Err("unexpected indexed PNG after expansion".into()),
    };
    if data.len() != n * 4 {
        return Err("unexpected PNG buffer size".into());
    }
    Ok(RgbaImage {
        width: w,
        height: h,
        data,
    })
}

/// Encodes a one-byte-per-pixel mask as an 8-bit grayscale PNG.
pub fn encode_gray_png(width: u32, height: u32, data: &[u8]) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    let mut enc = png::Encoder::new(&mut out, width, height);
    enc.set_color(png::ColorType::Grayscale);
    enc.set_depth(png::BitDepth::Eight);
    enc.set_compression(png::Compression::Fast);
    let mut w = enc.write_header().map_err(|e| e.to_string())?;
    w.write_image_data(data).map_err(|e| e.to_string())?;
    w.finish().map_err(|e| e.to_string())?;
    Ok(out)
}

/// Encodes RGBA8 (straight alpha) as PNG.
pub fn encode_rgba_png(img: &RgbaImage) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    let mut enc = png::Encoder::new(&mut out, img.width, img.height);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    let mut w = enc.write_header().map_err(|e| e.to_string())?;
    w.write_image_data(&img.data).map_err(|e| e.to_string())?;
    w.finish().map_err(|e| e.to_string())?;
    Ok(out)
}

/// Finds the screen: flood fill (4-connected) from the image centre over
/// pixels with alpha < [`ALPHA_THRESHOLD`]. Errors when the centre is
/// opaque or the region reaches the image border (no enclosed screen).
pub fn analyze(img: &RgbaImage) -> Result<ScreenAnalysis, String> {
    let (w, h) = (img.width, img.height);
    if w < 8 || h < 8 {
        return Err(format!("image too small ({w}×{h})"));
    }
    let (cx, cy) = (w / 2, h / 2);
    if img.alpha(cx, cy) >= ALPHA_THRESHOLD {
        return Err("the image centre is not transparent: no screen area found".into());
    }
    let idx = |x: u32, y: u32| y as usize * w as usize + x as usize;
    let mut inside = vec![false; w as usize * h as usize];
    let mut stack = vec![(cx, cy)];
    inside[idx(cx, cy)] = true;
    let (mut x0, mut y0, mut x1, mut y1) = (cx, cy, cx, cy);
    while let Some((x, y)) = stack.pop() {
        if x == 0 || y == 0 || x == w - 1 || y == h - 1 {
            return Err(
                "the transparent centre reaches the image edge: not a bezel with an enclosed screen"
                    .into(),
            );
        }
        x0 = x0.min(x);
        y0 = y0.min(y);
        x1 = x1.max(x);
        y1 = y1.max(y);
        for (nx, ny) in [(x - 1, y), (x + 1, y), (x, y - 1), (x, y + 1)] {
            let i = idx(nx, ny);
            if !inside[i] && img.alpha(nx, ny) < ALPHA_THRESHOLD {
                inside[i] = true;
                stack.push((nx, ny));
            }
        }
    }
    let screen = PxRect {
        x: x0,
        y: y0,
        w: x1 - x0 + 1,
        h: y1 - y0 + 1,
    };
    let radius = corner_radius(&inside, w, screen);
    let mask = grow_mask(&inside, w, h, screen);
    Ok(ScreenAnalysis {
        size: PixelSize::new(w, h),
        screen,
        radius,
        mask,
        swatch: body_swatch(img, screen),
    })
}

/// Pixels darker than this (Rec. 601 luma) count as the black glass
/// border around the screen, not as housing.
const GLASS_LUMA: u32 = 28;

/// The housing colour of a bezel, for the inspector's colour swatches.
///
/// A front bezel is: the transparent screen, a near-black glass border
/// around it, and outside that the coloured metal rim (plus side buttons).
/// Scanlines across the middle 60 % of each side run from the image edge
/// inwards; on each, the first `band` opaque pixels after the anti-aliased
/// edge are sampled (`band` = 40 % of the edge-to-screen distance, so the
/// rim is covered but not much glass). Glass-dark pixels are dropped when
/// lighter ones make up at least 1/12 of the samples (a thin rim; an
/// all-dark bezel keeps them all), and the
/// result is the mean of the middle half by luma (no highlights, no
/// shadows). Uniform test bezels give exactly their colour.
pub fn body_swatch(img: &RgbaImage, screen: PxRect) -> [u8; 3] {
    let (w, h) = (img.width, img.height);
    let px = |x: u32, y: u32| {
        let i = (y as usize * w as usize + x as usize) * 4;
        let d = &img.data[i..i + 4];
        [d[0], d[1], d[2], d[3]]
    };
    let mut samples: Vec<[u8; 3]> = Vec::new();
    let mut scan = |points: &mut dyn Iterator<Item = (u32, u32)>, depth: u32| {
        let band = (depth * 2 / 5).clamp(2, 64);
        let mut taken = 0;
        let mut skipped = 0;
        for (x, y) in points.take(depth as usize) {
            let p = px(x, y);
            if p[3] < 250 {
                if taken > 0 {
                    break;
                }
                continue;
            }
            // Skip the first opaque pixel (edge shading).
            if skipped < 1 {
                skipped += 1;
                continue;
            }
            samples.push([p[0], p[1], p[2]]);
            taken += 1;
            if taken >= band {
                break;
            }
        }
    };
    let lines = |a: u32, len: u32| {
        let lo = a + len / 5;
        let hi = a + len - len / 5;
        let step = ((hi - lo) / 48).max(1);
        (lo..hi).step_by(step as usize)
    };
    for y in lines(screen.y, screen.h) {
        scan(&mut (0..screen.x).map(|x| (x, y)), screen.x);
        let right = screen.x + screen.w;
        scan(&mut (right..w).rev().map(|x| (x, y)), w - right);
    }
    for x in lines(screen.x, screen.w) {
        scan(&mut (0..screen.y).map(|y| (x, y)), screen.y);
        let bottom = screen.y + screen.h;
        scan(&mut (bottom..h).rev().map(|y| (x, y)), h - bottom);
    }
    let luma = |p: &[u8; 3]| {
        (299 * u32::from(p[0]) + 587 * u32::from(p[1]) + 114 * u32::from(p[2])) / 1000
    };
    if samples.is_empty() {
        return [0, 0, 0];
    }
    let light: Vec<[u8; 3]> = samples
        .iter()
        .copied()
        .filter(|p| luma(p) >= GLASS_LUMA)
        .collect();
    let mut pool = if light.len() * 12 >= samples.len() {
        light
    } else {
        samples
    };
    pool.sort_by_key(luma);
    let n = pool.len();
    let mid = &pool[n / 4..(n - n / 4).max(n / 4 + 1)];
    let mut sum = [0u64; 3];
    for p in mid {
        for c in 0..3 {
            sum[c] += u64::from(p[c]);
        }
    }
    let k = mid.len() as u64;
    #[allow(clippy::cast_possible_truncation, reason = "a mean of u8 values")]
    sum.map(|v| ((v + k / 2) / k) as u8)
}

/// `#RRGGBB` of an RGB colour.
pub fn hex_rgb(c: [u8; 3]) -> String {
    format!("#{:02X}{:02X}{:02X}", c[0], c[1], c[2])
}

/// Mean corner radius: along each corner's diagonal, the distance `d` to
/// the first screen pixel; a circular corner of radius `r` gives
/// `d = r (1 − 1/√2)`.
fn corner_radius(inside: &[bool], w: u32, r: PxRect) -> f64 {
    let at = |x: u32, y: u32| inside[y as usize * w as usize + x as usize];
    let max = r.w.min(r.h) / 2;
    let corners = [
        (r.x, r.y, 1i64, 1i64),
        (r.x + r.w - 1, r.y, -1, 1),
        (r.x, r.y + r.h - 1, 1, -1),
        (r.x + r.w - 1, r.y + r.h - 1, -1, -1),
    ];
    let k = 1.0 - std::f64::consts::FRAC_1_SQRT_2;
    let sum: f64 = corners
        .iter()
        .map(|&(x, y, dx, dy)| {
            let d = (0..max)
                .find(|&d| {
                    let px = (i64::from(x) + dx * i64::from(d)) as u32;
                    let py = (i64::from(y) + dy * i64::from(d)) as u32;
                    at(px, py)
                })
                .unwrap_or(0);
            // The first inside pixel's centre lies ~0.5 px past the arc.
            (f64::from(d) / k - 0.5).max(0.0)
        })
        .sum();
    sum / 4.0
}

/// The region grown by [`MASK_GROW`] px (255) plus a 1 px feather (128).
fn grow_mask(inside: &[bool], w: u32, h: u32, bbox: PxRect) -> Vec<u8> {
    let (wu, hu) = (w as usize, h as usize);
    let m = MASK_GROW as usize + 1;
    let (bx0, by0) = (
        (bbox.x as usize).saturating_sub(m),
        (bbox.y as usize).saturating_sub(m),
    );
    let bx1 = (bbox.x as usize + bbox.w as usize + m).min(wu);
    let by1 = (bbox.y as usize + bbox.h as usize + m).min(hu);
    // Chebyshev distance transform limited to MASK_GROW + 1 (two passes of
    // a 3×3 dilation are enough for these small radii).
    let mut dist: Vec<u8> = inside
        .iter()
        .map(|&b| if b { 0 } else { u8::MAX })
        .collect();
    let limit = MASK_GROW as u8 + 1;
    for step in 1..=limit {
        let prev = dist.clone();
        for y in by0..by1 {
            for x in bx0..bx1 {
                let i = y * wu + x;
                if prev[i] != u8::MAX {
                    continue;
                }
                let near = (y.saturating_sub(1)..=(y + 1).min(hu - 1)).any(|yy| {
                    (x.saturating_sub(1)..=(x + 1).min(wu - 1))
                        .any(|xx| prev[yy * wu + xx] == step - 1)
                });
                if near {
                    dist[i] = step;
                }
            }
        }
    }
    dist.iter()
        .map(|&d| match d {
            d if d <= MASK_GROW as u8 => 255,
            d if d == limit => 128,
            _ => 0,
        })
        .collect()
}
