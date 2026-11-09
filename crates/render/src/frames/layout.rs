//! Frame geometry: catalog model units → device-local lu.
//!
//! The catalog gives every model in its natural orientation with the origin
//! at the body's top-left corner. [`Layout`] scales that so the outer width
//! (including laptop base / desktop stand) equals the element width, centres
//! it at (0, 0) and — for a portrait-natural device shown in landscape —
//! rotates it 90° counter-clockwise (the top edge goes to the left). All
//! rects are then axis-aligned rects in device-local lu again, so drawing
//! code works in local space with a fixed light direction; only shapes that
//! are not symmetric (notch, lens offsets) are built in model space and
//! mapped with [`Layout::m`].

use arrangia_core::catalog::{self, BrowserChrome, Device, DeviceClass};
use arrangia_core::geometry::{Rect, Vec2};
use arrangia_core::model::{Chrome, Orientation};
use arrangia_core::resolve::ResolvedDevice;
use tiny_skia::{Path, PathBuilder, Point, Transform};

use super::shapes::{self, Radii};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Phone or tablet.
    Handheld,
    Laptop,
    Desktop,
    /// Stand-alone browser window.
    Browser,
    /// No frame: just the rounded screenshot.
    Bare,
}

/// Browser chrome placed in a rect: px units of `browser_chrome` → lu.
#[derive(Debug, Clone, Copy)]
pub struct ChromeArea {
    /// Tab bar + toolbar in device-local lu.
    pub rect: Rect,
    /// lu per chrome px.
    pub k: f64,
    /// Radius of the two top corners (lu).
    pub radius: f64,
}

#[derive(Debug, Clone)]
pub struct Layout<'a> {
    pub device: &'a Device,
    pub kind: Kind,
    /// Model units → device-local lu (incl. landscape rotation).
    pub m: Transform,
    /// lu per model unit.
    pub s: f64,
    pub outer: Rect,
    /// Housing (laptop: lid, desktop: screen housing, browser: window).
    pub body: Rect,
    pub body_r: f64,
    /// Whole display (incl. browser chrome when drawn inside it).
    pub display: Rect,
    pub display_r: f64,
    pub chrome: Option<ChromeArea>,
    /// Where the screenshot goes.
    pub screen: Rect,
    pub screen_radii: Radii,
}

/// Minimum lu per chrome px for `chrome: browser` on laptops / desktops.
const MIN_CHROME_K: f64 = 0.4;

/// Desktop stand outlines and their bounding rects (device-local lu).
#[derive(Debug, Clone)]
pub struct Stand {
    pub neck: Path,
    pub neck_rect: Rect,
    /// Top face of the foot.
    pub top: Path,
    pub top_rect: Rect,
    /// Front edge of the foot.
    pub front: Path,
    pub front_rect: Rect,
    /// How far the neck reaches up behind the housing (lu).
    pub tuck: f64,
}

/// Closed polygon (clockwise, y down) with per-vertex corner radii.
fn rounded_polygon(pts: &[(f64, f64)], radii: &[f64]) -> Option<Path> {
    let n = pts.len();
    let mut pb = PathBuilder::new();
    for i in 0..n {
        let (px, py) = pts[(i + n - 1) % n];
        let (x, y) = pts[i];
        let (nx, ny) = pts[(i + 1) % n];
        let r = radii.get(i).copied().unwrap_or(0.0);
        let towards = |ax: f64, ay: f64, d: f64| {
            let len = (ax - x).hypot(ay - y).max(1e-9);
            let d = d.min(len / 2.0);
            (x + (ax - x) / len * d, y + (ay - y) / len * d)
        };
        let (ax, ay) = towards(px, py, r);
        let (bx, by) = towards(nx, ny, r);
        if i == 0 {
            pb.move_to(ax as f32, ay as f32);
        } else {
            pb.line_to(ax as f32, ay as f32);
        }
        if r > 0.0 {
            pb.quad_to(x as f32, y as f32, bx as f32, by as f32);
        }
    }
    pb.close();
    pb.finish()
}

pub fn browser_chrome() -> &'static BrowserChrome {
    &catalog::catalog().devices.browser_chrome
}

