//! Partial renders of a prepared scene and pixel helpers for the design
//! checks: element masks without shadows, the composite up to an element,
//! distance maps, WCAG luminance and contrast.

// Pixel and geometry code with short names: e(lement), d(evice), t(ext),
// w/h, x/y, k (pixel), c (cell / corner).
#![allow(clippy::many_single_char_names)]

use tiny_skia::Pixmap;
use vitrine_core::geometry::{Rect, Vec2, Viewport};
use vitrine_core::resolve::{ResolvedElement, ResolvedKind, ResolvedScene, TextEffects};

use crate::scene::{PreparedScene, Quality, RenderContext};

/// Largest probe raster (pixels); bigger regions are sampled coarser.
const MAX_PIXELS: f64 = 1_500_000.0;

/// How an element is drawn into a probe.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Draw {
    /// As in the export.
    Full,
    /// Opaque, without shadows, glows, outlines or span highlights: the
    /// element's own shape (masks).
    Bare,
    /// [`Draw::Bare`] without the element's clip: the whole shape, also
    /// where a page edge cuts it off.
    Unclipped,
}

/// A prepared scene plus the render context, for partial renders.
pub(super) struct Probe<'c, 's> {
    pub ctx: &'c RenderContext,
    pub prepared: PreparedScene<'s>,
}

impl Probe<'_, '_> {
    pub fn scene(&self) -> &ResolvedScene {
        self.prepared.scene
    }

    /// Renders the elements `pick` selects (in painting order) into
    /// `view`, with or without the canvas backgrounds.
    pub fn paint(
        &self,
        view: &Viewport,
        backgrounds: bool,
        pick: impl Fn(usize, &ResolvedElement) -> Option<Draw>,
    ) -> Pixmap {
        let sc = self.scene();
        let mut elements = Vec::new();
        let mut texts = Vec::new();
        for (i, e) in sc.elements.iter().enumerate() {
            let Some(mode) = pick(i, e) else { continue };
            let mut el = e.clone();
            let mut text = self.prepared.text(i).cloned();
            if mode == Draw::Unclipped {
                el.clip = None;
            }
            if mode != Draw::Full {
                strip(&mut el);
                if let Some(t) = &mut text {
                    t.layout
                        .effects
                        .iter_mut()
                        .for_each(|f| *f = TextEffects::default());
                    t.layout.highlights.clear();
                }
            }
            elements.push(el);
            texts.push(text);
        }
        let sub = ResolvedScene {
            target: sc.target.clone(),
            canvas: sc.canvas.clone(),
            lang: sc.lang.clone(),
            variant: sc.variant.clone(),
            frame: sc.frame.clone(),
            backgrounds: if backgrounds {
                sc.backgrounds.clone()
            } else {
                Vec::new()
            },
            elements,
            spans: sc.spans.clone(),
            diagnostics: Vec::new(),
        };
        let prepared = PreparedScene::from_parts(&sub, texts);
        self.ctx
            .render_view(&prepared, view, Quality::Preview)
            .pixmap
    }

    /// Alpha mask (`alpha ≥ 50 %`) of one element drawn bare.
    pub fn mask(&self, view: &Viewport, index: usize) -> Vec<bool> {
        self.alpha(view, index)
            .into_iter()
            .map(|a| a >= 128)
            .collect()
    }

    /// Alpha of one element drawn bare.
    pub fn alpha(&self, view: &Viewport, index: usize) -> Vec<u8> {
        let pm = self.paint(view, false, |i, _| (i == index).then_some(Draw::Bare));
        pm.pixels().iter().map(|p| p.alpha()).collect()
    }

    /// Alpha of one element drawn bare and without its clip.
    pub fn alpha_unclipped(&self, view: &Viewport, index: usize) -> Vec<u8> {
        let pm = self.paint(view, false, |i, _| (i == index).then_some(Draw::Unclipped));
        pm.pixels().iter().map(|p| p.alpha()).collect()
    }

