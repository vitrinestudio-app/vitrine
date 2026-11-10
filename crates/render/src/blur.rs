//! Soft shadows: silhouette → alpha mask → 3-pass box blur (≈ Gaussian).
//!
//! tiny-skia has no blur, so shadows are rendered as a coverage mask of the
//! element silhouette, blurred with three successive box blurs (the classic
//! "boxes for Gauss" approximation, error < 3 %) and composited as a solid
//! colour. For large radii the mask is rendered at reduced resolution and
//! scaled back up bilinearly while compositing — a blurred shadow has no high frequencies, so
//! this is visually lossless and keeps preview rendering fast.

#![allow(
    clippy::cast_possible_wrap,
    clippy::many_single_char_names,
    reason = "small blur kernels; standard short names"
)]

use tiny_skia::{FillRule, Paint, Path, Pixmap, Transform};

/// Box sizes for `n` box blurs approximating a Gaussian with `sigma`.
pub fn boxes_for_gauss(sigma: f64, n: usize) -> Vec<usize> {
    let n_f = n as f64;
    let w_ideal = (12.0 * sigma * sigma / n_f + 1.0).sqrt();
    let mut wl = w_ideal.floor() as i64;
    if wl % 2 == 0 {
        wl -= 1;
    }
    let wl = wl.max(1);
    let wu = wl + 2;
    let wl_f = wl as f64;
    let m_ideal = (12.0 * sigma * sigma - n_f * wl_f * wl_f - 4.0 * n_f * wl_f - 3.0 * n_f)
        / (-4.0 * wl_f - 4.0);
    let m = m_ideal.round() as i64;
    (0..n as i64)
        .map(|i| if i < m { wl as usize } else { wu as usize })
        .collect()
}

/// One horizontal box blur pass with radius `r` (box width `2r+1`) over a
/// single-channel buffer; edges are treated as transparent (zero).
fn box_blur_h(src: &[f32], dst: &mut [f32], w: usize, h: usize, r: usize) {
    let norm = 1.0 / (2 * r + 1) as f32;
    for y in 0..h {
        let row = &src[y * w..(y + 1) * w];
        let out = &mut dst[y * w..(y + 1) * w];
        let mut acc: f32 = row.iter().take(r.min(w)).sum();
        for x in 0..w {
            if x + r < w {
                acc += row[x + r];
            }
            out[x] = acc * norm;
            if x >= r {
                acc -= row[x - r];
            }
        }
    }
}

fn transpose(src: &[f32], dst: &mut [f32], w: usize, h: usize) {
    for y in 0..h {
        for x in 0..w {
            dst[x * h + y] = src[y * w + x];
        }
    }
}

/// Approximate Gaussian blur (3 box passes per axis) of a single channel.
pub fn gaussian_blur(buf: &mut [f32], w: usize, h: usize, sigma: f64) {
    if sigma <= 0.0 || w == 0 || h == 0 {
        return;
    }
    let boxes = boxes_for_gauss(sigma, 3);
    let mut tmp = vec![0.0f32; buf.len()];
    for b in &boxes {
        box_blur_h(buf, &mut tmp, w, h, (b - 1) / 2);
        buf.copy_from_slice(&tmp);
    }
    transpose(buf, &mut tmp, w, h);
    let mut tmp2 = vec![0.0f32; buf.len()];
    for b in &boxes {
        box_blur_h(&tmp, &mut tmp2, h, w, (b - 1) / 2);
        tmp.copy_from_slice(&tmp2);
    }
    transpose(&tmp, buf, h, w);
}

/// Drop shadow parameters in output pixels.
#[derive(Debug, Clone, Copy)]
pub struct ShadowPx {
    pub sigma: f64,
    pub dx: f64,
    pub dy: f64,
    /// Straight RGBA colour; alpha is the shadow opacity.
    pub color: [u8; 4],
}

/// Draws a blurred drop shadow of `path` (in local coordinates, placed with
/// `transform` in pixel space) onto `target`.
pub fn draw_shadow(target: &mut Pixmap, path: &Path, transform: Transform, shadow: ShadowPx) {
    let Some(bounds) = path.clone().transform(transform).map(|p| p.bounds()) else {
        return;
    };
    draw_blurred(target, bounds, shadow, |mask, to_mask| {
        let mut paint = Paint::default();
        paint.set_color_rgba8(0, 0, 0, 255);
        paint.anti_alias = true;
        mask.fill_path(
            path,
            &paint,
            FillRule::Winding,
            transform.post_concat(to_mask),
            None,
        );
    });
}

