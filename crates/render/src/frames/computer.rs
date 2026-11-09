//! Laptops (lid, hinge, base with thumb lip) and desktops (housing with
//! optional chin, stand). Front views without perspective; depth is
//! suggested by shading only.

use arrangia_core::catalog::CutoutKind;
use arrangia_core::geometry::Rect;
use tiny_skia::Pixmap;

use super::Ctx;
use super::browser;
use super::palette::{contrast, mix, shade};
use super::shapes::{self, inset};
use crate::paint::{hex, linear, shader_paint, solid};

pub fn laptop(cx: &mut Ctx<'_, '_>, img: Option<&Pixmap>) {
    let Some((hinge, base)) = cx.l.laptop_parts() else {
        return;
    };
    let Some(lp) = cx.l.device.laptop else {
        return;
    };
    let [light, mid, dark] = cx.pal.rim;
    let (body, edge) = (cx.pal.body, cx.pal.edge);

    // Base: the front edge of the deck, a lighter top surface strip, the
    // thumb lip in the middle of the top edge.
    let base_path = cx.l.base_path(base);
    let top_strip = Rect::new(base.x, base.y, base.w, base.h * 0.3);
    let rt = (base.h * 0.12).min(lp.base.radius * cx.l.s);
    if cx.detailed {
        let sh = linear(
            (0.0, base.y as f32),
            (0.0, base.bottom() as f32),
            &[
                (0.0, shade(light, 0.3)),
                (0.08, light),
                (0.3, mid),
                (0.75, mix(mid, dark, 0.55)),
                (1.0, dark),
            ],
        );
        cx.fill(base_path.as_ref(), &shader_paint(sh));
        // Soft horizontal falloff towards the ends (rounded body).
        let ends = linear(
            (base.x as f32, 0.0),
            (base.right() as f32, 0.0),
            &[
                (0.0, [0, 0, 0, 60]),
                (0.08, [0, 0, 0, 0]),
                (0.92, [0, 0, 0, 0]),
                (1.0, [0, 0, 0, 60]),
            ],
        );
        cx.fill(base_path.as_ref(), &shader_paint(ends));
    } else {
        cx.color(base_path.as_ref(), edge);
        cx.color(
            shapes::rect(top_strip, [rt, rt, 0.0, 0.0], false).as_ref(),
            body,
        );
    }
    // Thumb lip: a shallow scoop in the top edge.
    let lip_w = lp.base.lip.w * cx.l.s;
    let lip_h = lp.base.lip.h * cx.l.s;
    let lip = Rect::new(-lip_w / 2.0, base.y, lip_w, lip_h);
    let lr = lip_h * 0.95;
    let lip_path = shapes::rect(lip, [0.0, 0.0, lr, lr], false);
    if cx.detailed {
        let sh = linear(
            (0.0, lip.y as f32),
            (0.0, lip.bottom() as f32),
            &[(0.0, mix(dark, hex("#000000"), 0.3)), (1.0, mid)],
        );
        cx.fill(lip_path.as_ref(), &shader_paint(sh));
    } else {
        cx.color(lip_path.as_ref(), shade(edge, -0.08));
    }

    // Hinge: dark strip under the lid.
    let hinge_path = shapes::rect(hinge, [0.0, 0.0, hinge.h * 0.5, hinge.h * 0.5], false);
    if cx.detailed {
        let sh = linear(
            (0.0, hinge.y as f32),
            (0.0, hinge.bottom() as f32),
            &[(0.0, hex("#0B0B0D")), (1.0, hex("#2A2A2E"))],
        );
        cx.fill(hinge_path.as_ref(), &shader_paint(sh));
    } else {
        cx.color(hinge_path.as_ref(), hex("#1A1A1D"));
    }

    // Lid: thin metal edge, black glass bezel.
    lid_face(cx, None);
    display(cx, img);
    camera(cx);
    if cx.detailed {
        sheen(cx);
    }
}

pub fn desktop(cx: &mut Ctx<'_, '_>, img: Option<&Pixmap>) {
    let Some((neck, foot)) = cx.l.stand_parts() else {
        return;
    };
    let [light, mid, dark] = cx.pal.rim;
    let (body, edge) = (cx.pal.body, cx.pal.edge);

    // Stand neck: brushed plate, in shadow right under the housing.
    let neck_path = shapes::plain(neck);
    if cx.detailed {
        let sh = linear(
            (neck.x as f32, 0.0),
            (neck.right() as f32, 0.0),
            &[
                (0.0, mix(mid, dark, 0.4)),
                (0.12, mid),
                (0.4, mix(light, mid, 0.4)),
                (0.88, mid),
                (1.0, mix(mid, dark, 0.5)),
            ],
        );
        cx.fill(neck_path.as_ref(), &shader_paint(sh));
    } else {
        cx.color(neck_path.as_ref(), edge);
    }
    // Contact shadow right under the housing.
    let tuck = cx.l.device.stand.map_or(0.0, |s| s.neck.h * 0.15) * cx.l.s;
    let shadow = linear(
        (0.0, (neck.y + tuck) as f32),
        (0.0, (neck.y + tuck + neck.h * 0.16) as f32),
        &[(0.0, [0, 0, 0, 90]), (1.0, [0, 0, 0, 0])],
    );
    cx.fill(neck_path.as_ref(), &shader_paint(shadow));

    // Foot.
    let foot_path = cx.l.foot_path(foot);
    if cx.detailed {
        let sh = linear(
            (0.0, foot.y as f32),
            (0.0, foot.bottom() as f32),
            &[(0.0, shade(light, 0.3)), (0.3, light), (1.0, dark)],
        );
        cx.fill(foot_path.as_ref(), &shader_paint(sh));
    } else {
        cx.color(foot_path.as_ref(), body);
        let lower = Rect::new(foot.x, foot.y + foot.h * 0.55, foot.w, foot.h * 0.45);
        let fr = cx.l.device.stand.map_or(0.0, |s| s.foot.radius * cx.l.s);
        cx.color(
            shapes::rect(lower, [0.0, 0.0, fr, fr], false).as_ref(),
            edge,
        );
    }

    // Housing: chin below the display when the bottom border is much
    // larger than the top one (all-in-one), otherwise a uniform bezel.
    let d = cx.l.device;
    let chin = d.body.h - d.display.bottom() > 2.0 * d.display.y;
    let chin_top = if chin {
        Some(cx.l.pt(0.0, d.display.bottom() + d.display.y).1)
    } else {
        None
    };
    lid_face(cx, chin_top);
    display(cx, img);
    camera(cx);
    if cx.detailed {
        sheen(cx);
    }
}

