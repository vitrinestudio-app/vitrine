//! Parametric device frames (own vector artwork, no vendor graphics).
//!
//! Every built-in frame is drawn from the catalog data in
//! `crates/core/data/devices.yaml` (KONZEPT §4.1/§4.2) in two styles:
//! `stylized` (flat, clean, few details) and `detailed` (material gradients,
//! polished rim, buttons, lens details, glass sheen). Imported frame packs
//! (`imported:*`) are not handled here; they fall back to `detailed`.
//!
//! Coordinates are **device-local lu**: the outer box (including laptop base
//! or desktop stand) is centred at (0, 0) and as wide as the element. A
//! portrait-natural device in landscape is the natural frame rotated 90°
//! counter-clockwise; its display is rotated with it, the screenshot is not
//! (KONZEPT §4.3), so the screen rect is landscape-shaped.
//!
//! Callers pass a transform placing the device (translation, rotation,
//! content scale, lu → px) and a screenshot already resampled to the
//! on-screen pixel size of [`DeviceGeometry::screen`].

#![allow(
    clippy::many_single_char_names,
    clippy::too_many_lines,
    reason = "parametric drawing code with short geometric names"
)]

mod browser;
mod computer;
pub mod glyphs;
mod handheld;
mod layout;
pub mod palette;
mod screen;
pub mod shapes;

use arrangia_core::geometry::{Rect, Vec2};
use arrangia_core::model::{FitMode, FrameStyle, Orientation};
use arrangia_core::resolve::{DeviceColor, ResolvedDevice};
use tiny_skia::{FillRule, Paint, Path, Pixmap, PixmapMut, Stroke, Transform};

use layout::{Kind, Layout};
use palette::Palette;

use crate::paint::{Rgba, hex, radial, shader_paint, solid};

/// Frame geometry in device-local lu (outer box centred at (0, 0)).
#[derive(Debug, Clone)]
pub struct DeviceGeometry {
    /// Outer box incl. laptop base / desktop stand (buttons excluded).
    pub outer: Rect,
    /// Where the screenshot goes (below the browser chrome, if any).
    pub screen: Rect,
    /// Corner radius of the screen's bottom corners (the top corners are
    /// square when browser chrome sits above the screenshot). Handhelds and
    /// `none` use continuous (squircle-like) corners of this radius.
    pub screen_radius: f32,
    /// Outer shape incl. laptop base / monitor stand, for shadows.
    pub silhouette: Path,
}

/// Outer size (lu) of `dev` for an element `width`. Agrees with the element
/// size `core::resolve` computes (catalog aspect; for `aspect: free` models
/// the display already follows the screenshot in `dev.device`), so scene
/// rendering simply passes the resolved element size; this is for callers
/// that build a [`ResolvedDevice`] themselves ([`catalog_device`]).
pub fn device_size(dev: &ResolvedDevice, width: f64) -> Vec2 {
    layout::outer_size(dev, width)
}

/// Geometry of `dev` with outer size `size` (lu, usually the resolved
/// element size). Fixed-aspect models use only `size.x` and take the height
/// from the catalog; `aspect: free` models use `size` as given.
pub fn device_geometry(dev: &ResolvedDevice, size: Vec2) -> DeviceGeometry {
    let l = Layout::new(dev, size);
    let silhouette = l
        .silhouette()
        .or_else(|| shapes::plain(l.outer))
        .unwrap_or_else(|| {
            // A degenerate (zero-size) device: a tiny box keeps the type simple.
            tiny_skia::PathBuilder::from_rect(
                tiny_skia::Rect::from_xywh(0.0, 0.0, 1e-3, 1e-3).expect("non-empty rect"),
            )
        });
    DeviceGeometry {
        outer: l.outer,
        screen: l.screen,
        screen_radius: l.screen_radii[2] as f32,
        silhouette,
    }
}

/// Draws the whole device: buttons, body, bezel, screen content masked with
/// its corner radius, cutouts (island / notch / punch-hole) over the
/// screenshot, bezel details, laptop hinge and base, desktop stand, browser
/// chrome.
///
/// `size` is the outer size in lu (see [`device_geometry`]). `screen` is the
/// screenshot already resampled (and fitted) to the on-screen pixel size of
/// [`DeviceGeometry::screen`] under `transform`; it is stretched onto the
/// screen rect. `None` draws a placeholder showing the screenshot key.
/// `transform` maps device-local lu to pixels.
pub fn draw_device(
    pixmap: &mut PixmapMut<'_>,
    dev: &ResolvedDevice,
    size: Vec2,
    screen: Option<&Pixmap>,
    transform: Transform,
) {
    let l = Layout::new(dev, size);
    if l.outer.w <= 0.0 || l.outer.h <= 0.0 {
        return;
    }
    let mut cx = Ctx {
        pm: pixmap,
        ts: transform,
        detailed: !matches!(dev.frame, FrameStyle::Stylized),
        pal: Palette::for_device(dev),
        dev,
        l,
    };
    match cx.l.kind {
        Kind::Handheld => handheld::draw(&mut cx, screen),
        Kind::Laptop => computer::laptop(&mut cx, screen),
        Kind::Desktop => computer::desktop(&mut cx, screen),
        Kind::Browser => browser::window(&mut cx, screen),
        Kind::Bare => {
            if let Some(p) = cx.l.screen_path() {
                let r = cx.l.screen;
                screen::content(&mut cx, &p, r, screen);
            }
        }
    }
}