    /// Canvas bounds (rotated, clipped) of what the element draws itself
    /// (no shadows); `None` if it draws nothing.
    pub fn bounds(&self, index: usize) -> Option<Rect> {
        let e = self.scene().elements.get(index)?;
        let local = match &e.props {
            ResolvedKind::Text(_) => self.prepared.text(index)?.ink_rect(),
            ResolvedKind::Image(img) if img.auto_height => {
                let h = self
                    .ctx
                    .assets()
                    .natural_size(&img.asset)
                    .ok()
                    .filter(|n| n.0 > 0.0)
                    .map_or(e.size.y, |n| e.size.x * n.1 / n.0);
                Rect::centered(e.center, e.size.x, h)
            }
            ResolvedKind::Device(_) => {
                // Buttons stick out of the housing a little.
                let m = e.size.x.max(e.size.y) * 0.03;
                Rect::centered(e.center, e.size.x + 2.0 * m, e.size.y + 2.0 * m)
            }
            _ => Rect::centered(e.center, e.size.x, e.size.y),
        };
        let b = rotated_bounds(local, e);
        match e.clip {
            Some(c) => intersection(&b, &c),
            None => Some(b),
        }
    }
}

/// Removes everything outside the element's own shape and makes it opaque.
fn strip(e: &mut ResolvedElement) {
    e.opacity = 1.0;
    match &mut e.props {
        ResolvedKind::Device(d) => d.shadow.clear(),
        ResolvedKind::Shape(s) => s.shadow.clear(),
        ResolvedKind::Image(i) => i.shadow.clear(),
        ResolvedKind::Text(t) => {
            t.style.shadow.clear();
            t.style.stroke = None;
            t.style.glow = None;
        }
        ResolvedKind::Background { .. } => {}
    }
}

/// A raster over `region` at about `scale` px per lu (coarser for huge
/// regions).
pub(super) fn view_for(region: Rect, scale: f64) -> Viewport {
    let mut s = scale;
    let px = region.w * region.h * s * s;
    if px > MAX_PIXELS {
        s *= (MAX_PIXELS / px).sqrt();
    }
    Viewport {
        origin_x: region.x,
        origin_y: region.y,
        scale: s,
        width_px: (region.w * s).ceil().max(1.0) as u32,
        height_px: (region.h * s).ceil().max(1.0) as u32,
    }
}

/// Canvas point of the centre of pixel `i` of `view`.
pub(super) fn point(view: &Viewport, i: usize) -> Vec2 {
    let w = view.width_px as usize;
    let (x, y) = (i % w, i / w);
    Vec2::new(
        view.origin_x + (x as f64 + 0.5) / view.scale,
        view.origin_y + (y as f64 + 0.5) / view.scale,
    )
}

/// Axis-aligned bounds of `r` rotated with the element (around
/// `center + pivot`).
pub(super) fn rotated_bounds(r: Rect, e: &ResolvedElement) -> Rect {
    if e.rotation.rem_euclid(360.0) == 0.0 {
        return r;
    }
    bounds_of(&rotated_quad(r, e))
}

/// Corners of `r` (canvas, unrotated) rotated with the element:
/// top-left, top-right, bottom-right, bottom-left.
pub(super) fn rotated_quad(r: Rect, e: &ResolvedElement) -> [Vec2; 4] {
    let pivot = e.center + e.pivot;
    [
        Vec2::new(r.x, r.y),
        Vec2::new(r.right(), r.y),
        Vec2::new(r.right(), r.bottom()),
        Vec2::new(r.x, r.bottom()),
    ]
    .map(|c| c.rotated_around(pivot, e.rotation))
}

pub(super) fn bounds_of(points: &[Vec2]) -> Rect {
    let (mut lo, mut hi) = (points[0], points[0]);
    for c in &points[1..] {
        lo = Vec2::new(lo.x.min(c.x), lo.y.min(c.y));
        hi = Vec2::new(hi.x.max(c.x), hi.y.max(c.y));
    }
    Rect::new(lo.x, lo.y, hi.x - lo.x, hi.y - lo.y)
}