/// Draws a blurred, tinted copy of an arbitrary silhouette onto `target`:
/// `paint_mask` paints the silhouette (only its alpha counts) into the
/// mask pixmap, mapping target pixels to mask pixels with the transform it
/// is given (post-concatenate it to a pixel-space transform). `bounds` are
/// the silhouette's bounds in target pixels (before the shadow offset).
/// Used for drop shadows of paths, image alpha channels and text glows.
pub fn draw_blurred(
    target: &mut Pixmap,
    bounds: tiny_skia::Rect,
    shadow: ShadowPx,
    paint_mask: impl FnOnce(&mut Pixmap, Transform),
) {
    let margin = (shadow.sigma * 3.0).ceil() as f32 + 2.0;
    let left = (bounds.left() + shadow.dx as f32 - margin).floor();
    let top = (bounds.top() + shadow.dy as f32 - margin).floor();
    let right = (bounds.right() + shadow.dx as f32 + margin).ceil();
    let bottom = (bounds.bottom() + shadow.dy as f32 + margin).ceil();
    // Cull against the target.
    let (tw, th) = (target.width() as f32, target.height() as f32);
    if right <= 0.0 || bottom <= 0.0 || left >= tw || top >= th {
        return;
    }
    // Downsample factor: keep at least ~3 px of sigma in the small mask.
    let factor = (shadow.sigma / 3.0).floor().clamp(1.0, 8.0) as f32;
    let mw = ((right - left) / factor).ceil() as u32 + 1;
    let mh = ((bottom - top) / factor).ceil() as u32 + 1;
    let Some(mut mask) = Pixmap::new(mw, mh) else {
        return;
    };
    let to_mask = Transform::from_translate(shadow.dx as f32 - left, shadow.dy as f32 - top)
        .post_scale(1.0 / factor, 1.0 / factor);
    paint_mask(&mut mask, to_mask);

    let (w, h) = (mw as usize, mh as usize);
    let mut alpha: Vec<f32> = mask
        .data()
        .as_chunks::<4>()
        .0
        .iter()
        .map(|p| f32::from(p[3]))
        .collect();
    gaussian_blur(&mut alpha, w, h, shadow.sigma / f64::from(factor));

    let [r, g, b, a] = shadow.color;
    let opacity = f32::from(a) / 255.0 / 255.0;
    let color = [f32::from(r), f32::from(g), f32::from(b), 255.0];
    // Composite directly: bilinear upsampling of the small blurred mask +
    // premultiplied source-over (much faster than a transformed draw_pixmap).
    let x0 = left.max(0.0) as usize;
    let x1 = right.min(tw) as usize;
    let y0 = top.max(0.0) as usize;
    let y1 = bottom.min(th) as usize;
    let stride = target.width() as usize;
    let sample = |u: f32, v: f32| -> f32 {
        let u = u.clamp(0.0, (w - 1) as f32);
        let v = v.clamp(0.0, (h - 1) as f32);
        let (iu, iv) = (u as usize, v as usize);
        let (fu, fv) = (u - iu as f32, v - iv as f32);
        let iu1 = (iu + 1).min(w - 1);
        let iv1 = (iv + 1).min(h - 1);
        let a00 = alpha[iv * w + iu];
        let a01 = alpha[iv * w + iu1];
        let a10 = alpha[iv1 * w + iu];
        let a11 = alpha[iv1 * w + iu1];
        (a00 * (1.0 - fu) + a01 * fu) * (1.0 - fv) + (a10 * (1.0 - fu) + a11 * fu) * fv
    };
    let data = target.data_mut();
    for y in y0..y1 {
        let v = (y as f32 + 0.5 - top) / factor - 0.5;
        let row = &mut data[(y * stride + x0) * 4..(y * stride + x1) * 4];
        for (i, px) in row.as_chunks_mut::<4>().0.iter_mut().enumerate() {
            let u = ((x0 + i) as f32 + 0.5 - left) / factor - 0.5;
            let al = (sample(u, v) * opacity).clamp(0.0, 1.0);
            if al <= 0.001 {
                continue;
            }
            let inv = 1.0 - al;
            for c in 0..4 {
                px[c] = (color[c] * al + f32::from(px[c]) * inv).round().min(255.0) as u8;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boxes_approximate_sigma() {
        let b = boxes_for_gauss(10.0, 3);
        assert_eq!(b.len(), 3);
        // variance of n boxes of width w: sum (w^2-1)/12 ≈ sigma^2
        let var: f64 = b.iter().map(|w| ((w * w - 1) as f64) / 12.0).sum();
        assert!((var.sqrt() - 10.0).abs() < 0.6, "{var}");
        assert!(b.iter().all(|w| w % 2 == 1));
    }

    #[test]
    fn blur_preserves_mass_away_from_edges() {
        let (w, h) = (101, 101);
        let mut buf = vec![0.0f32; w * h];
        buf[50 * w + 50] = 1000.0;
        gaussian_blur(&mut buf, w, h, 5.0);
        let sum: f32 = buf.iter().sum();
        assert!((sum - 1000.0).abs() < 1.0, "{sum}");
        assert!(buf[50 * w + 50] < 20.0);
        // symmetric
        assert!((buf[50 * w + 40] - buf[50 * w + 60]).abs() < 1e-3);
        assert!((buf[40 * w + 50] - buf[60 * w + 50]).abs() < 1e-3);
    }
}
