//! Small drawing helpers on top of tiny-skia (paths, colours, gradients).

#![allow(clippy::many_single_char_names, reason = "geometry helpers")]

use tiny_skia::{
    Color, GradientStop, LinearGradient, Paint, Path, PathBuilder, Point, RadialGradient, Shader,
    SpreadMode, Transform,
};

/// Straight RGBA colour.
pub type Rgba = [u8; 4];

/// Parses `#RRGGBB` or `#RRGGBBAA`.
pub fn hex(s: &str) -> Rgba {
    let s = s.trim_start_matches('#');
    let v = |i: usize| u8::from_str_radix(&s[i..i + 2], 16).unwrap_or(0);
    if s.len() >= 8 {
        [v(0), v(2), v(4), v(6)]
    } else {
        [v(0), v(2), v(4), 255]
    }
}

pub fn with_alpha(c: Rgba, a: f32) -> Rgba {
    [
        c[0],
        c[1],
        c[2],
        (f32::from(c[3]) * a).round().clamp(0.0, 255.0) as u8,
    ]
}

pub fn sk_color(c: Rgba) -> Color {
    Color::from_rgba8(c[0], c[1], c[2], c[3])
}

/// Solid anti-aliased paint.
pub fn solid(c: Rgba) -> Paint<'static> {
    let mut p = Paint::default();
    p.set_color(sk_color(c));
    p.anti_alias = true;
    p
}

pub fn shader_paint(shader: Shader<'_>) -> Paint<'_> {
    Paint {
        shader,
        anti_alias: true,
        ..Paint::default()
    }
}

fn stops(s: &[(f32, Rgba)]) -> Vec<GradientStop> {
    s.iter()
        .map(|(p, c)| GradientStop::new(*p, sk_color(*c)))
        .collect()
}

/// Linear gradient from `p0` to `p1` (local coordinates).
pub fn linear(p0: (f32, f32), p1: (f32, f32), s: &[(f32, Rgba)]) -> Shader<'static> {
    LinearGradient::new(
        Point::from_xy(p0.0, p0.1),
        Point::from_xy(p1.0, p1.1),
        stops(s),
        SpreadMode::Pad,
        Transform::identity(),
    )
    .unwrap_or(Shader::SolidColor(sk_color(s[0].1)))
}

/// Radial gradient around `c` with radius `r`.
pub fn radial(c: (f32, f32), r: f32, s: &[(f32, Rgba)]) -> Shader<'static> {
    RadialGradient::new(
        Point::from_xy(c.0, c.1),
        0.0,
        Point::from_xy(c.0, c.1),
        r,
        stops(s),
        SpreadMode::Pad,
        Transform::identity(),
    )
    .unwrap_or(Shader::SolidColor(sk_color(s[0].1)))
}

/// CSS-style linear gradient line for a rectangle: `angle` in degrees,
/// 0° = towards top, 90° = towards right (clockwise). Returns start/end points.
pub fn css_gradient_line(
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    angle_deg: f32,
) -> ((f32, f32), (f32, f32)) {
    let a = angle_deg.to_radians();
    let (dx, dy) = (a.sin(), -a.cos());
    let len = (w * a.sin()).abs() + (h * a.cos()).abs();
    let (cx, cy) = (x + w / 2.0, y + h / 2.0);
    (
        (cx - dx * len / 2.0, cy - dy * len / 2.0),
        (cx + dx * len / 2.0, cy + dy * len / 2.0),
    )
}

/// Rounded rectangle with circular corners.
pub fn round_rect(x: f32, y: f32, w: f32, h: f32, r: f32) -> Option<Path> {
    let r = r.min(w / 2.0).min(h / 2.0).max(0.0);
    let k = 0.552_284_8 * r;
    let mut pb = PathBuilder::new();
    pb.move_to(x + r, y);
    pb.line_to(x + w - r, y);
    pb.cubic_to(x + w - r + k, y, x + w, y + r - k, x + w, y + r);
    pb.line_to(x + w, y + h - r);
    pb.cubic_to(x + w, y + h - r + k, x + w - r + k, y + h, x + w - r, y + h);
    pb.line_to(x + r, y + h);
    pb.cubic_to(x + r - k, y + h, x, y + h - r + k, x, y + h - r);
    pb.line_to(x, y + r);
    pb.cubic_to(x, y + r - k, x + r - k, y, x + r, y);
    pb.close();
    pb.finish()
}

/// Rounded rectangle with "continuous" (squircle-like) corners: the curve
/// starts earlier (1.5 r from the corner) and blends smoothly into the edge,
/// which avoids the visible kink of circular corners on device housings.
pub fn smooth_rect(x: f32, y: f32, w: f32, h: f32, r: f32) -> Option<Path> {
    let r = r.min(w / 3.0).min(h / 3.0).max(0.0);
    let e = 1.5 * r; // start of the curve measured from the corner
    let c1 = 0.42 * r; // control point distance from the corner (along edge)
    let mut pb = PathBuilder::new();
    let (l, t, rt, b) = (x, y, x + w, y + h);
    pb.move_to(l + e, t);
    pb.line_to(rt - e, t);
    pb.cubic_to(rt - c1, t, rt, t + c1, rt, t + e);
    pb.line_to(rt, b - e);
    pb.cubic_to(rt, b - c1, rt - c1, b, rt - e, b);
    pb.line_to(l + e, b);
    pb.cubic_to(l + c1, b, l, b - c1, l, b - e);
    pb.line_to(l, t + e);
    pb.cubic_to(l, t + c1, l + c1, t, l + e, t);
    pb.close();
    pb.finish()
}

pub fn circle(cx: f32, cy: f32, r: f32) -> Option<Path> {
    PathBuilder::from_circle(cx, cy, r)
}
