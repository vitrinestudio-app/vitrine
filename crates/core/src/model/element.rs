//! Elements (§3.5): common properties plus kind-specific properties.
//!
//! In YAML an element is one flat map tagged by `kind`. Internally it is a
//! typed [`Element`] with an [`ElementKind`] enum. The conversion goes
//! through [`ElementWire`], a flat struct with every possible field, which
//! gives us `deny_unknown_fields` (typos are errors), a stable field order
//! and good error messages for fields that do not belong to a kind
//! (serde's `flatten` would disable unknown-field detection).
//!
//! [`ElementPatch`] has the same fields as [`ElementWire`] (minus `id`,
//! `kind`, `children`), each as `Option<Option<T>>`: missing = unchanged,
//! `null` = reset to default, value = set. It is used both for variant
//! overrides and for `Op::UpdateElement`.

// Patch fields are `Option<Option<T>>` on purpose: unchanged / reset / set.
#![allow(clippy::option_option)]

use serde::{Deserialize, Serialize};

use super::color::{ColorRef, Rgba};
use super::common::{AnchorY, Clip, FitMode, Point, VisibleIn};
use super::fill::Fill;
use super::fmt::{double_option, flow_style, num};
use super::ids::{ElementId, StringKey, is_valid_id};
use super::style::FrameStyle;

/// An element on a strip.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "ElementWire", into = "ElementWire")]
pub struct Element {
    /// Unique across the project.
    pub id: ElementId,
    /// Home page (0-based index). Always written to the file.
    pub page: u32,
    /// Position relative to the home page's top-left corner, in lu. For
    /// devices, images, shapes and free text boxes this is the centre of the
    /// element. Default `(0, 0)`.
    pub at: Point,
    pub width: Option<f64>,
    pub height: Option<f64>,
    /// Degrees, positive = clockwise.
    pub rotation: f64,
    /// Rotation pivot as offset from the element centre in lu (default centre).
    pub pivot: Option<Point>,
    /// `None` = kind default (`stretch` for backgrounds, `top` otherwise).
    pub anchor_y: Option<AnchorY>,
    /// `0..=1`, default 1.
    pub opacity: f64,
    /// Layer; painting order is `(z, list order)`. Default 0.
    pub z: i32,
    /// `None` = kind default (`page` for text, `none` otherwise).
    pub clip: Option<Clip>,
    /// `false` hides the element (useful in variant overrides).
    pub visible: bool,
    pub visible_in: Option<VisibleIn>,
    pub kind: ElementKind,
}

/// Kind-specific properties.
#[derive(Clone, Debug, PartialEq)]
pub enum ElementKind {
    Background(BackgroundProps),
    Device(DeviceProps),
    Text(TextProps),
    Image(ImageProps),
    Shape(ShapeProps),
    Group(GroupProps),
}

/// Background layer on its home page (canvas-wide backgrounds live in
/// `Strip::backgrounds`).
#[derive(Clone, Debug, PartialEq)]
pub struct BackgroundProps {
    pub fill: Fill,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DeviceProps {
    /// Device model id from the catalog (`iphone-island` …).
    pub model: String,
    pub orientation: Option<Orientation>,
    /// Overrides the project's frame style.
    pub frame: Option<FrameStyle>,
    /// Colour variant of the catalog model (`midnight`) or a custom `#RRGGBB`.
    pub color: Option<String>,
    /// Browser chrome inside a laptop/desktop frame (`chrome: browser`).
    pub chrome: Option<Chrome>,
    /// URL shown in browser chrome.
    pub url: Option<String>,
    pub appearance: Option<Appearance>,
    /// Screenshot asset key (`home` → `screenshots/<lang>/<variant>/home.png`).
    pub screenshot: Option<String>,
    /// Screenshot fit, default `cover`.
    pub fit: Option<FitMode>,
    pub shadow: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TextProps {
    /// Translation key.
    pub text: StringKey,
    /// Text style name.
    pub style: Option<String>,
    /// Box from page margins. Without `box`, the text is a free box centred
    /// at `at` with `width` (and optional `height`).
    pub r#box: Option<TextBox>,
    pub align: Option<Align>,
    pub valign: Option<VAlign>,
    pub fit: Option<TextFit>,
    /// Overrides the style colour.
    pub color: Option<ColorRef>,
    /// Overrides the style size, lu.
    pub font_size: Option<f64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ImageProps {
    /// Asset path relative to the project folder.
    pub asset: String,
    pub fit: Option<FitMode>,
    pub shadow: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ShapeProps {
    pub shape: ShapeKind,
    /// Corner radius for rectangles, lu.
    pub radius: Option<f64>,
    pub fill: Option<Fill>,
    pub stroke: Option<Stroke>,
    pub shadow: Option<String>,
}

/// Groups elements for moving/rotating them together. Children keep their
/// own home page and position; the group's `at` is added as offset and its
/// rotation/opacity apply to all children (resolve, task 1.4).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct GroupProps {
    pub children: Vec<Element>,
}

/// `kind:` tag of an element.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ElementKindTag {
    Background,
    Device,
    Text,
    Image,
    Shape,
    Group,
}

impl ElementKindTag {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Background => "background",
            Self::Device => "device",
            Self::Text => "text",
            Self::Image => "image",
            Self::Shape => "shape",
            Self::Group => "group",
        }
    }

