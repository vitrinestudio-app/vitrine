//! Screen content: the screenshot mapped onto the screen rect, or a
//! placeholder when it is missing.

use tiny_skia::{FillRule, FilterQuality, Path, Pattern, Pixmap, SpreadMode, Stroke, Transform};
use vitrine_core::geometry::Rect;

use super::Ctx;
use super::glyphs;
use super::shapes;
use crate::paint::{hex, shader_paint, solid};

/// Fills `mask` (the screen outline, local lu) with the screenshot
/// stretched onto `rect`, or with a placeholder.
pub fn content(cx: &mut Ctx<'_, '_>, mask: &Path, rect: Rect, img: Option<&Pixmap>) {
    let ts = cx.ts;
    if let Some(img) = img {
        let pat_ts = Transform::from_row(
            (rect.w / f64::from(img.width())) as f32,
            0.0,
            0.0,
            (rect.h / f64::from(img.height())) as f32,
            rect.x as f32,
            rect.y as f32,
        );
        let pat = Pattern::new(
            img.as_ref(),
            SpreadMode::Pad,
            FilterQuality::Bilinear,
            1.0,
            pat_ts,
        );
        cx.pm
            .fill_path(mask, &shader_paint(pat), FillRule::Winding, ts, None);
        return;
    }
    placeholder(cx, mask, rect);
}

/// Neutral fill, soft diagonal hatching, an image glyph and the screenshot
/// key.
fn placeholder(cx: &mut Ctx<'_, '_>, mask: &Path, rect: Rect) {
    let ts = cx.ts;
    let (bg, stripe, fg) = (hex("#E6E9EE"), hex("#EEF0F3"), hex("#9AA2AE"));
    let unit = rect.w.min(rect.h);
    // Soft diagonal hatching as a repeating hard-stop gradient (clipped by
    // the screen outline like any fill).
    let step = (unit / 14.0) as f32;
    let (x0, y0) = (rect.x as f32, rect.y as f32);
    let hatch = tiny_skia::LinearGradient::new(
        tiny_skia::Point::from_xy(x0, y0),
        tiny_skia::Point::from_xy(x0 + step, y0 + step),
        vec![
            tiny_skia::GradientStop::new(0.0, crate::paint::sk_color(bg)),
            tiny_skia::GradientStop::new(0.6, crate::paint::sk_color(bg)),
            tiny_skia::GradientStop::new(0.6, crate::paint::sk_color(stripe)),
            tiny_skia::GradientStop::new(1.0, crate::paint::sk_color(stripe)),
        ],
        SpreadMode::Repeat,
        Transform::identity(),
    )
    .unwrap_or(tiny_skia::Shader::SolidColor(crate::paint::sk_color(bg)));
    cx.pm
        .fill_path(mask, &shader_paint(hatch), FillRule::Winding, ts, None);
    // Image glyph: rounded frame, sun, mountains.
    let (mx, my) = (rect.x + rect.w / 2.0, rect.y + rect.h / 2.0);
    let g = unit * 0.22;
    let frame = Rect::new(mx - g / 2.0, my - g * 0.42, g, g * 0.76);
    let stroke = Stroke {
        width: (g * 0.06) as f32,
        line_join: tiny_skia::LineJoin::Round,
        ..Stroke::default()
    };
    if let Some(p) = shapes::round(frame, g * 0.12) {
        cx.pm.stroke_path(&p, &solid(fg), &stroke, ts, None);
    }
    if let Some(p) = shapes::circle(frame.x + g * 0.68, frame.y + g * 0.22, g * 0.08) {
        cx.pm.fill_path(&p, &solid(fg), FillRule::Winding, ts, None);
    }
    let mut pb = tiny_skia::PathBuilder::new();
    let base = frame.bottom() - g * 0.12;
    pb.move_to((frame.x + g * 0.14) as f32, base as f32);
    pb.line_to((frame.x + g * 0.38) as f32, (frame.y + g * 0.30) as f32);
    pb.line_to((frame.x + g * 0.58) as f32, (base - g * 0.18) as f32);
    pb.line_to((frame.x + g * 0.70) as f32, (base - g * 0.30) as f32);
    pb.line_to((frame.right() - g * 0.12) as f32, base as f32);
    pb.close();
    if let Some(p) = pb.finish() {
        cx.pm.fill_path(&p, &solid(fg), FillRule::Winding, ts, None);
    }
    // Screenshot key.
    let key = cx
        .dev
        .screenshot
        .as_ref()
        .map_or("no screenshot", |s| s.key.as_str());
    let size = (unit * 0.055) as f32;
    if let Some(l) = glyphs::label(key, size, (rect.w * 0.85) as f32) {
        let x = mx as f32 - l.width / 2.0;
        let y = (frame.bottom() + g * 0.32) as f32 + l.cap;
        cx.pm.fill_path(
            &l.path,
            &solid(hex("#6B7380")),
            FillRule::Winding,
            ts.pre_translate(x, y),
            None,
        );
    }
}
