//! Parametric device frames (own vector artwork, no vendor graphics).
//!
//! Geometry is derived from a handful of ratios (catalog data in Phase 1,
//! hard-coded here) and drawn in two styles (`docs/KONZEPT.md` §4.1):
//! `stylized` (flat, modern) and `detailed` (rim highlights, material
//! gradients, buttons, camera details, glass sheen).
//!
//! All coordinates are local lu with the device centre at (0, 0); callers pass
//! a transform placing the device on the canvas (translation, rotation,
//! content scale, lu → px).

#![allow(
    clippy::many_single_char_names,
    clippy::too_many_lines,
    reason = "parametric drawing code with short geometric names"
)]

use tiny_skia::{FillRule, FilterQuality, Path, Pattern, Pixmap, SpreadMode, Stroke, Transform};

use crate::geometry::Rect;
use crate::paint::{
    Rgba, circle, hex, linear, radial, round_rect, shader_paint, smooth_rect, solid, with_alpha,
};

/// Device model of the spike (catalog IDs from `docs/KONZEPT.md` §4.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceModel {
    /// Phone with Dynamic Island (`iphone-island`), portrait.
    PhoneIsland,
    /// Modern tablet without home button (`ipad-modern`), landscape.
    TabletLandscape,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FrameStyle {
    Stylized,
    #[default]
    Detailed,
}

/// Colour variant of a frame.
#[derive(Debug, Clone, Copy)]
pub struct FrameColors {
    /// Housing/rim: light and dark end of the material gradient.
    pub rim_light: Rgba,
    pub rim_mid: Rgba,
    pub rim_dark: Rgba,
    /// Glass border around the display (`detailed`), or housing face (`stylized`).
    pub bezel: Rgba,
}

impl FrameColors {
    /// Dark titanium.
    pub fn graphite() -> Self {
        Self {
            rim_light: hex("#8A8A90"),
            rim_mid: hex("#3C3C41"),
            rim_dark: hex("#1B1B1E"),
            bezel: hex("#060607"),
        }
    }

    /// Light natural titanium.
    pub fn silver() -> Self {
        Self {
            rim_light: hex("#F2F1EE"),
            rim_mid: hex("#BEBCB7"),
            rim_dark: hex("#7E7C78"),
            bezel: hex("#060607"),
        }
    }

    /// Flat counterpart for the `stylized` style (light or dark by luminance).
    #[must_use]
    pub fn flat(&self) -> Self {
        let [r, g, b, _] = self.rim_mid;
        let lum = 0.2126 * f32::from(r) + 0.7152 * f32::from(g) + 0.0722 * f32::from(b);
        if lum > 128.0 {
            Self::porcelain()
        } else {
            Self::midnight()
        }
    }

    /// Flat light housing for `stylized`.
    pub fn porcelain() -> Self {
        Self {
            rim_light: hex("#FFFFFF"),
            rim_mid: hex("#F4F4F2"),
            rim_dark: hex("#D9D9D6"),
            bezel: hex("#F4F4F2"),
        }
    }

    /// Flat dark housing for `stylized`.
    pub fn midnight() -> Self {
        Self {
            rim_light: hex("#2C2C30"),
            rim_mid: hex("#1C1C1F"),
            rim_dark: hex("#111113"),
            bezel: hex("#1C1C1F"),
        }
    }
}

/// A hardware button on the housing edge.
#[derive(Debug, Clone, Copy)]
pub struct Button {
    /// Full rectangle including the part hidden behind the housing.
    pub rect: Rect,
}

/// Derived frame geometry in local lu (centre at the origin).
#[derive(Debug, Clone)]
pub struct DeviceGeometry {
    pub model: DeviceModel,
    pub body: Rect,
    pub body_r: f64,
    /// Thickness of the metal rim.
    pub rim: f64,
    /// Glass area (inside the rim).
    pub glass: Rect,
    pub glass_r: f64,
    /// Visible display area.
    pub screen: Rect,
    pub screen_r: f64,
    /// Dynamic Island (pill) in screen space, if any.
    pub island: Option<Rect>,
    /// Front camera in the bezel (centre x, y, radius), if any.
    pub bezel_camera: Option<(f64, f64, f64)>,
    pub buttons: Vec<Button>,
}