    /// Kind-specific fields allowed for this kind (besides the common ones).
    fn fields(self) -> &'static [&'static str] {
        match self {
            Self::Background => &["fill"],
            Self::Device => &[
                "model",
                "orientation",
                "frame",
                "color",
                "chrome",
                "url",
                "appearance",
                "screenshot",
                "fit",
                "shadow",
            ],
            Self::Text => &[
                "text",
                "style",
                "box",
                "align",
                "valign",
                "fit",
                "color",
                "font_size",
            ],
            Self::Image => &["asset", "fit", "shadow"],
            Self::Shape => &["shape", "radius", "fill", "stroke", "shadow"],
            Self::Group => &["children"],
        }
    }
}

/// Fields every element kind has.
const COMMON_FIELDS: [&str; 12] = [
    "page",
    "at",
    "width",
    "height",
    "rotation",
    "pivot",
    "anchor_y",
    "opacity",
    "z",
    "clip",
    "visible",
    "visible_in",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Orientation {
    Portrait,
    Landscape,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Chrome {
    None,
    Browser,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Appearance {
    Light,
    Dark,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Align {
    Left,
    Center,
    Right,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VAlign {
    Top,
    Center,
    Bottom,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShapeKind {
    Rect,
    Ellipse,
    Line,
}

/// Text box from page margins in lu (`box: { top: 140, left: 80, right: 80 }`).
/// Missing `bottom` = the box grows downwards as needed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(remote = "Self", deny_unknown_fields)]
pub struct TextBox {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        serialize_with = "num"
    )]
    pub top: Option<f64>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        serialize_with = "num"
    )]
    pub left: Option<f64>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        serialize_with = "num"
    )]
    pub right: Option<f64>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        serialize_with = "num"
    )]
    pub bottom: Option<f64>,
}
flow_style!(TextBox);

/// Text auto-fit: shrink down to `shrink_to` (lu), at most `max_lines`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(remote = "Self", deny_unknown_fields)]
pub struct TextFit {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        serialize_with = "num"
    )]
    pub shrink_to: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_lines: Option<u32>,
}
flow_style!(TextFit);

/// Shape outline.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(remote = "Self", deny_unknown_fields)]
pub struct Stroke {
    pub color: ColorRef,
    #[serde(serialize_with = "num")]
    pub width: f64,
}
flow_style!(Stroke);

/// Value of the `fit` field: a fit mode for devices/images
/// (`fit: contain`), auto-fit settings for text (`fit: { shrink_to: 52 }`).
#[derive(Clone, Debug, PartialEq)]
pub enum FitValue {
    Mode(FitMode),
    Text(TextFit),
}

impl Serialize for FitValue {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Mode(m) => m.serialize(s),
            Self::Text(t) => t.serialize(s),
        }
    }
}

impl<'de> Deserialize<'de> for FitValue {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> serde::de::Visitor<'de> for V {
            type Value = FitValue;
            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("`cover`, `contain`, `stretch` or `{ shrink_to, max_lines }`")
            }
            fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<FitValue, E> {
                FitMode::deserialize(serde::de::value::StrDeserializer::<E>::new(v))
                    .map(FitValue::Mode)
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(self, map: A) -> Result<FitValue, A::Error> {
                TextFit::deserialize(serde::de::value::MapAccessDeserializer::new(map))
                    .map(FitValue::Text)
            }
        }
        d.deserialize_any(V)
    }
}

