//! Phones and tablets: buttons, housing, front glass, screen, cutouts.

use arrangia_core::catalog::{ButtonStyle, CutoutKind, CutoutShape, DeviceClass, Edge, HingeAt};
use arrangia_core::geometry::Rect;
use tiny_skia::{FillRule, Mask, Pixmap};

use super::Ctx;
use super::palette::{contrast, luma, mix, shade};
use super::shapes::{self, inset};
use crate::paint::{Rgba, hex, linear, radial, shader_paint, solid};

/// Colour of the bezel (front face around the display). `stylized`: the
/// housing colour for light housings; mid and dark housings get a near-black
/// face so the colour shows as the rim (a flat two-tone look).
fn face(cx: &Ctx<'_, '_>) -> Rgba {
    // Watches always have a black front around the display.
    if cx.detailed || cx.l.device.class == DeviceClass::Watch {
        cx.pal.glass
    } else if luma(cx.pal.body) < 0.6 {
        mix(cx.pal.body, hex("#141416"), 0.7)
    } else {
        cx.pal.body
    }
}

pub fn draw(cx: &mut Ctx<'_, '_>, img: Option<&Pixmap>) {
    buttons(cx);
    housing(cx);
    hinge_spine(cx);
    cx.screen(img);
    crease(cx);
    display_cutouts(cx);
    bezel_features(cx);
    if cx.detailed {
        glass_finish(cx);
    }
}

fn buttons(cx: &mut Ctx<'_, '_>) {
    let d = cx.l.device;
    let (bw, bh) = (d.body.w, d.body.h);
    for b in &d.buttons {
        // Full button incl. the part hidden behind the housing.
        let t = b.depth * 2.4;
        let hide = t - b.depth;
        let r = match b.edge {
            Edge::Left => cx.l.rect_xywh(-b.depth, b.at, t, b.length),
            Edge::Right => cx.l.rect_xywh(bw - hide, b.at, t, b.length),
            Edge::Top => cx.l.rect_xywh(b.at, -b.depth, b.length, t),
            Edge::Bottom => cx.l.rect_xywh(b.at, bh - hide, b.length, t),
        };
        // Is the edge vertical on screen (the frame may be rotated)?
        let vertical = matches!(b.edge, Edge::Left | Edge::Right) != super::layout::rotated(cx.dev);
        match b.style {
            ButtonStyle::Crown => {
                crown(cx, r, vertical);
                continue;
            }
            ButtonStyle::Guard => {
                guard(cx, r, vertical);
                continue;
            }
            ButtonStyle::Plain => {}
        }
        let path = shapes::round(r, r.w.min(r.h) * 0.42);
        if cx.detailed {
            let [light, mid, dark] = cx.pal.rim;
            // Gradient across the thin dimension (outwards = towards the
            // light), in local space so it follows the rotated frame.
            let (p0, p1) = if r.w < r.h {
                ((r.x as f32, 0.0), (r.right() as f32, 0.0))
            } else {
                ((0.0, r.y as f32), (0.0, r.bottom() as f32))
            };
            let sh = linear(
                p0,
                p1,
                &[(0.0, dark), (0.35, light), (0.7, mid), (1.0, dark)],
            );
            cx.fill(path.as_ref(), &shader_paint(sh));
        } else {
            cx.color(path.as_ref(), cx.pal.edge);
        }
    }
}

/// Watch crown: a short cylinder sticking out of the edge, shaded along
/// its round side, with ribs along its axis.
fn crown(cx: &mut Ctx<'_, '_>, r: Rect, vertical: bool) {
    let path = shapes::round(r, r.w.min(r.h) * 0.22);
    // `along` = the round side (along the edge), `across` = the axis.
    let (a0, a1) = if vertical {
        (r.y, r.bottom())
    } else {
        (r.x, r.right())
    };
    let ends = |t0: f64, t1: f64| -> ((f32, f32), (f32, f32)) {
        if vertical {
            ((0.0, t0 as f32), (0.0, t1 as f32))
        } else {
            ((t0 as f32, 0.0), (t1 as f32, 0.0))
        }
    };
    let [light, mid, dark] = cx.pal.rim;
    let (p0, p1) = ends(a0, a1);
    if cx.detailed {
        let sh = linear(
            p0,
            p1,
            &[(0.0, dark), (0.25, light), (0.55, mid), (1.0, dark)],
        );
        cx.fill(path.as_ref(), &shader_paint(sh));
    } else {
        cx.color(path.as_ref(), cx.pal.edge);
    }
    // Ribs.
    let len = a1 - a0;
    let n = 9;
    let rib = if cx.detailed {
        [0, 0, 0, 70]
    } else {
        mix(cx.pal.edge, hex("#000000"), 0.25)
    };
    for i in 1..n {
        let t = a0 + len * f64::from(i) / f64::from(n);
        let w = len * 0.035;
        let rr = if vertical {
            Rect::new(r.x, t - w / 2.0, r.w, w)
        } else {
            Rect::new(t - w / 2.0, r.y, w, r.h)
        };
        cx.color(shapes::plain(rr).as_ref(), rib);
    }
}

