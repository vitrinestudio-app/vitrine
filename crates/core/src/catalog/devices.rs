//! Device models for the parametric frames (`data/devices.yaml`, KONZEPT §4.1).
//!
//! Data only; drawing is done in `vitrine-render`.

use std::collections::BTreeMap;

use serde::Deserialize;

use super::{Catalog, aspect_matches, oriented_ratio};
use crate::model::{Aspect, Fold, Orientation, PixelSize, Rgba, is_valid_id};

/// `devices.yaml`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DevicesFile {
    /// Named colour variants.
    pub colors: BTreeMap<String, ColorVariant>,
    pub browser_chrome: BrowserChrome,
    pub devices: Vec<Device>,
}

/// A colour variant of a frame.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ColorVariant {
    pub label: String,
    /// Flat housing colour (`stylized`).
    pub body: Rgba,
    /// Flat edge colour (`stylized`).
    pub edge: Rgba,
    /// Material gradient light / mid / dark (`detailed`).
    pub rim: [Rgba; 3],
    /// Front glass around the display (`detailed`).
    pub glass: Rgba,
}

/// Browser window chrome (px at `reference_width` content width).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BrowserChrome {
    pub reference_width: f64,
    pub tab_bar: f64,
    pub toolbar: f64,
    pub window_radius: f64,
    pub tab: BrowserTab,
    pub url_field: UrlField,
    pub traffic_lights: TrafficLights,
    pub themes: BTreeMap<String, BrowserTheme>,
}

/// Minimum lu per chrome px for `chrome: browser` on laptops / desktops:
/// the chrome scales with the display width, with a floor so tab and URL
/// text stay legible on small laptops / monitors.
pub const MIN_CHROME_SCALE: f64 = 0.4;

impl BrowserChrome {
    /// Total chrome height above the content.
    pub fn height(&self) -> f64 {
        self.tab_bar + self.toolbar
    }

    /// Scale (lu per chrome px) and height in lu of the browser bars drawn
    /// into a laptop / desktop display of `display_w` × `display_h` lu
    /// (`chrome: browser`). The screenshot gets the rest of the display.
    pub fn in_display(&self, display_w: f64, display_h: f64) -> (f64, f64) {
        let k = (display_w / self.reference_width).max(MIN_CHROME_SCALE);
        (k, (self.height() * k).min(display_h * 0.5))
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BrowserTab {
    pub width: f64,
    pub radius: f64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UrlField {
    pub height: f64,
    pub radius: f64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrafficLights {
    pub left: f64,
    pub diameter: f64,
    pub spacing: f64,
}

/// Colours of the browser chrome (`light` / `dark`).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BrowserTheme {
    pub frame: Rgba,
    pub toolbar: Rgba,
    pub tab: Rgba,
    pub url_field: Rgba,
    pub text: Rgba,
    pub muted: Rgba,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeviceClass {
    Phone,
    Tablet,
    Laptop,
    Desktop,
    Browser,
    /// Smartwatch: drawn like a handheld, with a crown.
    Watch,
    None,
}

/// Where a foldable's hinge shows (`hinge:` in `devices.yaml`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HingeAt {
    /// Vertical hinge through the body centre (natural orientation): a
    /// crease line on the display and hinge marks in the top and bottom
    /// bezel (open book-style foldable).
    Center,
    /// Spine along the left body edge (closed book-style foldable).
    Left,
}

/// Hinge of a foldable (model units).
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Hinge {
    pub at: HingeAt,
    /// Width of the hinge band / crease.
    pub w: f64,
}

/// The other fold state of a foldable: the catalog model drawn instead.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FoldLink {
    /// This model's own state.
    pub state: Fold,
    /// Model id of the other state.
    pub other: String,
}

/// Width, height and corner radius (model units).
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Size {
    pub w: f64,
    pub h: f64,
    #[serde(default)]
    pub radius: f64,
}

/// Rectangle relative to the body's top-left corner, with corner radius.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    #[serde(default)]
    pub radius: f64,
}

impl Rect {
    pub fn right(&self) -> f64 {
        self.x + self.w
    }

    pub fn bottom(&self) -> f64 {
        self.y + self.h
    }

    /// Is `self` inside `other` (with a small epsilon)?
    pub fn inside(&self, other: &Rect) -> bool {
        const EPS: f64 = 1e-9;
        self.x >= other.x - EPS
            && self.y >= other.y - EPS
            && self.right() <= other.right() + EPS
            && self.bottom() <= other.bottom() + EPS
    }