/// Defines [`ElementWire`] and [`ElementPatch`] from one field list so
/// both always stay in sync. Order = order in the YAML file.
macro_rules! element_fields {
    ($( $(#[$attr:meta])* $field:ident : $ty:ty ),* $(,)?) => {
        /// Flat on-disk form of an [`Element`]. See module docs.
        #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
        #[serde(deny_unknown_fields)]
        pub struct ElementWire {
            pub id: ElementId,
            pub kind: ElementKindTag,
            $(
                $(#[$attr])*
                #[serde(default, skip_serializing_if = "Option::is_none")]
                pub $field: Option<$ty>,
            )*
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub children: Option<Vec<Element>>,
        }

        /// Partial update of an element; see module docs.
        #[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
        #[serde(remote = "Self", deny_unknown_fields)]
        pub struct ElementPatch {
            $(
                $(#[$attr])*
                #[serde(default, deserialize_with = "double_option", skip_serializing_if = "Option::is_none")]
                pub $field: Option<Option<$ty>>,
            )*
        }

        impl ElementWire {
            /// Names of the fields that are set.
            fn present_fields(&self) -> Vec<&'static str> {
                let mut out = Vec::new();
                $( if self.$field.is_some() { out.push(field_name(stringify!($field))); } )*
                if self.children.is_some() { out.push("children"); }
                out
            }
        }

        impl ElementPatch {
            /// Apply to a wire element; returns the patch that undoes it.
            fn apply_to_wire(&self, w: &mut ElementWire) -> ElementPatch {
                let mut inv = ElementPatch::default();
                $(
                    if let Some(v) = &self.$field {
                        inv.$field = Some(std::mem::replace(&mut w.$field, v.clone()));
                    }
                )*
                inv
            }

            /// Does the patch change nothing?
            pub fn is_empty(&self) -> bool {
                $( self.$field.is_none() && )* true
            }

            /// Names of the fields the patch touches.
            pub fn fields(&self) -> Vec<&'static str> {
                let mut out = Vec::new();
                $( if self.$field.is_some() { out.push(field_name(stringify!($field))); } )*
                out
            }

            /// Merge `other` on top of `self` (fields of `other` win).
            #[must_use]
            pub fn merged(&self, other: &ElementPatch) -> ElementPatch {
                ElementPatch {
                    $( $field: other.$field.clone().or_else(|| self.$field.clone()), )*
                }
            }
        }
    };
}

fn field_name(s: &'static str) -> &'static str {
    s.strip_prefix("r#").unwrap_or(s)
}

element_fields! {
    page: u32,
    at: Point,
    #[serde(serialize_with = "num")]
    width: f64,
    #[serde(serialize_with = "num")]
    height: f64,
    #[serde(serialize_with = "num")]
    rotation: f64,
    pivot: Point,
    anchor_y: AnchorY,
    // device
    model: String,
    orientation: Orientation,
    frame: FrameStyle,
    color: String,
    chrome: Chrome,
    url: String,
    appearance: Appearance,
    screenshot: String,
    // text
    text: StringKey,
    style: String,
    r#box: TextBox,
    align: Align,
    valign: VAlign,
    #[serde(serialize_with = "num")]
    font_size: f64,
    // image
    asset: String,
    // shape
    shape: ShapeKind,
    #[serde(serialize_with = "num")]
    radius: f64,
    stroke: Stroke,
    // shared kind-specific
    fill: Fill,
    fit: FitValue,
    shadow: String,
    // common tail
    #[serde(serialize_with = "num")]
    opacity: f64,
    z: i32,
    clip: Clip,
    visible: bool,
    visible_in: VisibleIn,
}
flow_style!(ElementPatch);

fn check_name(field: &str, v: &str) -> Result<(), String> {
    if is_valid_id(v) {
        Ok(())
    } else {
        Err(format!("invalid {field} `{v}`"))
    }
}

fn check_device_color(v: &str) -> Result<(), String> {
    if v.starts_with('#') {
        Rgba::parse(v).map(|_| ())
    } else {
        check_name("device colour", v)
    }
}

impl TryFrom<ElementWire> for Element {
    type Error = String;

    #[allow(clippy::too_many_lines)]
    fn try_from(w: ElementWire) -> Result<Self, String> {
        let kind = w.kind;
        for f in w.present_fields() {
            if !COMMON_FIELDS.contains(&f) && !kind.fields().contains(&f) {
                return Err(format!(
                    "element `{}`: field `{f}` is not allowed on {} elements",
                    w.id,
                    kind.as_str()
                ));
            }
        }
        let missing =
            |f: &str| format!("element `{}`: {} element needs `{f}`", w.id, kind.as_str());
        let fit_mode = |fit: Option<FitValue>| match fit {
            None => Ok(None),
            Some(FitValue::Mode(m)) => Ok(Some(m)),
            Some(FitValue::Text(_)) => Err(format!(
                "element `{}`: `fit` must be `cover`, `contain` or `stretch` on {} elements",
                w.id,
                kind.as_str()
            )),
        };
        if let Some(s) = &w.shadow {
            check_name("shadow preset", s)?;
        }
        if let Some(o) = w.opacity
            && !(0.0..=1.0).contains(&o)
        {
            return Err(format!("element `{}`: opacity must be within 0..1", w.id));
        }
        let props = match kind {
            ElementKindTag::Background => ElementKind::Background(BackgroundProps {
                fill: w.fill.clone().ok_or_else(|| missing("fill"))?,
            }),
            ElementKindTag::Device => {
                if w.height.is_some() {
                    return Err(format!(
                        "element `{}`: a device's height follows from its model; set `width` only",
                        w.id
                    ));
                }
                let model = w.model.clone().ok_or_else(|| missing("model"))?;
                check_name("device model", &model)?;
                if let Some(c) = &w.color {
                    check_device_color(c)?;
                }
                ElementKind::Device(DeviceProps {
                    model,
                    orientation: w.orientation,
                    frame: w.frame.clone(),
                    color: w.color.clone(),
                    chrome: w.chrome,
                    url: w.url.clone(),
                    appearance: w.appearance,
                    screenshot: w.screenshot.clone(),
                    fit: fit_mode(w.fit.clone())?,
                    shadow: w.shadow.clone(),
                })
            }
            ElementKindTag::Text => {
                let fit = match w.fit.clone() {
                    None => None,
                    Some(FitValue::Text(t)) => Some(t),
                    Some(FitValue::Mode(_)) => {
                        return Err(format!(
                            "element `{}`: `fit` on text elements is `{{ shrink_to, max_lines }}`",
                            w.id
                        ));
                    }
                };
                if let Some(s) = &w.style {
                    check_name("text style", s)?;
                }
                ElementKind::Text(TextProps {
                    text: w.text.clone().ok_or_else(|| missing("text"))?,
                    style: w.style.clone(),
                    r#box: w.r#box,
                    align: w.align,
                    valign: w.valign,
                    fit,
                    color: w
                        .color
                        .as_deref()
                        .map(ColorRef::parse)
                        .transpose()
                        .map_err(|e| format!("element `{}`: {e}", w.id))?,
                    font_size: w.font_size,
                })
            }
            ElementKindTag::Image => ElementKind::Image(ImageProps {
                asset: w.asset.clone().ok_or_else(|| missing("asset"))?,
                fit: fit_mode(w.fit.clone())?,
                shadow: w.shadow.clone(),
            }),
            ElementKindTag::Shape => ElementKind::Shape(ShapeProps {
                shape: w.shape.ok_or_else(|| missing("shape"))?,
                radius: w.radius,
                fill: w.fill.clone(),
                stroke: w.stroke.clone(),
                shadow: w.shadow.clone(),
            }),
            ElementKindTag::Group => ElementKind::Group(GroupProps {
                children: w.children.clone().unwrap_or_default(),
            }),
        };
        Ok(Element {
            id: w.id,
            page: w.page.unwrap_or(0),
            at: w.at.unwrap_or_default(),
            width: w.width,
            height: w.height,
            rotation: w.rotation.unwrap_or(0.0),
            pivot: w.pivot,
            anchor_y: w.anchor_y,
            opacity: w.opacity.unwrap_or(1.0),
            z: w.z.unwrap_or(0),
            clip: w.clip,
            visible: w.visible.unwrap_or(true),
            visible_in: w
                .visible_in
                .filter(|v| !v.targets.is_empty() || !v.langs.is_empty()),
            kind: props,
        })
    }
}

impl From<Element> for ElementWire {
    fn from(e: Element) -> Self {
        let mut w = ElementWire {
            id: e.id,
            kind: ElementKindTag::Background,
            page: Some(e.page),
            at: (!e.at.is_zero()).then_some(e.at),
            width: e.width,
            height: e.height,
            rotation: (e.rotation != 0.0).then_some(e.rotation),
            pivot: e.pivot,
            anchor_y: e.anchor_y,
            model: None,
            orientation: None,
            frame: None,
            color: None,
            chrome: None,
            url: None,
            appearance: None,
            screenshot: None,
            text: None,
            style: None,
            r#box: None,
            align: None,
            valign: None,
            font_size: None,
            asset: None,
            shape: None,
            radius: None,
            stroke: None,
            fill: None,
            fit: None,
            shadow: None,
            opacity: ((e.opacity - 1.0).abs() > f64::EPSILON).then_some(e.opacity),
            z: (e.z != 0).then_some(e.z),
            clip: e.clip,
            visible: (!e.visible).then_some(false),
            visible_in: e.visible_in,
            children: None,
        };
        match e.kind {
            ElementKind::Background(p) => {
                w.kind = ElementKindTag::Background;
                w.fill = Some(p.fill);
            }
            ElementKind::Device(p) => {
                w.kind = ElementKindTag::Device;
                w.model = Some(p.model);
                w.orientation = p.orientation;
                w.frame = p.frame;
                w.color = p.color;
                w.chrome = p.chrome;
                w.url = p.url;
                w.appearance = p.appearance;
                w.screenshot = p.screenshot;
                w.fit = p.fit.map(FitValue::Mode);
                w.shadow = p.shadow;
            }
            ElementKind::Text(p) => {
                w.kind = ElementKindTag::Text;
                w.text = Some(p.text);
                w.style = p.style;
                w.r#box = p.r#box;
                w.align = p.align;
                w.valign = p.valign;
                w.fit = p.fit.map(FitValue::Text);
                w.color = p.color.map(|c| c.to_string());
                w.font_size = p.font_size;
            }
            ElementKind::Image(p) => {
                w.kind = ElementKindTag::Image;
                w.asset = Some(p.asset);
                w.fit = p.fit.map(FitValue::Mode);
                w.shadow = p.shadow;
            }
            ElementKind::Shape(p) => {
                w.kind = ElementKindTag::Shape;
                w.shape = Some(p.shape);
                w.radius = p.radius;
                w.fill = p.fill;
                w.stroke = p.stroke;
                w.shadow = p.shadow;
            }
            ElementKind::Group(p) => {
                w.kind = ElementKindTag::Group;
                w.children = (!p.children.is_empty()).then_some(p.children);
            }
        }
        w
    }
}

impl Element {
    /// The `kind:` tag.
    pub fn kind_tag(&self) -> ElementKindTag {
        match &self.kind {
            ElementKind::Background(_) => ElementKindTag::Background,
            ElementKind::Device(_) => ElementKindTag::Device,
            ElementKind::Text(_) => ElementKindTag::Text,
            ElementKind::Image(_) => ElementKindTag::Image,
            ElementKind::Shape(_) => ElementKindTag::Shape,
            ElementKind::Group(_) => ElementKindTag::Group,
        }
    }

    /// Anchor with the kind default applied.
    pub fn effective_anchor_y(&self) -> AnchorY {
        self.anchor_y.unwrap_or(match self.kind {
            ElementKind::Background(_) => AnchorY::Stretch,
            _ => AnchorY::Top,
        })
    }

    /// Clip with the kind default applied (`page` for text, `none` otherwise).
    pub fn effective_clip(&self) -> Clip {
        self.clip.unwrap_or(match self.kind {
            ElementKind::Text(_) => Clip::Page,
            _ => Clip::None,
        })
    }

    /// Apply a patch and return the patched element plus the inverse patch.
    /// Fails if the result is not a valid element (e.g. `model` on a text).
    pub fn patched(&self, patch: &ElementPatch) -> Result<(Element, ElementPatch), String> {
        let mut w = ElementWire::from(self.clone());
        let inverse = patch.apply_to_wire(&mut w);
        Ok((Element::try_from(w)?, inverse))
    }

    /// Direct children (groups only).
    pub fn children(&self) -> &[Element] {
        match &self.kind {
            ElementKind::Group(g) => &g.children,
            _ => &[],
        }
    }

    pub fn children_mut(&mut self) -> Option<&mut Vec<Element>> {
        match &mut self.kind {
            ElementKind::Group(g) => Some(&mut g.children),
            _ => None,
        }
    }

    /// Visit this element and all descendants (pre-order).
    pub fn walk<'a>(&'a self, f: &mut impl FnMut(&'a Element)) {
        f(self);
        for c in self.children() {
            c.walk(f);
        }
    }

    /// Colour references used by this element (not its children).
    pub fn color_refs(&self) -> Vec<&ColorRef> {
        match &self.kind {
            ElementKind::Background(p) => p.fill.colors(),
            ElementKind::Text(p) => p.color.iter().collect(),
            ElementKind::Shape(p) => {
                let mut v: Vec<&ColorRef> = p.fill.iter().flat_map(Fill::colors).collect();
                v.extend(p.stroke.as_ref().map(|s| &s.color));
                v
            }
            ElementKind::Device(_) | ElementKind::Image(_) | ElementKind::Group(_) => Vec::new(),
        }
    }

    /// Shadow preset used by this element.
    pub fn shadow(&self) -> Option<&str> {
        match &self.kind {
            ElementKind::Device(p) => p.shadow.as_deref(),
            ElementKind::Image(p) => p.shadow.as_deref(),
            ElementKind::Shape(p) => p.shadow.as_deref(),
            _ => None,
        }
    }

    /// Text style used by this element.
    pub fn text_style(&self) -> Option<&str> {
        match &self.kind {
            ElementKind::Text(p) => p.style.as_deref(),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(y: &str) -> Result<Element, String> {
        crate::io::from_yaml::<Element>(y)
    }

    #[test]
    fn device_roundtrip() {
        let e = parse(
            "id: hero-phone\nkind: device\nmodel: iphone-island\nscreenshot: home\npage: 0\nat: { x: 1000, y: 2172 }\nanchor_y: bottom\nwidth: 820\nrotation: -8\nshadow: soft\n",
        )
        .unwrap();
        assert_eq!(e.page, 0);
        assert!((e.rotation + 8.0).abs() < f64::EPSILON);
        assert_eq!(e.effective_anchor_y(), AnchorY::Bottom);
        assert_eq!(e.effective_clip(), Clip::None);
        let ElementKind::Device(d) = &e.kind else {
            panic!()
        };
        assert_eq!(d.model, "iphone-island");
    }

    #[test]
    fn wrong_field_for_kind() {
        let err = parse("id: t\nkind: text\ntext: a.b\nmodel: x\n").unwrap_err();
        assert!(
            err.contains("field `model` is not allowed on text elements"),
            "{err}"
        );
        let err = parse("id: t\nkind: text\ntext: a.b\nfit: cover\n").unwrap_err();
        assert!(err.contains("shrink_to"), "{err}");
        let err = parse("id: d\nkind: device\n").unwrap_err();
        assert!(err.contains("needs `model`"), "{err}");
        let err = parse("id: d\nkind: device\nmodel: x\nmdoel: y\n").unwrap_err();
        assert!(err.contains("unknown field `mdoel`"), "{err}");
    }

    #[test]
    fn text_defaults() {
        let e =
            parse("id: t\nkind: text\ntext: hero.title\nfit: { shrink_to: 52, max_lines: 3 }\n")
                .unwrap();
        assert_eq!(e.effective_clip(), Clip::Page);
        assert_eq!(e.effective_anchor_y(), AnchorY::Top);
    }

    #[test]
    fn patch_and_inverse() {
        let e = parse("id: t\nkind: text\ntext: hero.title\npage: 1\n").unwrap();
        let patch: ElementPatch =
            crate::io::from_yaml("{ at: { x: 10, y: 20 }, rotation: 5, page: ~ }").unwrap();
        assert_eq!(patch.fields(), vec!["page", "at", "rotation"]);
        let (p, inv) = e.patched(&patch).unwrap();
        assert_eq!(p.page, 0);
        assert_eq!(p.at, Point::new(10.0, 20.0));
        let (back, _) = p.patched(&inv).unwrap();
        assert_eq!(back, e);
        let bad: ElementPatch = crate::io::from_yaml("{ model: x }").unwrap();
        assert!(e.patched(&bad).is_err());
    }
}