pub(super) fn intersection(a: &Rect, b: &Rect) -> Option<Rect> {
    let x = a.x.max(b.x);
    let y = a.y.max(b.y);
    let r = a.right().min(b.right());
    let btm = a.bottom().min(b.bottom());
    (r > x && btm > y).then(|| Rect::new(x, y, r - x, btm - y))
}

pub(super) fn outset(r: Rect, m: f64) -> Rect {
    Rect::new(r.x - m, r.y - m, r.w + 2.0 * m, r.h + 2.0 * m)
}

/// Distance (px) from every pixel to the nearest `true` pixel of `mask`
/// (two-pass chamfer, 1 / √2 steps; `f32::INFINITY` if there is none).
pub(super) fn distance_map(mask: &[bool], w: usize, h: usize) -> Vec<f32> {
    const D: f32 = std::f32::consts::SQRT_2;
    let mut d: Vec<f32> = mask
        .iter()
        .map(|&m| if m { 0.0 } else { f32::INFINITY })
        .collect();
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            let mut v = d[i];
            if x > 0 {
                v = v.min(d[i - 1] + 1.0);
            }
            if y > 0 {
                v = v.min(d[i - w] + 1.0);
                if x > 0 {
                    v = v.min(d[i - w - 1] + D);
                }
                if x + 1 < w {
                    v = v.min(d[i - w + 1] + D);
                }
            }
            d[i] = v;
        }
    }
    for y in (0..h).rev() {
        for x in (0..w).rev() {
            let i = y * w + x;
            let mut v = d[i];
            if x + 1 < w {
                v = v.min(d[i + 1] + 1.0);
            }
            if y + 1 < h {
                v = v.min(d[i + w] + 1.0);
                if x + 1 < w {
                    v = v.min(d[i + w + 1] + D);
                }
                if x > 0 {
                    v = v.min(d[i + w - 1] + D);
                }
            }
            d[i] = v;
        }
    }
    d
}

/// WCAG relative luminance of a premultiplied pixel shown over white (the
/// export flattens onto white).
pub(super) fn luminance(p: tiny_skia::PremultipliedColorU8) -> f64 {
    let a = 255 - u16::from(p.alpha());
    let c = |v: u8| f64::from((u16::from(v) + a).min(255)) / 255.0;
    rel_luminance(c(p.red()), c(p.green()), c(p.blue()))
}

/// WCAG 2.x relative luminance of an sRGB colour (channels 0..=1).
pub(super) fn rel_luminance(r: f64, g: f64, b: f64) -> f64 {
    let lin = |c: f64| {
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b)
}

/// WCAG contrast ratio of two luminances (1..=21).
pub(super) fn contrast(a: f64, b: f64) -> f64 {
    let (hi, lo) = if a > b { (a, b) } else { (b, a) };
    (hi + 0.05) / (lo + 0.05)
}

/// The `q` quantile (0..=1) of `v` (sorted in place).
pub(super) fn quantile(v: &mut [f64], q: f64) -> f64 {
    v.sort_by(f64::total_cmp);
    let i = ((v.len() as f64 - 1.0) * q).round() as usize;
    v[i.min(v.len() - 1)]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wcag_contrast_of_black_and_white_is_21() {
        let w = rel_luminance(1.0, 1.0, 1.0);
        let b = rel_luminance(0.0, 0.0, 0.0);
        assert!((contrast(w, b) - 21.0).abs() < 1e-9);
        // #777777 on white: 4.48:1 (just below AA).
        let g = rel_luminance(119.0 / 255.0, 119.0 / 255.0, 119.0 / 255.0);
        assert!((contrast(w, g) - 4.48).abs() < 0.01);
    }

    #[test]
    fn distance_map_measures_from_the_mask() {
        let (w, h) = (7, 3);
        let mut m = vec![false; w * h];
        m[w + 1] = true;
        let d = distance_map(&m, w, h);
        assert!(d[w + 1].abs() < f32::EPSILON);
        assert!((d[w + 5] - 4.0).abs() < 1e-5);
        assert!((d[2] - std::f32::consts::SQRT_2).abs() < 1e-5);
    }
}