    pub fn intersects(&self, other: &Rect) -> bool {
        self.x < other.right()
            && other.x < self.right()
            && self.y < other.bottom()
            && other.y < self.bottom()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CutoutKind {
    /// Pill over the display (drawn over the screenshot).
    Island,
    /// Notch at the top display edge (drawn over the screenshot).
    Notch,
    /// Camera hole in the display (drawn over the screenshot).
    PunchHole,
    /// Home button in the bezel.
    HomeButton,
    /// Camera in the bezel.
    Camera,
    /// Speaker slot in the bezel.
    Speaker,
}

impl CutoutKind {
    /// Drawn over the screenshot (inside the display) rather than in the bezel.
    pub fn over_display(self) -> bool {
        matches!(self, Self::Island | Self::Notch | Self::PunchHole)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CutoutShape {
    Pill,
    Circle,
    /// Rectangle hanging from the top display edge, rounded bottom corners
    /// (`radius`) and concave shoulders.
    Notch,
    Rect,
}

/// A cutout or bezel feature (rect relative to the body).
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Cutout {
    pub kind: CutoutKind,
    pub shape: CutoutShape,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    #[serde(default)]
    pub radius: f64,
}

impl Cutout {
    pub fn rect(&self) -> Rect {
        Rect {
            x: self.x,
            y: self.y,
            w: self.w,
            h: self.h,
            radius: self.radius,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Edge {
    Left,
    Right,
    Top,
    Bottom,
}

/// How a button is drawn.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ButtonStyle {
    /// Flat key.
    #[default]
    Plain,
    /// Watch crown: a ribbed cylinder.
    Crown,
    /// Raised part of the housing (crown guard), body colour.
    Guard,
}

/// A hardware button on the body edge.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Button {
    pub name: String,
    #[serde(default)]
    pub style: ButtonStyle,
    pub edge: Edge,
    /// Start along the edge, from the top (left/right edges) or left
    /// (top/bottom edges) body corner.
    pub at: f64,
    pub length: f64,
    /// How far the button sticks out of the body.
    pub depth: f64,
}

/// Laptop lower half in front view: hinge and base, centred below the lid.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Laptop {
    pub hinge: Size,
    pub base: LaptopBase,
}

#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LaptopBase {
    pub w: f64,
    pub h: f64,
    #[serde(default)]
    pub radius: f64,
    /// Thumb notch at the front edge, centred.
    pub lip: Size,
}

/// Desktop stand in front view: neck centred below the body, foot below.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Stand {
    pub neck: Size,
    pub foot: StandFoot,
}

pub type StandFoot = Size;

/// Expected screenshot of a device.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Screenshot {
    /// Display aspect in the natural orientation; `None` = `free` (the
    /// frame adapts to the screenshot, e.g. browser, `none`).
    #[serde(deserialize_with = "free_or_aspect")]
    pub aspect: Option<Aspect>,
    /// Default screenshot variant key (`ios-phone`).
    #[serde(default)]
    pub variant: Option<String>,
    /// Typical raw screenshot sizes (natural orientation).
    #[serde(default)]
    pub sizes: Vec<PixelSize>,
    /// Target presets whose sizes match the display.
    #[serde(default)]
    pub targets: Vec<String>,
}

fn free_or_aspect<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<Aspect>, D::Error> {
    let s = String::deserialize(d)?;
    if s == "free" {
        Ok(None)
    } else {
        Aspect::parse(&s)
            .map(Some)
            .map_err(serde::de::Error::custom)
    }
}

/// A device model.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Device {
    pub id: String,
    pub label: String,
    pub class: DeviceClass,
    /// Model unit of the numbers (`mm`, `px`, `nominal`); only ratios matter.
    pub unit: String,
    /// Orientation the geometry is given in.
    pub natural: Orientation,
    pub orientations: Vec<Orientation>,
    pub body: Size,
    /// Metal rim thickness (`detailed` style).
    #[serde(default)]
    pub rim: f64,
    pub display: Rect,
    #[serde(default)]
    pub cutouts: Vec<Cutout>,
    #[serde(default)]
    pub buttons: Vec<Button>,
    #[serde(default)]
    pub laptop: Option<Laptop>,
    #[serde(default)]
    pub stand: Option<Stand>,
    /// The body is a browser window drawn with `browser_chrome`.
    #[serde(default)]
    pub browser: bool,
    /// Foldables: hinge band / crease.
    #[serde(default)]
    pub hinge: Option<Hinge>,
    /// Foldables: this model's fold state and the model of the other one
    /// (`fold:` on a device element switches between them).
    #[serde(default)]
    pub fold: Option<FoldLink>,
    #[serde(default)]
    pub colors: Vec<String>,
    #[serde(default)]
    pub default_color: Option<String>,
    pub screenshot: Screenshot,
}

impl Device {
    /// Body as a rect at the origin.
    pub fn body_rect(&self) -> Rect {
        Rect {
            x: 0.0,
            y: 0.0,
            w: self.body.w,
            h: self.body.h,
            radius: self.body.radius,
        }
    }