/// Crown guard: a raised part of the housing around crown and button.
fn guard(cx: &mut Ctx<'_, '_>, r: Rect, vertical: bool) {
    let path = shapes::round(r, r.w.min(r.h) * 0.5);
    if cx.detailed {
        let [light, mid, dark] = cx.pal.rim;
        let (p0, p1) = if vertical {
            ((0.0, r.y as f32), (0.0, r.bottom() as f32))
        } else {
            ((r.x as f32, 0.0), (r.right() as f32, 0.0))
        };
        let sh = linear(
            p0,
            p1,
            &[(0.0, light), (0.3, mid), (0.75, dark), (1.0, mid)],
        );
        cx.fill(path.as_ref(), &shader_paint(sh));
    } else {
        cx.color(path.as_ref(), cx.pal.body);
        cx.stroke(path.as_ref(), &solid(cx.pal.edge), r.w.min(r.h) * 0.08);
    }
}

/// Fills `path` (local lu) clipped to the housing outline.
fn fill_in_body(
    cx: &mut Ctx<'_, '_>,
    path: Option<&tiny_skia::Path>,
    paint: &tiny_skia::Paint<'_>,
) {
    let (Some(path), Some(body)) = (path, cx.l.body_path()) else {
        return;
    };
    let Some(mut mask) = Mask::new(cx.pm.width(), cx.pm.height()) else {
        return;
    };
    mask.fill_path(&body, FillRule::Winding, true, cx.ts);
    cx.pm
        .fill_path(path, paint, FillRule::Winding, cx.ts, Some(&mask));
}

/// Closed foldable: the hinge spine along the (model) left edge, a band
/// shaded like a cylinder, clipped to the housing.
fn hinge_spine(cx: &mut Ctx<'_, '_>) {
    let d = cx.l.device;
    let Some(h) = d.hinge.filter(|h| h.at == HingeAt::Left) else {
        return;
    };
    let band = cx.l.rect_xywh(0.0, 0.0, h.w, d.body.h);
    let vertical = band.h > band.w;
    let (p0, p1) = if vertical {
        ((band.x as f32, 0.0), (band.right() as f32, 0.0))
    } else {
        ((0.0, band.y as f32), (0.0, band.bottom() as f32))
    };
    let [light, mid, dark] = cx.pal.rim;
    let paint = if cx.detailed {
        shader_paint(linear(
            p0,
            p1,
            &[
                (0.0, dark),
                (0.3, light),
                (0.6, mid),
                (0.92, dark),
                (1.0, hex("#000000")),
            ],
        ))
    } else {
        let e = cx.pal.edge;
        shader_paint(linear(
            p0,
            p1,
            &[(0.0, e), (0.8, e), (1.0, mix(e, hex("#000000"), 0.3))],
        ))
    };
    fill_in_body(cx, shapes::plain(band).as_ref(), &paint);
}

/// Open foldable: a faint crease over the screenshot along the hinge and
/// small seams where the hinge meets the bezel (top and bottom, natural
/// orientation).
fn crease(cx: &mut Ctx<'_, '_>) {
    let d = cx.l.device;
    let Some(h) = d.hinge.filter(|h| h.at == HingeAt::Center) else {
        return;
    };
    let disp = d.display;
    let cxm = disp.x + disp.w / 2.0;
    let r = cx.l.rect_xywh(cxm - h.w / 2.0, disp.y, h.w, disp.h);
    let vertical = r.h > r.w;
    let (p0, p1) = if vertical {
        ((r.x as f32, 0.0), (r.right() as f32, 0.0))
    } else {
        ((0.0, r.y as f32), (0.0, r.bottom() as f32))
    };
    let k: f32 = if cx.detailed { 1.0 } else { 0.6 };
    let a = |v: f32| (v * k).round() as u8;
    let sh = linear(
        p0,
        p1,
        &[
            (0.0, [0, 0, 0, 0]),
            (0.3, [0, 0, 0, a(26.0)]),
            (0.5, [255, 255, 255, a(30.0)]),
            (0.7, [0, 0, 0, a(26.0)]),
            (1.0, [0, 0, 0, 0]),
        ],
    );
    if let Some(screen) = cx.l.screen_path() {
        let Some(mut mask) = Mask::new(cx.pm.width(), cx.pm.height()) else {
            return;
        };
        mask.fill_path(&screen, FillRule::Winding, true, cx.ts);
        if let Some(p) = shapes::plain(r) {
            cx.pm
                .fill_path(&p, &shader_paint(sh), FillRule::Winding, cx.ts, Some(&mask));
        }
    }
    // Seams in the bezel above and below the display.
    let seam = mix(face(cx), hex("#000000"), 0.3);
    let sw = (h.w * 0.4).max(d.rim * 0.5);
    for (y0, y1) in [(0.0, disp.y), (disp.bottom(), d.body.h)] {
        let s = cx.l.rect_xywh(cxm - sw / 2.0, y0, sw, y1 - y0);
        fill_in_body(cx, shapes::plain(s).as_ref(), &solid(seam));
    }
}