/// A [`ResolvedDevice`] straight from the built-in catalog, for previews,
/// examples and tests (no screenshot, no shadow). `color`: a palette name
/// or `#RRGGBB`; `None` = the model's default colour.
pub fn catalog_device(
    model: &str,
    orientation: Orientation,
    style: FrameStyle,
    color: Option<&str>,
) -> Option<ResolvedDevice> {
    let catalog = arrangia_core::catalog::catalog();
    let device = catalog.device(model)?.clone();
    let name = color.or(device.default_color.as_deref());
    let (color, palette) = match name {
        Some(c) if c.starts_with('#') => (
            arrangia_core::model::Rgba::parse(c).map_or(DeviceColor::None, DeviceColor::Custom),
            None,
        ),
        Some(c) => (
            DeviceColor::Named(c.to_owned()),
            catalog.devices.colors.get(c).cloned(),
        ),
        None => (DeviceColor::None, None),
    };
    let orientation = if device.supports(orientation) {
        orientation
    } else {
        device.natural
    };
    Some(ResolvedDevice {
        model: model.to_owned(),
        mapped_from: None,
        free_aspect: device.screenshot.aspect.is_none(),
        device,
        orientation,
        frame: style,
        color,
        palette,
        chrome: None,
        url: None,
        appearance: None,
        screenshot: None,
        fit: FitMode::Cover,
        content_scale: 1.0,
        shadow: Vec::new(),
    })
}

/// `dev` with an `aspect: free` display (browser, `none`) adapted to a
/// screenshot of aspect `w / h`, the way `core::resolve` does it when the
/// screenshot's pixel size is known. Fixed-aspect models are unchanged.
#[must_use]
pub fn with_screenshot_aspect(mut dev: ResolvedDevice, aspect: f64) -> ResolvedDevice {
    if dev.free_aspect && aspect.is_finite() && aspect > 0.0 {
        let r = if dev.orientation == dev.device.natural {
            aspect
        } else {
            1.0 / aspect
        };
        dev.device = dev.device.with_display_aspect(r);
    }
    dev
}

/// Drawing context shared by the model-specific painters.
pub(crate) struct Ctx<'p, 'q> {
    pub pm: &'p mut PixmapMut<'q>,
    pub ts: Transform,
    pub l: Layout<'p>,
    pub pal: Palette,
    pub detailed: bool,
    pub dev: &'p ResolvedDevice,
}

impl Ctx<'_, '_> {
    pub fn fill(&mut self, path: Option<&Path>, paint: &Paint<'_>) {
        if let Some(p) = path {
            self.pm
                .fill_path(p, paint, FillRule::Winding, self.ts, None);
        }
    }

    pub fn fill_even_odd(&mut self, path: Option<&Path>, paint: &Paint<'_>) {
        if let Some(p) = path {
            self.pm
                .fill_path(p, paint, FillRule::EvenOdd, self.ts, None);
        }
    }

    pub fn color(&mut self, path: Option<&Path>, c: Rgba) {
        self.fill(path, &solid(c));
    }

    pub fn stroke(&mut self, path: Option<&Path>, paint: &Paint<'_>, width: f64) {
        if let Some(p) = path {
            let stroke = Stroke {
                width: width as f32,
                ..Stroke::default()
            };
            self.pm.stroke_path(p, paint, &stroke, self.ts, None);
        }
    }

    /// Screen content (screenshot or placeholder) inside the screen outline.
    pub fn screen(&mut self, img: Option<&Pixmap>) {
        if let Some(p) = self.l.screen_path() {
            let r = self.l.screen;
            screen::content(self, &p, r, img);
        }
    }

    /// Camera lens: dark glass with a bluish depth gradient and a small
    /// reflection.
    pub fn lens(&mut self, cx: f64, cy: f64, r: f64) {
        let (x, y, rf) = (cx as f32, cy as f32, r as f32);
        let sh = radial(
            (x - rf * 0.2, y - rf * 0.2),
            rf * 1.1,
            &[
                (0.0, hex("#2B3A5C")),
                (0.55, hex("#111827")),
                (1.0, hex("#05060A")),
            ],
        );
        self.fill(shapes::circle(cx, cy, r).as_ref(), &shader_paint(sh));
        self.stroke(
            shapes::circle(cx, cy, r * 0.92).as_ref(),
            &solid([255, 255, 255, 18]),
            r * 0.1,
        );
        self.color(
            shapes::circle(cx - r * 0.32, cy - r * 0.32, r * 0.22).as_ref(),
            [170, 190, 255, 110],
        );
    }
}

#[cfg(test)]
mod tests;