    /// Outer size in the natural orientation, including laptop base and
    /// desktop stand (buttons excluded). The element `width` maps to `w`.
    pub fn outer_size(&self) -> Size {
        let mut w = self.body.w;
        let mut h = self.body.h;
        if let Some(l) = &self.laptop {
            w = w.max(l.hinge.w).max(l.base.w);
            h += l.hinge.h + l.base.h;
        }
        if let Some(s) = &self.stand {
            w = w.max(s.neck.w).max(s.foot.w);
            h += s.neck.h + s.foot.h;
        }
        Size {
            w,
            h,
            radius: self.body.radius,
        }
    }

    /// Display `w / h` in the given orientation.
    pub fn display_ratio(&self, orientation: Orientation) -> f64 {
        oriented_ratio(self.display.w, self.display.h, self.natural, orientation)
    }

    /// Display size as fractions `(w, h)` of the outer size
    /// ([`Device::outer_size`]) in the given orientation.
    pub fn display_fraction(&self, orientation: Orientation) -> (f64, f64) {
        let o = self.outer_size();
        let (fw, fh) = (self.display.w / o.w, self.display.h / o.h);
        if orientation == self.natural {
            (fw, fh)
        } else {
            (fh, fw)
        }
    }

    /// A copy whose display has the aspect `ratio` (`w / h`, natural
    /// orientation): the display keeps its width, its height changes and
    /// the body grows or shrinks by the same amount (the chrome above a
    /// browser display stays). Meant for `aspect: free` models, which have
    /// no cutouts, buttons, laptop base or stand below the display; corner
    /// radii are clamped to the new size.
    #[must_use]
    pub fn with_display_aspect(&self, ratio: f64) -> Device {
        let mut d = self.clone();
        if !(ratio.is_finite() && ratio > 0.0) {
            return d;
        }
        let delta = self.display.w / ratio - self.display.h;
        d.display.h += delta;
        d.body.h += delta;
        d.display.radius = d.display.radius.min(d.display.w.min(d.display.h) / 2.0);
        d.body.radius = d.body.radius.min(d.body.w.min(d.body.h) / 2.0);
        d
    }

    /// Does a screenshot of `size` fit the display within 2 % (in the
    /// orientation the size implies)? Always true for `free` devices.
    pub fn matches_screenshot(&self, size: PixelSize) -> bool {
        if self.screenshot.aspect.is_none() {
            return true;
        }
        let o = if size.w > size.h {
            Orientation::Landscape
        } else {
            Orientation::Portrait
        };
        aspect_matches(f64::from(size.w) / f64::from(size.h), self.display_ratio(o))
    }

    pub fn supports(&self, orientation: Orientation) -> bool {
        self.orientations.contains(&orientation)
    }

    /// The model id to draw for the element setting `fold` (`None` = the
    /// model as it is): the linked model of the other state, if any.
    pub fn fold_model(&self, fold: Option<Fold>) -> Option<&str> {
        let link = self.fold.as_ref()?;
        (fold? != link.state).then_some(link.other.as_str())
    }

    /// Is `color` one of the model's colour variants?
    pub fn has_color(&self, color: &str) -> bool {
        self.colors.iter().any(|c| c == color)
    }