fn housing(cx: &mut Ctx<'_, '_>) {
    let b = cx.l.body;
    let body = cx.l.body_path();
    // Visible metal rim (falls back to a thin edge for rim-less data).
    let rim = cx.l.rim().max(b.w.min(b.h) * 0.008);
    let glass_rect = inset(b, rim);
    let glass = cx.l.body_inset(glass_rect, rim);
    if cx.detailed {
        let [light, mid, dark] = cx.pal.rim;
        // Diagonal material gradient, lit from the top left.
        let sh = linear(
            (b.x as f32, b.y as f32),
            (b.right() as f32, b.bottom() as f32),
            &[
                (0.0, light),
                (0.18, mid),
                (0.55, dark),
                (0.82, mid),
                (1.0, light),
            ],
        );
        // Only the ring is filled (the glass covers the rest).
        let e = rim * 0.3;
        let inner = cx.l.body_inset(inset(glass_rect, e), rim + e);
        let ring = match (&body, &inner) {
            (Some(o), Some(i)) => shapes::ring(o, i),
            _ => body.clone(),
        };
        cx.fill_even_odd(ring.as_ref(), &shader_paint(sh));
        // Polished outer edge.
        let ins = rim * 0.2;
        let edge = cx.l.body_inset(inset(b, ins), ins);
        let hl = linear(
            (b.x as f32, b.y as f32),
            (b.right() as f32, b.bottom() as f32),
            &[
                (0.0, [255, 255, 255, 200]),
                (0.35, [255, 255, 255, 40]),
                (0.65, [255, 255, 255, 25]),
                (1.0, [255, 255, 255, 140]),
            ],
        );
        cx.stroke(edge.as_ref(), &shader_paint(hl), rim * 0.28);
        cx.color(glass.as_ref(), cx.pal.glass);
    } else {
        cx.color(body.as_ref(), cx.pal.body);
        let face = face(cx);
        if face != cx.pal.body {
            cx.color(glass.as_ref(), face);
        }
        // A hairline separates the rim from the face.
        let line = shade(cx.pal.edge, -0.04);
        cx.stroke(glass.as_ref(), &solid(line), rim * 0.3);
    }
}

/// Island, notch and punch-hole: drawn over the screenshot.
fn display_cutouts(cx: &mut Ctx<'_, '_>) {
    let d = cx.l.device;
    let face = face(cx);
    for c in d.cutouts.iter().filter(|c| c.kind.over_display()) {
        match (c.kind, c.shape) {
            (CutoutKind::Notch, _) | (_, CutoutShape::Notch) => {
                // Cutouts are always black. On a dark face the notch
                // extends up into the bezel (to the glass edge) so it joins
                // the face without a seam; on a light face it starts at the
                // display edge.
                let top = if luma(face) < 0.25 {
                    d.rim.min(c.y)
                } else {
                    c.y
                };
                let p = cx.l.path(shapes::notch(c.x, c.y, c.w, c.h, c.radius, top));
                cx.color(p.as_ref(), hex("#000000"));
                if cx.detailed {
                    // Speaker slit and front camera inside the notch.
                    let sw = c.w * 0.26;
                    let sh = c.h * 0.16;
                    let sr =
                        cx.l.rect_xywh(c.x + (c.w - sw) / 2.0, c.y + c.h * 0.22, sw, sh);
                    cx.color(shapes::pill(sr).as_ref(), hex("#1B1C20"));
                    let (lx, ly) = cx.l.pt(c.x + c.w * 0.72, c.y + c.h * 0.45);
                    cx.lens(lx, ly, c.h * 0.17 * cx.l.s);
                }
            }
            (_, CutoutShape::Circle) => {
                let r = c.w.min(c.h) / 2.0;
                let (x, y) = cx.l.pt(c.x + c.w / 2.0, c.y + c.h / 2.0);
                let rs = r * cx.l.s;
                cx.color(shapes::circle(x, y, rs).as_ref(), hex("#000000"));
                if cx.detailed {
                    cx.lens(x, y, rs * 0.62);
                }
            }
            _ => {
                let r = cx.l.rect(&c.rect());
                cx.color(shapes::pill(r).as_ref(), hex("#000000"));
                if cx.detailed {
                    // Lens at the (natural) right end of the pill.
                    let (lx, ly) = cx.l.pt(c.right_end(), c.y + c.h / 2.0);
                    cx.lens(lx, ly, c.h * 0.30 * cx.l.s);
                }
            }
        }
    }
}