/// Is `dev` drawn rotated (portrait-natural device in landscape)?
pub fn rotated(dev: &ResolvedDevice) -> bool {
    dev.device.natural == Orientation::Portrait
        && dev.orientation == Orientation::Landscape
        && dev.device.class != DeviceClass::None
}

pub fn kind(device: &Device) -> Kind {
    match device.class {
        DeviceClass::Phone | DeviceClass::Tablet => Kind::Handheld,
        DeviceClass::Laptop => Kind::Laptop,
        DeviceClass::Desktop => Kind::Desktop,
        DeviceClass::Browser => Kind::Browser,
        DeviceClass::None => Kind::Bare,
    }
}

/// Outer size (lu) of `dev` at `width` lu, exactly as `core::resolve`
/// sizes device elements: the catalog outer aspect of `dev.device` (for
/// `aspect: free` models already adapted to the screenshot), inverted when
/// the device is shown in its non-natural orientation.
pub fn outer_size(dev: &ResolvedDevice, width: f64) -> Vec2 {
    let o = dev.device.outer_size();
    let width = width.max(1e-6);
    let ratio = if o.w <= 0.0 || o.h <= 0.0 {
        1.0
    } else if dev.orientation == dev.device.natural {
        o.h / o.w
    } else {
        o.w / o.h
    };
    Vec2::new(width, width * ratio)
}

fn map_rect(m: Transform, r: &catalog::Rect) -> Rect {
    let mut pts = [
        Point::from_xy(r.x as f32, r.y as f32),
        Point::from_xy(r.right() as f32, r.bottom() as f32),
    ];
    m.map_points(&mut pts);
    let (x0, x1) = (pts[0].x.min(pts[1].x), pts[0].x.max(pts[1].x));
    let (y0, y1) = (pts[0].y.min(pts[1].y), pts[0].y.max(pts[1].y));
    Rect::new(
        f64::from(x0),
        f64::from(y0),
        f64::from(x1 - x0),
        f64::from(y1 - y0),
    )
}

impl<'a> Layout<'a> {
    /// Geometry of `dev` with outer size `size` (lu). Fixed-aspect models
    /// take only `size.x` and derive the height from the catalog; `aspect:
    /// free` models (browser, none) use `size.y` as given.
    pub fn new(dev: &'a ResolvedDevice, size: Vec2) -> Self {
        let d = &dev.device;
        let kind = kind(d);
        let w = size.x.max(1e-6);
        let h = if dev.free_aspect && size.y > 0.0 {
            size.y
        } else {
            outer_size(dev, w).y
        };
        let outer = Rect::centered(Vec2::ZERO, w, h);
        match kind {
            Kind::Browser => {
                let s = w / d.body.w;
                let chrome = browser_chrome();
                let k = w / chrome.reference_width;
                let ch = (chrome.height() * k).min(h * 0.5);
                let r = chrome.window_radius * k;
                let screen = Rect::new(outer.x, outer.y + ch, w, h - ch);
                Self {
                    device: d,
                    kind,
                    m: Transform::from_row(
                        s as f32,
                        0.0,
                        0.0,
                        s as f32,
                        outer.x as f32,
                        outer.y as f32,
                    ),
                    s,
                    outer,
                    body: outer,
                    body_r: r,
                    display: outer,
                    display_r: r,
                    chrome: Some(ChromeArea {
                        rect: Rect::new(outer.x, outer.y, w, ch),
                        k,
                        radius: r,
                    }),
                    screen,
                    screen_radii: [0.0, 0.0, r, r],
                }
            }
            Kind::Bare => {
                let s = w / d.body.w;
                // The catalog radius is relative to the nominal width (which
                // `with_display_aspect` never changes) and applies to the
                // short side, like a phone screen's corners.
                let r = (d.body.radius / d.body.w * w.min(h)).min(w.min(h) / 2.0);
                Self {
                    device: d,
                    kind,
                    m: Transform::from_row(
                        s as f32,
                        0.0,
                        0.0,
                        s as f32,
                        outer.x as f32,
                        outer.y as f32,
                    ),
                    s,
                    outer,
                    body: outer,
                    body_r: r,
                    display: outer,
                    display_r: r,
                    chrome: None,
                    screen: outer,
                    screen_radii: shapes::uniform(r),
                }
            }
            Kind::Handheld | Kind::Laptop | Kind::Desktop => {
                let o = d.outer_size();
                let rot = rotated(dev);
                let s = w / if rot { o.h } else { o.w };
                let (cx, cy) = (d.body.w / 2.0, o.h / 2.0);
                let m = if rot {
                    Transform::from_row(
                        0.0,
                        -s as f32,
                        s as f32,
                        0.0,
                        (-s * cy) as f32,
                        (s * cx) as f32,
                    )
                } else {
                    Transform::from_row(
                        s as f32,
                        0.0,
                        0.0,
                        s as f32,
                        (-s * cx) as f32,
                        (-s * cy) as f32,
                    )
                };
                let body = map_rect(m, &d.body_rect());
                let display = map_rect(m, &d.display);
                let display_r = d.display.radius * s;
                let with_chrome = dev.chrome == Some(Chrome::Browser)
                    && matches!(kind, Kind::Laptop | Kind::Desktop);
                let (chrome, screen, screen_radii) = if with_chrome {
                    let bc = browser_chrome();
                    // Scales with the display width, with a floor so tab and
                    // URL text stay legible on small laptops / monitors
                    // (URL text ≥ 5.6 lu ≈ 7 px on a 1320 px page).
                    let k = (display.w / bc.reference_width).max(MIN_CHROME_K);
                    let ch = (bc.height() * k).min(display.h * 0.5);
                    (
                        Some(ChromeArea {
                            rect: Rect::new(display.x, display.y, display.w, ch),
                            k,
                            radius: display_r,
                        }),
                        Rect::new(display.x, display.y + ch, display.w, display.h - ch),
                        [0.0, 0.0, display_r, display_r],
                    )
                } else {
                    (None, display, shapes::uniform(display_r))
                };
                Self {
                    device: d,
                    kind,
                    m,
                    s,
                    outer,
                    body,
                    body_r: d.body.radius * s,
                    display,
                    display_r,
                    chrome,
                    screen,
                    screen_radii,
                }
            }
        }
    }

