//! Composite views: store gallery preview and canvas debug view.

use tiny_skia::{
    FillRule, FilterQuality, Pattern, Pixmap, SpreadMode, Stroke, StrokeDash, Transform,
};

use crate::geometry::{StripLayout, Viewport};
use crate::paint::{Rgba, hex, round_rect, shader_paint, solid};
use crate::text::draw_layout;
use arrangia_core::layout::{ResolvedTextStyle, TextInput, TextLayouter};

/// Store gallery preview: export images side by side with the gallery gap
/// and rounded corners, the way the store shows them.
///
/// `images` are the rendered export images (all the same size); `scale` is px
/// per lu of these images. Placement uses the canvas x of each image, so
/// content crossing a gap lines up exactly as on the canvas.
pub fn gallery(
    images: &[Pixmap],
    layout: &StripLayout,
    scale: f64,
    corner_lu: f64,
    padding_px: u32,
    background: Rgba,
) -> Pixmap {
    let pad = padding_px as f32;
    let w = (layout.canvas_width() * scale).round() as u32 + 2 * padding_px;
    let h = images.first().map_or(1, Pixmap::height) + 2 * padding_px;
    let mut pm = Pixmap::new(w, h).expect("gallery size");
    pm.fill(crate::paint::sk_color(background));
    for (i, img) in images.iter().enumerate() {
        let r = layout.image_rect(i);
        let x = pad + (r.x * scale) as f32;
        let (iw, ih) = (img.width() as f32, img.height() as f32);
        let Some(path) = round_rect(x, pad, iw, ih, (corner_lu * scale) as f32) else {
            continue;
        };
        // Hairline border like the store's screenshot frame.
        pm.stroke_path(
            &path,
            &solid([0, 0, 0, 30]),
            &Stroke {
                width: 1.0,
                ..Stroke::default()
            },
            Transform::identity(),
            None,
        );
        let pat = Pattern::new(
            img.as_ref(),
            SpreadMode::Pad,
            FilterQuality::Nearest,
            1.0,
            Transform::from_translate(x, pad),
        );
        pm.fill_path(
            &path,
            &shader_paint(pat),
            FillRule::Winding,
            Transform::identity(),
            None,
        );
    }
    pm
}

/// Overlays page boundaries, hatched gaps and page labels onto a render of
/// the whole canvas (editor-style debug view, `docs/KONZEPT.md` §3.3).
pub fn debug_overlay(
    pm: &mut Pixmap,
    layout: &StripLayout,
    view: &Viewport,
    layouter: &mut TextLayouter,
) {
    let vt = view.transform();
    // Gaps: semi-transparent veil + diagonal hatching.
    for g in layout.gap_rects() {
        if let Some(r) = tiny_skia::Rect::from_xywh(g.x as f32, g.y as f32, g.w as f32, g.h as f32)
        {
            pm.fill_rect(r, &solid([255, 255, 255, 110]), vt, None);
            let mut pb = tiny_skia::PathBuilder::new();
            let step = 24.0;
            let mut t = -g.w;
            while t < g.h {
                let y0 = g.y + t;
                pb.move_to(g.x as f32, y0 as f32);
                pb.line_to(g.right() as f32, (y0 + g.w) as f32);
                t += step;
            }
            if let Some(p) = pb.finish() {
                let mask = {
                    let mut m = tiny_skia::Mask::new(pm.width(), pm.height()).expect("mask");
                    m.fill_path(
                        &tiny_skia::PathBuilder::from_rect(r),
                        FillRule::Winding,
                        true,
                        vt,
                    );
                    m
                };
                pm.stroke_path(
                    &p,
                    &solid([200, 30, 60, 200]),
                    &Stroke {
                        width: 5.0,
                        ..Stroke::default()
                    },
                    vt,
                    Some(&mask),
                );
            }
        }
    }
    // Page boundaries (dashed) and labels.
    for i in 0..layout.page_count {
        let r = layout.page_rect(i);
        if let Some(rect) =
            tiny_skia::Rect::from_xywh(r.x as f32, r.y as f32, r.w as f32, r.h as f32)
        {
            let p = tiny_skia::PathBuilder::from_rect(rect);
            let stroke = Stroke {
                width: (2.0 / view.scale) as f32,
                dash: StrokeDash::new(
                    vec![(18.0 / view.scale) as f32, (10.0 / view.scale) as f32],
                    0.0,
                ),
                ..Stroke::default()
            };
            pm.stroke_path(&p, &solid([255, 255, 255, 220]), &stroke, vt, None);
        }
        let style = ResolvedTextStyle {
            weight: 600,
            size: 44.0,
            color: super::core_color(hex("#FFFFFF")),
            ..ResolvedTextStyle::default()
        };
        let label = format!("page {i} · x = {:.0} lu", r.x);
        let l = layouter.layout(&TextInput::new(&label, &style, 900.0));
        let ts = vt.pre_translate((r.x + 24.0) as f32, (r.bottom() - 90.0) as f32);
        if let Some(bg) = round_rect(-12.0, -8.0, l.content.w + 24.0, l.content.h + 16.0, 14.0) {
            pm.fill_path(&bg, &solid([0, 0, 0, 150]), FillRule::Winding, ts, None);
        }
        draw_layout(pm, &l, layouter.fonts(), ts, None);
    }
}
