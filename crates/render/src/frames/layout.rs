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
use tiny_skia::{Path, Point, Transform};

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

/// Outer size (lu) of `dev` at `width` lu: from the catalog aspect, or for
/// `aspect: free` models from the screenshot's `w / h` when given.
pub fn outer_size(dev: &ResolvedDevice, width: f64, screenshot_aspect: Option<f64>) -> Vec2 {
    let d = &dev.device;
    let width = width.max(1e-6);
    match (kind(d), screenshot_aspect.filter(|a| *a > 0.0)) {
        (Kind::Browser, Some(a)) => {
            let chrome = browser_chrome();
            let chrome_h = chrome.height() * width / chrome.reference_width;
            Vec2::new(width, chrome_h + width / a)
        }
        (Kind::Bare, Some(a)) => Vec2::new(width, width / a),
        (Kind::Browser | Kind::Bare, None) => Vec2::new(width, width * d.body.h / d.body.w),
        _ => {
            let o = d.outer_size();
            let ratio = if rotated(dev) { o.w / o.h } else { o.h / o.w };
            Vec2::new(width, width * ratio)
        }
    }
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
            outer_size(dev, w, None).y
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
                let r = d.body.radius / d.body.w.min(d.body.h) * w.min(h);
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
                    let k = display.w / bc.reference_width;
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

    /// Desktop stand neck and foot rects (local), if any. The neck reaches
    /// a little up behind the housing.
    pub fn stand_parts(&self) -> Option<(Rect, Rect)> {
        let st = self.device.stand.as_ref()?;
        let b = &self.device.body;
        let tuck = st.neck.h * 0.15;
        let neck = self.rect_xywh(
            (b.w - st.neck.w) / 2.0,
            b.h - tuck,
            st.neck.w,
            st.neck.h + tuck,
        );
        let foot = self.rect_xywh(
            (b.w - st.foot.w) / 2.0,
            b.h + st.neck.h,
            st.foot.w,
            st.foot.h,
        );
        Some((neck, foot))
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

    pub fn foot_path(&self, foot: Rect) -> Option<Path> {
        let st = self.device.stand.as_ref()?;
        shapes::round(foot, st.foot.radius * self.s)
    }

    /// Outer shape incl. laptop base / desktop stand (for shadows).
    pub fn silhouette(&self) -> Option<Path> {
        match self.kind {
            Kind::Laptop => {
                let (hinge, base) = self.laptop_parts()?;
                shapes::union([self.body_path(), shapes::plain(hinge), self.base_path(base)])
            }
            Kind::Desktop => {
                let (neck, foot) = self.stand_parts()?;
                shapes::union([self.body_path(), shapes::plain(neck), self.foot_path(foot)])
            }
            _ => self.body_path(),
        }
    }
}