    /// Model rect → local rect.
    pub fn rect(&self, r: &catalog::Rect) -> Rect {
        map_rect(self.m, r)
    }

    /// Model rect from components.
    pub fn rect_xywh(&self, x: f64, y: f64, w: f64, h: f64) -> Rect {
        self.rect(&catalog::Rect {
            x,
            y,
            w,
            h,
            radius: 0.0,
        })
    }

    /// Model point → local point.
    pub fn pt(&self, x: f64, y: f64) -> (f64, f64) {
        let mut p = [Point::from_xy(x as f32, y as f32)];
        self.m.map_points(&mut p);
        (f64::from(p[0].x), f64::from(p[0].y))
    }

    /// Model path → local path.
    pub fn path(&self, p: Option<Path>) -> Option<Path> {
        p?.transform(self.m)
    }

    /// Continuous corners for handhelds and the frameless screenshot,
    /// circular ones for the rest.
    pub fn smooth(&self) -> bool {
        matches!(self.kind, Kind::Handheld | Kind::Bare)
    }

    /// Screen (screenshot) outline.
    pub fn screen_path(&self) -> Option<Path> {
        shapes::rect(self.screen, self.screen_radii, self.smooth())
    }

    /// Model rim thickness in lu.
    pub fn rim(&self) -> f64 {
        self.device.rim * self.s
    }

    /// Laptop hinge and base rects (local), if any.
    pub fn laptop_parts(&self) -> Option<(Rect, Rect)> {
        let l = self.device.laptop.as_ref()?;
        let b = &self.device.body;
        let hinge = self.rect_xywh((b.w - l.hinge.w) / 2.0, b.h, l.hinge.w, l.hinge.h);
        let base = self.rect_xywh((b.w - l.base.w) / 2.0, b.h + l.hinge.h, l.base.w, l.base.h);
        Some((hinge, base))
    }

