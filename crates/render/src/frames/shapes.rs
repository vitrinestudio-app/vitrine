//! Path builders for the frames: rectangles with per-corner radii (circular
//! or "continuous"), pills, circles, the notch outline.

use tiny_skia::{Path, PathBuilder, Rect as SkRect};
use vitrine_core::geometry::Rect;

/// Circular (`κ`) control distance of a quarter circle, measured from the
/// corner point along the edge: `r − 0.5523 r`.
const CIRCLE_C: f64 = 1.0 - 0.552_284_8;

/// Corner radii: top-left, top-right, bottom-right, bottom-left.
pub type Radii = [f64; 4];

pub fn uniform(r: f64) -> Radii {
    [r; 4]
}

/// Rectangle with per-corner radii. `smooth` = "continuous" corners (the
/// curve starts 1.5 r from the corner and blends into the edge, like
/// modern device housings); otherwise circular corners.
///
/// The outline runs clockwise (y down) starting at the top edge, so several
/// such paths filled with the non-zero rule form their union.
pub fn rect(r: Rect, radii: Radii, smooth: bool) -> Option<Path> {
    let (l, t, rt, b) = (r.x, r.y, r.right(), r.bottom());
    let limit = r.w.min(r.h) / if smooth { 3.0 } else { 2.0 };
    // (extent from the corner where the curve starts, control distance)
    let corner = |radius: f64| {
        let radius = radius.clamp(0.0, limit.max(0.0));
        if smooth {
            (1.5 * radius, 0.42 * radius)
        } else {
            (radius, CIRCLE_C * radius)
        }
    };
    let [(e0, c0), (e1, c1), (e2, c2), (e3, c3)] = radii.map(corner);
    let p = |x: f64, y: f64| (x as f32, y as f32);
    let mut pb = PathBuilder::new();
    let m = p(l + e0, t);
    pb.move_to(m.0, m.1);
    line(&mut pb, p(rt - e1, t));
    cubic(&mut pb, p(rt - c1, t), p(rt, t + c1), p(rt, t + e1));
    line(&mut pb, p(rt, b - e2));
    cubic(&mut pb, p(rt, b - c2), p(rt - c2, b), p(rt - e2, b));
    line(&mut pb, p(l + e3, b));
    cubic(&mut pb, p(l + c3, b), p(l, b - c3), p(l, b - e3));
    line(&mut pb, p(l, t + e0));
    cubic(&mut pb, p(l, t + c0), p(l + c0, t), p(l + e0, t));
    pb.close();
    pb.finish()
}

fn line(pb: &mut PathBuilder, p: (f32, f32)) {
    pb.line_to(p.0, p.1);
}

fn cubic(pb: &mut PathBuilder, a: (f32, f32), b: (f32, f32), c: (f32, f32)) {
    pb.cubic_to(a.0, a.1, b.0, b.1, c.0, c.1);
}

/// Rounded rect with circular corners of one radius.
pub fn round(r: Rect, radius: f64) -> Option<Path> {
    rect(r, uniform(radius), false)
}

/// Rounded rect with continuous corners of one radius.
pub fn smooth(r: Rect, radius: f64) -> Option<Path> {
    rect(r, uniform(radius), true)
}

/// Stadium (fully rounded short sides).
pub fn pill(r: Rect) -> Option<Path> {
    round(r, r.w.min(r.h) / 2.0)
}

pub fn circle(cx: f64, cy: f64, radius: f64) -> Option<Path> {
    PathBuilder::from_circle(cx as f32, cy as f32, radius as f32)
}

pub fn plain(r: Rect) -> Option<Path> {
    SkRect::from_xywh(r.x as f32, r.y as f32, r.w as f32, r.h as f32).map(PathBuilder::from_rect)
}

/// Union of several closed, clockwise outlines (non-zero fill).
pub fn union(paths: impl IntoIterator<Item = Option<Path>>) -> Option<Path> {
    let mut pb = PathBuilder::new();
    for p in paths.into_iter().flatten() {
        pb.push_path(&p);
    }
    pb.finish()
}

/// Ring between two outlines (even-odd fill).
pub fn ring(outer: &Path, inner: &Path) -> Option<Path> {
    let mut pb = PathBuilder::new();
    pb.push_path(outer);
    pb.push_path(inner);
    pb.finish()
}

/// Notch hanging from the top display edge at `y` (model units, natural
/// orientation): `x, w, h` of the notch body, `radius` of its lower
/// corners, concave shoulders where it meets the display edge. The shape
/// extends up to `top` (into the bezel) so it joins the bezel seamlessly.
pub fn notch(x: f64, y: f64, w: f64, h: f64, radius: f64, top: f64) -> Option<Path> {
    let r = radius.clamp(0.0, (w / 2.0).min(h));
    let sr = (radius * 0.8).clamp(0.0, h / 2.0);
    let k = 0.552_284_8;
    let p = |x: f64, y: f64| (x as f32, y as f32);
    let mut pb = PathBuilder::new();
    let s = p(x - sr, top);
    pb.move_to(s.0, s.1);
    line(&mut pb, p(x - sr, y));
    cubic(
        &mut pb,
        p(x - sr + k * sr, y),
        p(x, y + sr - k * sr),
        p(x, y + sr),
    );
    line(&mut pb, p(x, y + h - r));
    cubic(
        &mut pb,
        p(x, y + h - r + k * r),
        p(x + r - k * r, y + h),
        p(x + r, y + h),
    );
    line(&mut pb, p(x + w - r, y + h));
    cubic(
        &mut pb,
        p(x + w - r + k * r, y + h),
        p(x + w, y + h - r + k * r),
        p(x + w, y + h - r),
    );
    line(&mut pb, p(x + w, y + sr));
    cubic(
        &mut pb,
        p(x + w, y + sr - k * sr),
        p(x + w + sr - k * sr, y),
        p(x + w + sr, y),
    );
    line(&mut pb, p(x + w + sr, top));
    pb.close();
    pb.finish()
}

/// `r` shrunk by `d` on every side.
pub fn inset(r: Rect, d: f64) -> Rect {
    Rect::new(
        r.x + d,
        r.y + d,
        (r.w - 2.0 * d).max(0.0),
        (r.h - 2.0 * d).max(0.0),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rect_bounds_match() {
        let r = Rect::new(-10.0, -20.0, 20.0, 40.0);
        for smooth in [false, true] {
            let p = rect(r, [3.0, 0.0, 8.0, 100.0], smooth).expect("path");
            let b = p.bounds();
            assert!((b.left() + 10.0).abs() < 1e-3 && (b.bottom() - 20.0).abs() < 1e-3);
        }
    }
}