/// Housing face: metal edge + glass bezel; with `chin_top`, the glass ends
/// there and the housing colour shows below (chin).
fn lid_face(cx: &mut Ctx<'_, '_>, chin_top: Option<f64>) {
    let b = cx.l.body;
    let br = cx.l.body_r;
    let body_path = cx.l.body_path();
    let [light, mid, dark] = cx.pal.rim;
    if cx.detailed {
        let sh = linear(
            (b.x as f32, b.y as f32),
            (b.right() as f32, b.bottom() as f32),
            &[(0.0, light), (0.45, mid), (1.0, dark)],
        );
        cx.fill(body_path.as_ref(), &shader_paint(sh));
    } else {
        cx.color(body_path.as_ref(), cx.pal.body);
    }
    // Visible edge: catalog rim, at least a hairline.
    let e = cx.l.rim().max(b.w * 0.0025);
    let glass = if let Some(y) = chin_top {
        let g = Rect::new(b.x + e, b.y + e, b.w - 2.0 * e, y - b.y - e);
        shapes::rect(g, [br - e, br - e, 0.0, 0.0], true)
    } else {
        let r = (br - e).max(0.0);
        let g = inset(b, e);
        if cx.l.kind == super::layout::Kind::Laptop {
            shapes::rect(g, [r, r, r * 0.3, r * 0.3], true)
        } else {
            shapes::smooth(g, r)
        }
    };
    cx.color(glass.as_ref(), cx.pal.glass);
    if let Some(y) = chin_top
        && cx.detailed
    {
        // Chin: soft vertical shading on the housing colour.
        let chin = Rect::new(b.x, y, b.w, b.bottom() - y);
        let sh = linear(
            (0.0, chin.y as f32),
            (0.0, chin.bottom() as f32),
            &[
                (0.0, shade(cx.pal.body, 0.12)),
                (1.0, shade(cx.pal.body, -0.1)),
            ],
        );
        let p = shapes::rect(chin, [0.0, 0.0, br, br], true);
        cx.fill(p.as_ref(), &shader_paint(sh));
    }
    if cx.detailed {
        // Polished edge.
        let ins = e * 0.35;
        let edge = shapes::smooth(inset(b, ins), br - ins);
        let hl = linear(
            (b.x as f32, b.y as f32),
            (b.x as f32, b.bottom() as f32),
            &[(0.0, [255, 255, 255, 150]), (1.0, [255, 255, 255, 30])],
        );
        cx.stroke(edge.as_ref(), &shader_paint(hl), e * 0.35);
    }
}

/// Display content and (for `chrome: browser`) the browser bars above it.
fn display(cx: &mut Ctx<'_, '_>, img: Option<&Pixmap>) {
    if let Some(area) = cx.l.chrome {
        browser::chrome(cx, area);
    }
    cx.screen(img);
}

fn camera(cx: &mut Ctx<'_, '_>) {
    let d = cx.l.device;
    for c in d.cutouts.iter().filter(|c| c.kind == CutoutKind::Camera) {
        let (x, y) = cx.l.pt(c.x + c.w / 2.0, c.y + c.h / 2.0);
        let r = c.w.min(c.h) / 2.0 * cx.l.s;
        if cx.detailed {
            cx.color(
                shapes::circle(x, y, r * 1.5).as_ref(),
                mix(cx.pal.glass, hex("#000000"), 0.6),
            );
            cx.lens(x, y, r);
        } else {
            cx.color(
                shapes::circle(x, y, r).as_ref(),
                contrast(cx.pal.glass, 0.18),
            );
        }
    }
}

/// Faint diagonal reflection on the glass (`detailed`).
fn sheen(cx: &mut Ctx<'_, '_>) {
    let d = cx.l.display;
    let sh = linear(
        (d.x as f32, d.y as f32),
        ((d.x + d.w * 0.6) as f32, (d.y + d.h * 1.2) as f32),
        &[
            (0.0, [255, 255, 255, 18]),
            (0.45, [255, 255, 255, 6]),
            (0.47, [255, 255, 255, 0]),
            (1.0, [255, 255, 255, 0]),
        ],
    );
    let p = cx.l.screen_path();
    cx.fill(p.as_ref(), &shader_paint(sh));
    let disp = shapes::round(d, cx.l.display_r);
    cx.stroke(disp.as_ref(), &solid([0, 0, 0, 60]), d.w * 0.0012);
}