    /// Desktop stand (local): a plate neck tapering slightly towards the
    /// bottom with concave sides and a small flare where it meets the foot,
    /// standing on a foot slab seen slightly from above (top face + front
    /// edge). The neck reaches a little up behind the housing. The stand
    /// stays within the catalog outer box (neck + foot height).
    pub fn stand(&self) -> Option<Stand> {
        let st = self.device.stand.as_ref()?;
        let b = &self.device.body;
        let cx = b.w / 2.0;
        let (nw, nh) = (st.neck.w, st.neck.h);
        let (fw, fh) = (st.foot.w, st.foot.h);
        let y0 = b.h - nh * 0.15;
        let f = b.h + nh;
        // Depth of the visible top face of the foot.
        let tf = (fh * 2.2).min(nh * 0.22);
        let y1 = f - tf * 0.45;
        let (wt, wb) = (nw, nw * 0.86);
        let fr = (nw * 0.07).min(tf * 0.5);
        let p = |x: f64, y: f64| (x as f32, y as f32);
        // Clockwise (y down) like the other outlines, for the union.
        let mut pb = PathBuilder::new();
        let (l0, r0, lb, rb) = (cx - wt / 2.0, cx + wt / 2.0, cx - wb / 2.0, cx + wb / 2.0);
        let ctrl_y = y0 + (y1 - y0) * 0.6;
        pb.move_to(p(l0, y0).0, p(l0, y0).1);
        pb.line_to(p(r0, y0).0, p(r0, y0).1);
        let (a, c) = (p(rb, ctrl_y), p(rb, y1 - fr));
        pb.quad_to(a.0, a.1, c.0, c.1);
        let (a, c) = (p(rb, y1), p(rb + fr, y1));
        pb.quad_to(a.0, a.1, c.0, c.1);
        pb.line_to(p(lb - fr, y1).0, p(lb - fr, y1).1);
        let (a, c) = (p(lb, y1), p(lb, y1 - fr));
        pb.quad_to(a.0, a.1, c.0, c.1);
        let (a, c) = (p(lb, ctrl_y), p(l0, y0));
        pb.quad_to(a.0, a.1, c.0, c.1);
        pb.close();
        let neck = self.path(pb.finish())?;
        // Top face: a trapezoid, the back edge a little narrower, rounded
        // back corners.
        let back = fw * 0.44;
        let rb_ = (tf * 0.6).min(st.foot.radius.max(tf * 0.3));
        let top = self.path(rounded_polygon(
            &[
                (cx - back, f - tf),
                (cx + back, f - tf),
                (cx + fw / 2.0, f),
                (cx - fw / 2.0, f),
            ],
            &[rb_, rb_, 0.0, 0.0],
        ))?;
        let front_rect = self.rect_xywh(cx - fw / 2.0, f, fw, fh);
        let r = st.foot.radius * self.s;
        let front = shapes::rect(front_rect, [r * 0.3, r * 0.3, r, r], false)?;
        Some(Stand {
            neck,
            neck_rect: self.rect_xywh(l0, y0, wt, y1 - y0),
            top,
            top_rect: self.rect_xywh(cx - fw / 2.0, f - tf, fw, tf),
            front,
            front_rect,
            tuck: (b.h - y0) * self.s,
        })
    }

    /// Lid / housing outline.
    pub fn body_path(&self) -> Option<Path> {
        match self.kind {
            Kind::Handheld | Kind::Desktop => shapes::smooth(self.body, self.body_r),
            Kind::Laptop => {
                let r = self.body_r;
                shapes::rect(self.body, [r, r, r * 0.3, r * 0.3], true)
            }
            Kind::Browser => shapes::round(self.body, self.body_r),
            Kind::Bare => self.screen_path(),
        }
    }

    /// Laptop base outline: small top corners, `radius` bottom corners.
    pub fn base_path(&self, base: Rect) -> Option<Path> {
        let l = self.device.laptop.as_ref()?;
        let rb = l.base.radius * self.s;
        let rt = (base.h * 0.12).min(rb);
        shapes::rect(base, [rt, rt, rb, rb], false)
    }

    /// Outer shape incl. laptop base / desktop stand (for shadows).
    pub fn silhouette(&self) -> Option<Path> {
        match self.kind {
            Kind::Laptop => {
                let (hinge, base) = self.laptop_parts()?;
                shapes::union([self.body_path(), shapes::plain(hinge), self.base_path(base)])
            }
            Kind::Desktop => {
                let st = self.stand()?;
                shapes::union([
                    self.body_path(),
                    Some(st.neck),
                    Some(st.top),
                    Some(st.front),
                ])
            }
            _ => self.body_path(),
        }
    }
}