impl DeviceGeometry {
    /// Geometry for `model` with overall width `width` lu (long side for a
    /// landscape tablet). `screen_aspect` = screen height / width as the
    /// screenshot will be shown (portrait phone ≈ 2.17, landscape tablet 0.75).
    pub fn new(model: DeviceModel, width: f64, screen_aspect: f64) -> Self {
        match model {
            DeviceModel::PhoneIsland => Self::phone(width, screen_aspect),
            DeviceModel::TabletLandscape => Self::tablet(width, screen_aspect),
        }
    }

    fn phone(w: f64, aspect: f64) -> Self {
        let rim = 0.0135 * w;
        let bezel = 0.033 * w;
        let sw = w - 2.0 * (rim + bezel);
        let sh = sw * aspect;
        let h = sh + 2.0 * (rim + bezel);
        let body = Rect::new(-w / 2.0, -h / 2.0, w, h);
        let body_r = 0.17 * w;
        let glass = Rect::new(body.x + rim, body.y + rim, w - 2.0 * rim, h - 2.0 * rim);
        let screen = Rect::new(-sw / 2.0, -sh / 2.0, sw, sh);
        let island_w = 0.285 * sw;
        let island_h = 0.083 * sw;
        let island = Rect::new(-island_w / 2.0, screen.y + 0.026 * sw, island_w, island_h);
        let bt = 0.016 * w; // button thickness (incl. hidden part)
        let out = 0.0085 * w; // protrusion
        let side_l = body.x - out;
        let side_r = body.right() + out - bt;
        let at = |frac: f64| body.y + frac * h;
        let buttons = vec![
            // action button
            Button {
                rect: Rect::new(side_l, at(0.165), bt, 0.05 * h),
            },
            // volume up / down
            Button {
                rect: Rect::new(side_l, at(0.255), bt, 0.085 * h),
            },
            Button {
                rect: Rect::new(side_l, at(0.36), bt, 0.085 * h),
            },
            // side (power) button
            Button {
                rect: Rect::new(side_r, at(0.27), bt, 0.13 * h),
            },
        ];
        Self {
            model: DeviceModel::PhoneIsland,
            body,
            body_r,
            rim,
            glass,
            glass_r: body_r - rim,
            screen,
            screen_r: body_r - rim - bezel,
            island: Some(island),
            bezel_camera: None,
            buttons,
        }
    }

    fn tablet(w: f64, aspect: f64) -> Self {
        let rim = 0.0065 * w;
        let bezel = 0.026 * w;
        let sw = w - 2.0 * (rim + bezel);
        let sh = sw * aspect;
        let h = sh + 2.0 * (rim + bezel);
        let body = Rect::new(-w / 2.0, -h / 2.0, w, h);
        let body_r = 0.05 * w;
        let glass = Rect::new(body.x + rim, body.y + rim, w - 2.0 * rim, h - 2.0 * rim);
        let screen = Rect::new(-sw / 2.0, -sh / 2.0, sw, sh);
        let bt = 0.011 * w;
        let out = 0.005 * w;
        let top = body.y - out;
        let buttons = vec![
            // volume buttons on the top edge, right side
            Button {
                rect: Rect::new(body.right() - 0.25 * w, top, 0.06 * w, bt),
            },
            Button {
                rect: Rect::new(body.right() - 0.175 * w, top, 0.06 * w, bt),
            },
            // top button on the right short edge
            Button {
                rect: Rect::new(body.right() + out - bt, body.y + 0.08 * h, bt, 0.1 * h),
            },
        ];
        Self {
            model: DeviceModel::TabletLandscape,
            body,
            body_r,
            rim,
            glass,
            glass_r: body_r - rim,
            screen,
            screen_r: (body_r - rim - bezel).max(0.012 * w),
            island: None,
            bezel_camera: Some((0.0, body.y + rim + bezel / 2.0, 0.0045 * w)),
            buttons,
        }
    }

    /// Housing outline (used for the drop shadow).
    pub fn body_path(&self) -> Option<Path> {
        rr(self.body, self.body_r)
    }