trait CutoutExt {
    fn right_end(&self) -> f64;
}

impl CutoutExt for arrangia_core::catalog::Cutout {
    fn right_end(&self) -> f64 {
        self.x + self.w - self.h / 2.0
    }
}

/// Camera, speaker and home button in the bezel.
fn bezel_features(cx: &mut Ctx<'_, '_>) {
    let d = cx.l.device;
    let face = face(cx);
    let s = cx.l.s;
    for c in d.cutouts.iter().filter(|c| !c.kind.over_display()) {
        let (x, y) = cx.l.pt(c.x + c.w / 2.0, c.y + c.h / 2.0);
        let r = c.w.min(c.h) / 2.0 * s;
        match c.kind {
            CutoutKind::Camera => {
                if cx.detailed {
                    cx.color(
                        shapes::circle(x, y, r * 1.25).as_ref(),
                        mix(face, hex("#000000"), 0.5),
                    );
                    cx.lens(x, y, r);
                } else {
                    cx.color(shapes::circle(x, y, r).as_ref(), contrast(face, 0.22));
                }
            }
            CutoutKind::Speaker => {
                let rect = cx.l.rect(&c.rect());
                let col = if cx.detailed {
                    mix(face, hex("#000000"), 0.55)
                } else {
                    contrast(face, 0.16)
                };
                cx.color(shapes::pill(rect).as_ref(), col);
                if cx.detailed {
                    cx.stroke(
                        shapes::pill(rect).as_ref(),
                        &solid([255, 255, 255, 22]),
                        rect.w.min(rect.h) * 0.12,
                    );
                }
            }
            CutoutKind::HomeButton => home_button(cx, x, y, r, face),
            _ => {}
        }
    }
}

fn home_button(cx: &mut Ctx<'_, '_>, x: f64, y: f64, r: f64, face: Rgba) {
    let circle = shapes::circle(x, y, r);
    if cx.detailed {
        // Slightly concave glass: darker towards the top left.
        let sh = radial(
            ((x + r * 0.25) as f32, (y + r * 0.3) as f32),
            (r * 1.3) as f32,
            &[
                (0.0, shade(face, 0.04)),
                (1.0, mix(face, hex("#000000"), 0.12)),
            ],
        );
        cx.fill(circle.as_ref(), &shader_paint(sh));
        let [light, mid, dark] = cx.pal.rim;
        let ring = linear(
            ((x - r) as f32, (y - r) as f32),
            ((x + r) as f32, (y + r) as f32),
            &[(0.0, dark), (0.5, mid), (1.0, light)],
        );
        cx.stroke(
            shapes::circle(x, y, r * 0.97).as_ref(),
            &shader_paint(ring),
            r * 0.07,
        );
    } else {
        cx.stroke(
            shapes::circle(x, y, r * 0.96).as_ref(),
            &solid(contrast(face, 0.2)),
            r * 0.07,
        );
    }
}

/// Glass sheen and glass edge (`detailed`).
fn glass_finish(cx: &mut Ctx<'_, '_>) {
    let b = cx.l.body;
    let rim = cx.l.rim().max(b.w.min(b.h) * 0.008);
    let g = inset(b, rim);
    let glass = cx.l.body_inset(g, rim);
    let sheen = linear(
        (g.x as f32, g.y as f32),
        ((g.x + g.w * 0.9) as f32, (g.y + g.w * 1.1) as f32),
        &[
            (0.0, [255, 255, 255, 24]),
            (0.40, [255, 255, 255, 9]),
            (0.42, [255, 255, 255, 0]),
            (1.0, [255, 255, 255, 0]),
        ],
    );
    cx.fill(glass.as_ref(), &shader_paint(sheen));
    cx.stroke(glass.as_ref(), &solid([255, 255, 255, 28]), rim * 0.18);
    // Subtle inner edge of the display (where the panel meets the glass).
    let screen = cx.l.screen_path();
    cx.stroke(screen.as_ref(), &solid([0, 0, 0, 70]), rim * 0.12);
}
