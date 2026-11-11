//! Editor-style overlay for canvas views (KONZEPT §3.3): gaps veiled and
//! hatched (their content "disappears" in the store), dashed page
//! boundaries and page labels.

use tiny_skia::{FillRule, Mask, PathBuilder, Pixmap, Stroke, StrokeDash};
use vitrine_core::geometry::Viewport;
use vitrine_core::layout::{ResolvedTextStyle, TextInput, TextLayouter};
use vitrine_core::model::Rgba;
use vitrine_core::resolve::ResolvedScene;

use crate::geometry::view_transform;
use crate::paint::{round_rect, solid};
use crate::text::draw_layout;

pub(super) fn debug_overlay(
    pm: &mut Pixmap,
    scene: &ResolvedScene,
    view: &Viewport,
    layouter: &mut TextLayouter,
) {
    let vt = view_transform(view);
    let px = |v: f64| (v / view.scale) as f32; // px → lu
    for g in &scene.frame.gaps {
        let Some(r) = tiny_skia::Rect::from_xywh(g.x as f32, g.y as f32, g.w as f32, g.h as f32)
        else {
            continue;
        };
        pm.fill_rect(r, &solid([255, 255, 255, 110]), vt, None);
        let mut pb = PathBuilder::new();
        let step = f64::from(px(10.0));
        let mut t = -g.w;
        while t < g.h {
            pb.move_to(g.x as f32, (g.y + t) as f32);
            pb.line_to(g.right() as f32, (g.y + t + g.w) as f32);
            t += step;
        }
        let Some(p) = pb.finish() else { continue };
        let Some(mut mask) = Mask::new(pm.width(), pm.height()) else {
            continue;
        };
        mask.fill_path(&PathBuilder::from_rect(r), FillRule::Winding, true, vt);
        let stroke = Stroke {
            width: px(2.0),
            ..Stroke::default()
        };
        pm.stroke_path(&p, &solid([200, 30, 60, 200]), &stroke, vt, Some(&mask));
    }
    let label_size = px(14.0);
    let style = ResolvedTextStyle {
        weight: 600,
        size: label_size,
        color: Rgba::rgb(255, 255, 255),
        ..ResolvedTextStyle::default()
    };
    for page in &scene.frame.pages {
        let r = page.rect;
        if let Some(rect) =
            tiny_skia::Rect::from_xywh(r.x as f32, r.y as f32, r.w as f32, r.h as f32)
        {
            let stroke = Stroke {
                width: px(1.5),
                dash: StrokeDash::new(vec![px(8.0), px(5.0)], 0.0),
                ..Stroke::default()
            };
            pm.stroke_path(
                &PathBuilder::from_rect(rect),
                &solid([255, 255, 255, 220]),
                &stroke,
                vt,
                None,
            );
        }
        let label = format!("{} · {} · x = {:.0} lu", page.index, page.id, r.x);
        let l = layouter.layout(&TextInput::new(&label, &style, 2000.0));
        let pad = label_size * 0.4;
        let ts = vt.pre_translate(
            (r.x + f64::from(px(8.0))) as f32,
            (r.bottom() - f64::from(px(8.0)) - f64::from(l.content.h + pad)) as f32,
        );
        if let Some(bg) = round_rect(
            -pad,
            -pad / 2.0,
            l.content.w + 2.0 * pad,
            l.content.h + pad,
            pad,
        ) {
            pm.fill_path(&bg, &solid([0, 0, 0, 150]), FillRule::Winding, ts, None);
        }
        draw_layout(pm, &l, layouter.fonts(), ts, None);
    }
}