    /// Geometry of a device element `width` lu wide in `orientation`, as
    /// the parametric frames draw it: outer height and the screen rect
    /// relative to the element box's top-left corner. `None` for `aspect:
    /// free` models (browser, `none`: the height follows the screenshot).
    /// Imported vendor frames have their own proportions.
    pub fn frame_metrics(&self, orientation: Orientation, width: f64) -> Option<FrameMetrics> {
        if self.screenshot.aspect.is_none() || self.browser {
            return None;
        }
        let outer = self.outer_size();
        if outer.w <= 0.0 || outer.h <= 0.0 || width <= 0.0 {
            return None;
        }
        // Same mapping as `vitrine_render::frames::Layout` (rotated = 90°
        // counter-clockwise around the outer centre).
        let rot = orientation != self.natural && self.class != DeviceClass::None;
        let height = if rot {
            width * outer.w / outer.h
        } else {
            width * outer.h / outer.w
        };
        let k = width / if rot { outer.h } else { outer.w };
        let (cx, cy) = (self.body.w / 2.0, outer.h / 2.0);
        let disp = &self.display;
        let screen = if rot {
            // (x, y) → (k·(y − cy), k·(cx − x))
            Rect {
                x: k * (disp.y - cy) + width / 2.0,
                y: k * (cx - disp.right()) + height / 2.0,
                w: disp.h * k,
                h: disp.w * k,
                radius: disp.radius * k,
            }
        } else {
            Rect {
                x: k * (disp.x - cx) + width / 2.0,
                y: k * (disp.y - cy) + height / 2.0,
                w: disp.w * k,
                h: disp.h * k,
                radius: disp.radius * k,
            }
        };
        Some(FrameMetrics {
            width,
            height,
            screen,
            corner_radius: self.body.radius * k,
        })
    }
}

/// Parametric frame geometry of one device element (lu), see
/// [`Device::frame_metrics`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FrameMetrics {
    pub width: f64,
    pub height: f64,
    /// Screen rect relative to the top-left corner of the element box
    /// (before rotation), with its corner radius.
    pub screen: Rect,
    /// Corner radius of the body.
    pub corner_radius: f64,
}

pub(super) fn check(c: &Catalog, errors: &mut Vec<String>) {
    let file = &c.devices;
    for name in file.colors.keys() {
        if !is_valid_id(name) {
            errors.push(format!("devices: invalid colour name `{name}`"));
        }
    }
    let chrome = &file.browser_chrome;
    for theme in ["light", "dark"] {
        if !chrome.themes.contains_key(theme) {
            errors.push(format!("devices: browser_chrome needs a `{theme}` theme"));
        }
    }
    let mut seen = std::collections::BTreeSet::new();
    for d in &file.devices {
        if !is_valid_id(&d.id) {
            errors.push(format!("devices: invalid id `{}`", d.id));
        }
        if !seen.insert(d.id.as_str()) {
            errors.push(format!("devices: duplicate device id `{}`", d.id));
        }
        check_geometry(d, errors);
        check_class_parts(d, chrome, errors);
        check_fold(d, c, errors);
        check_colors(d, file, errors);
        check_screenshot(d, c, errors);
    }
}

fn check_geometry(d: &Device, errors: &mut Vec<String>) {
    let id = &d.id;
    let body = d.body_rect();
    let disp = &d.display;
    if !(body.w > 0.0 && body.h > 0.0 && disp.w > 0.0 && disp.h > 0.0) {
        errors.push(format!("{id}: body and display must have a positive size"));
    }
    if !disp.inside(&body) {
        errors.push(format!("{id}: display is not inside the body"));
    }
    for (name, r, w, h) in [
        ("body", body.radius, body.w, body.h),
        ("display", disp.radius, disp.w, disp.h),
    ] {
        if r < 0.0 || r > w.min(h) / 2.0 {
            errors.push(format!("{id}: {name} radius out of range"));
        }
    }
    if d.rim < 0.0 || d.rim > disp.x.min(disp.y) {
        errors.push(format!("{id}: rim must fit between body and display"));
    }
    for cut in &d.cutouts {
        let r = cut.rect();
        if r.w <= 0.0 || r.h <= 0.0 {
            errors.push(format!("{id}: {:?} has no size", cut.kind));
        } else if cut.kind.over_display() {
            if !r.inside(disp) {
                errors.push(format!("{id}: {:?} must lie inside the display", cut.kind));
            }
        } else if !r.inside(&body) || r.intersects(disp) {
            errors.push(format!(
                "{id}: {:?} must lie in the bezel (inside the body, outside the display)",
                cut.kind
            ));
        }
        if cut.shape == CutoutShape::Circle && (r.w - r.h).abs() > 1e-9 {
            errors.push(format!("{id}: circular {:?} must be square", cut.kind));
        }
        if cut.kind == CutoutKind::Notch && (r.y - disp.y).abs() > 1e-9 {
            errors.push(format!("{id}: notch must hang from the display top edge"));
        }
    }
    for b in &d.buttons {
        let edge_len = match b.edge {
            Edge::Left | Edge::Right => body.h,
            Edge::Top | Edge::Bottom => body.w,
        };
        if b.at < 0.0 || b.length <= 0.0 || b.at + b.length > edge_len || b.depth <= 0.0 {
            errors.push(format!("{id}: button `{}` does not fit its edge", b.name));
        }
    }
    if !d.supports(d.natural) {
        errors.push(format!("{id}: natural orientation must be supported"));
    }
}