    /// Display outline (screenshot mask).
    pub fn screen_path(&self) -> Option<Path> {
        rr(self.screen, self.screen_r)
    }
}

fn rr(r: Rect, radius: f64) -> Option<Path> {
    smooth_rect(
        r.x as f32,
        r.y as f32,
        r.w as f32,
        r.h as f32,
        radius as f32,
    )
}

fn f(v: f64) -> f32 {
    v as f32
}

/// Draws a device: housing, buttons, bezel, screenshot (masked to the
/// display with its corner radius), island/camera and — for `detailed` —
/// highlights and glass sheen.
///
/// `screenshot` must already be resampled (Lanczos3) to roughly the pixel
/// size of the screen; it is mapped onto the screen rect with bicubic
/// filtering, which then only has to handle rotation and sub-pixel scaling.
pub fn draw_device(
    pm: &mut Pixmap,
    g: &DeviceGeometry,
    style: FrameStyle,
    colors: &FrameColors,
    screenshot: Option<&Pixmap>,
    ts: Transform,
) {
    let detailed = style == FrameStyle::Detailed;
    let b = g.body;

    // 1. Buttons (behind the housing).
    for btn in &g.buttons {
        let r = btn.rect;
        let Some(p) = round_rect(f(r.x), f(r.y), f(r.w), f(r.h), f(r.w.min(r.h) * 0.45)) else {
            continue;
        };
        if detailed {
            let horizontal = r.w < r.h; // side buttons: gradient across x
            let (p0, p1) = if horizontal {
                ((f(r.x), 0.0), (f(r.right()), 0.0))
            } else {
                ((0.0, f(r.y)), (0.0, f(r.bottom())))
            };
            let sh = linear(
                p0,
                p1,
                &[
                    (0.0, colors.rim_dark),
                    (0.45, colors.rim_light),
                    (1.0, colors.rim_dark),
                ],
            );
            pm.fill_path(&p, &shader_paint(sh), FillRule::Winding, ts, None);
        } else {
            pm.fill_path(&p, &solid(colors.rim_dark), FillRule::Winding, ts, None);
        }
    }

    // 2. Housing.
    let Some(body) = g.body_path() else { return };
    if detailed {
        // Diagonal material gradient: lit from the top left.
        let sh = linear(
            (f(b.x), f(b.y)),
            (f(b.right()), f(b.bottom())),
            &[
                (0.0, colors.rim_light),
                (0.18, colors.rim_mid),
                (0.55, colors.rim_dark),
                (0.82, colors.rim_mid),
                (1.0, colors.rim_light),
            ],
        );
        // Only the visible ring (housing minus glass) is filled; the glass is
        // painted on top anyway, and this saves a full-area gradient pass.
        let gi = g.glass;
        let e = g.rim * 0.3;
        let ring = rr(
            Rect::new(gi.x + e, gi.y + e, gi.w - 2.0 * e, gi.h - 2.0 * e),
            g.glass_r - e,
        )
        .and_then(|inner| {
            let mut pb = tiny_skia::PathBuilder::new();
            pb.push_path(&body);
            pb.push_path(&inner);
            pb.finish()
        });
        pm.fill_path(
            ring.as_ref().unwrap_or(&body),
            &shader_paint(sh),
            FillRule::EvenOdd,
            ts,
            None,
        );
        // Polished outer edge.
        let stroke = Stroke {
            width: f(g.rim * 0.28),
            ..Stroke::default()
        };
        let inset = g.rim * 0.2;
        if let Some(edge) = rr(
            Rect::new(
                b.x + inset,
                b.y + inset,
                b.w - 2.0 * inset,
                b.h - 2.0 * inset,
            ),
            g.body_r - inset,
        ) {
            let hl = linear(
                (f(b.x), f(b.y)),
                (f(b.right()), f(b.bottom())),
                &[
                    (0.0, [255, 255, 255, 200]),
                    (0.35, [255, 255, 255, 40]),
                    (0.65, [255, 255, 255, 25]),
                    (1.0, [255, 255, 255, 140]),
                ],
            );
            pm.stroke_path(&edge, &shader_paint(hl), &stroke, ts, None);
        }
    } else {
        pm.fill_path(&body, &solid(colors.rim_mid), FillRule::Winding, ts, None);
    }

    // 3. Glass / bezel.
    let Some(glass) = rr(g.glass, g.glass_r) else {
        return;
    };
    if detailed {
        pm.fill_path(&glass, &solid(colors.bezel), FillRule::Winding, ts, None);
    } else {
        // Stylized: the housing face is the bezel; a hairline separates the rim.
        let stroke = Stroke {
            width: f(g.rim * 0.25),
            ..Stroke::default()
        };
        pm.fill_path(&glass, &solid(colors.bezel), FillRule::Winding, ts, None);
        pm.stroke_path(
            &glass,
            &solid(with_alpha(colors.rim_dark, 0.6)),
            &stroke,
            ts,
            None,
        );
    }

    // 4. Screenshot, masked to the display shape.
    let Some(screen) = g.screen_path() else {
        return;
    };
    let s = g.screen;
    if let Some(img) = screenshot {
        let pat_ts = Transform::from_row(
            f(s.w) / img.width() as f32,
            0.0,
            0.0,
            f(s.h) / img.height() as f32,
            f(s.x),
            f(s.y),
        );
        let pat = Pattern::new(
            img.as_ref(),
            SpreadMode::Pad,
            FilterQuality::Bilinear,
            1.0,
            pat_ts,
        );
        pm.fill_path(&screen, &shader_paint(pat), FillRule::Winding, ts, None);
    } else {
        pm.fill_path(&screen, &solid(hex("#2A2A2E")), FillRule::Winding, ts, None);
    }

    // 5. Island / front camera (drawn over the screenshot).
    if let Some(isl) = g.island {
        if let Some(p) = round_rect(f(isl.x), f(isl.y), f(isl.w), f(isl.h), f(isl.h / 2.0)) {
            pm.fill_path(&p, &solid(hex("#000000")), FillRule::Winding, ts, None);
        }
        if detailed {
            let r = isl.h * 0.30;
            let (cx, cy) = (isl.right() - isl.h * 0.5, isl.y + isl.h / 2.0);
            lens(pm, cx, cy, r, ts);
        }
    }
    if let Some((cx, cy, r)) = g.bezel_camera {
        if detailed {
            lens(pm, cx, cy, r, ts);
        } else if let Some(p) = circle(f(cx), f(cy), f(r * 0.8)) {
            pm.fill_path(
                &p,
                &solid(with_alpha(hex("#000000"), 0.35)),
                FillRule::Winding,
                ts,
                None,
            );
        }
    }

    // 6. Glass sheen and edge (detailed only).
    if detailed {
        let gl = g.glass;
        let sheen = linear(
            (f(gl.x), f(gl.y)),
            (f(gl.x + gl.w * 0.9), f(gl.y + gl.w * 1.1)),
            &[
                (0.0, [255, 255, 255, 24]),
                (0.40, [255, 255, 255, 9]),
                (0.42, [255, 255, 255, 0]),
                (1.0, [255, 255, 255, 0]),
            ],
        );
        pm.fill_path(&glass, &shader_paint(sheen), FillRule::Winding, ts, None);
        let stroke = Stroke {
            width: f(g.rim * 0.18),
            ..Stroke::default()
        };
        pm.stroke_path(&glass, &solid([255, 255, 255, 28]), &stroke, ts, None);
    }
}

fn lens(pm: &mut Pixmap, cx: f64, cy: f64, r: f64, ts: Transform) {
    let (x, y, r) = (f(cx), f(cy), f(r));
    if let Some(p) = circle(x, y, r) {
        let sh = radial(
            (x - r * 0.2, y - r * 0.2),
            r * 1.1,
            &[
                (0.0, hex("#2B3A5C")),
                (0.55, hex("#111827")),
                (1.0, hex("#05060A")),
            ],
        );
        pm.fill_path(&p, &shader_paint(sh), FillRule::Winding, ts, None);
    }
    if let Some(p) = circle(x - r * 0.32, y - r * 0.32, r * 0.22) {
        pm.fill_path(
            &p,
            &solid([170, 190, 255, 110]),
            FillRule::Winding,
            ts,
            None,
        );
    }
}