fn check_class_parts(d: &Device, chrome: &BrowserChrome, errors: &mut Vec<String>) {
    let id = &d.id;
    let class = d.class;
    if d.laptop.is_some() != (class == DeviceClass::Laptop) {
        errors.push(format!(
            "{id}: `laptop` geometry is required for (only) laptops"
        ));
    }
    if d.stand.is_some() != (class == DeviceClass::Desktop) {
        errors.push(format!(
            "{id}: `stand` geometry is required for (only) desktops"
        ));
    }
    if d.browser != (class == DeviceClass::Browser) {
        errors.push(format!(
            "{id}: `browser: true` is required for (only) the browser class"
        ));
    }
    if let Some(l) = &d.laptop
        && (l.base.w < d.body.w || l.base.lip.w > l.base.w || l.base.lip.h > l.base.h)
    {
        errors.push(format!(
            "{id}: laptop base must be at least as wide as the lid"
        ));
    }
    if d.browser {
        let expected = chrome.height() * d.body.w / chrome.reference_width;
        if (d.display.y - expected).abs() > 1e-6 || (d.display.bottom() - d.body.h).abs() > 1e-6 {
            errors.push(format!(
                "{id}: browser content must start below the chrome ({expected}) and fill the rest"
            ));
        }
    }
}

fn check_fold(d: &Device, c: &Catalog, errors: &mut Vec<String>) {
    let id = &d.id;
    if let Some(h) = &d.hinge {
        let max = match h.at {
            HingeAt::Center => d.display.w / 4.0,
            HingeAt::Left => d.display.x,
        };
        if h.w <= 0.0 || h.w > max {
            errors.push(format!("{id}: hinge width out of range"));
        }
    }
    let Some(link) = &d.fold else { return };
    match c.device(&link.other) {
        None => errors.push(format!("{id}: unknown fold model `{}`", link.other)),
        Some(o) => {
            let back = o.fold.as_ref();
            if back.is_none_or(|b| b.other != *id || b.state == link.state) {
                errors.push(format!(
                    "{id}: fold model `{}` must link back with the other state",
                    link.other
                ));
            }
            if o.colors != d.colors || o.default_color != d.default_color {
                errors.push(format!(
                    "{id}: fold model `{}` must have the same colours",
                    link.other
                ));
            }
        }
    }
}

fn check_colors(d: &Device, file: &DevicesFile, errors: &mut Vec<String>) {
    let id = &d.id;
    for name in &d.colors {
        if !file.colors.contains_key(name) {
            errors.push(format!("{id}: unknown colour `{name}`"));
        }
    }
    match &d.default_color {
        Some(c) if !d.has_color(c) => {
            errors.push(format!("{id}: default colour `{c}` is not in `colors`"));
        }
        None if !d.colors.is_empty() => {
            errors.push(format!("{id}: `default_color` is missing"));
        }
        _ => {}
    }
}

fn check_screenshot(d: &Device, c: &Catalog, errors: &mut Vec<String>) {
    let id = &d.id;
    let shot = &d.screenshot;
    let Some(aspect) = shot.aspect else {
        if !shot.sizes.is_empty() || !shot.targets.is_empty() {
            errors.push(format!(
                "{id}: free screenshot aspect takes no sizes/targets"
            ));
        }
        return;
    };
    let natural = d.display.w / d.display.h;
    let want = f64::from(aspect.w) / f64::from(aspect.h);
    if !aspect_matches(natural, want) {
        errors.push(format!(
            "{id}: display ratio {natural:.4} does not match screenshot aspect {aspect} ({want:.4})"
        ));
    }
    for s in &shot.sizes {
        if !aspect_matches(f64::from(s.w) / f64::from(s.h), want) {
            errors.push(format!(
                "{id}: screenshot size {}x{} does not match {aspect}",
                s.w, s.h
            ));
        }
    }
    if shot.variant.is_none() {
        errors.push(format!("{id}: screenshot variant is missing"));
    }
    for t in &shot.targets {
        let Some(p) = c.target_preset(t) else {
            errors.push(format!("{id}: unknown target preset `{t}`"));
            continue;
        };
        for s in p.sizes.all() {
            let o = if s.w > s.h {
                Orientation::Landscape
            } else {
                Orientation::Portrait
            };
            if d.supports(o) && !d.matches_screenshot(s) {
                errors.push(format!(
                    "{id}: target `{t}` size {}x{} differs from the display by more than 2 %",
                    s.w, s.h
                ));
            }
        }
    }
}
