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

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::color::{ColorRef, Rgba};
use super::common::{AnchorY, Clip, FitMode, Point, VisibleIn};
use super::fill::Fill;
use super::fmt::{double_option, flow_style, num};
use super::ids::{ElementId, Lang, StringKey, is_valid_id};
use super::style::{FrameStyle, Glow};

/// An element on a canvas.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "ElementWire", into = "ElementWire")]
pub struct Element {
    /// Unique across the project.
    pub id: ElementId,
    /// Home page (0-based index). Always written to the file.
    pub page: u32,
    /// Keep the home page when the element is dragged to another page
    /// (edit intents default `keep_page` from it). Default `false`.
    pub lock_page: bool,
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
/// `Canvas::backgrounds`).
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
    /// Imported frames: the vendor model whose bezel to use (`iPhone 18
    /// Pro`); `None` = automatic per target (ADR 0012).
    pub bezel: Option<String>,
    /// Colour variant of the catalog model (`midnight`) or a custom `#RRGGBB`.
    pub color: Option<String>,
    /// Foldables: open (inner display) or folded (outer display); `None` =
    /// the state of `model`.
    pub fold: Option<Fold>,
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
    /// Overrides the style outline.
    pub stroke: Option<Stroke>,
    /// Overrides the style glow.
    pub glow: Option<Glow>,
    /// Overrides the style shadow preset.
    pub shadow: Option<String>,
    /// Automatic hyphenation (overrides the style), used only when the text
    /// does not fit at `fit.shrink_to` (ADR 0009).
    pub hyphenate: Option<bool>,
    /// Per-language overrides (KONZEPT §5.1): language → patch with only
    /// [`LANG_OVERRIDE_FIELDS`]. Applied after variant overrides; for
    /// `de-AT` the `de` override applies first, then the `de-AT` one.
    pub langs: BTreeMap<Lang, ElementPatch>,
}

/// Fields a per-language override of a text element may set: typography,
/// box, fit and the size/position of a free box. Not the home page,
/// rotation, pivot, layer, clip, opacity or visibility: those are layout
/// decisions shared by all languages (`visible_in` covers languages).
pub const LANG_OVERRIDE_FIELDS: [&str; 15] = [
    "at",
    "width",
    "height",
    "anchor_y",
    "style",
    "box",
    "align",
    "valign",
    "fit",
    "color",
    "font_size",
    "stroke",
    "glow",
    "shadow",
    "hyphenate",
];

impl TextProps {
    /// The language overrides that apply to `lang`, most general first:
    /// those of its prefixes and its own (`de`, `de-AT` for `de-AT`).
    pub fn lang_overrides_for(&self, lang: &str) -> Vec<(&Lang, &ElementPatch)> {
        let mut out = Vec::new();
        let mut l = lang;
        loop {
            if let Some(kv) = self.langs.get_key_value(l) {
                out.push(kv);
            }
            match l.rfind('-') {
                Some(i) => l = &l[..i],
                None => break,
            }
        }
        out.reverse();
        out
    }

    /// The combined language override for `lang` (prefix overrides merged,
    /// the most specific wins per field) and its most specific key.
    pub fn lang_override(&self, lang: &str) -> Option<(&Lang, ElementPatch)> {
        let list = self.lang_overrides_for(lang);
        let key = list.last()?.0;
        let patch = list
            .iter()
            .fold(ElementPatch::default(), |acc, (_, p)| acc.merged(p));
        Some((key, patch))
    }
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
    pub fn fields(self) -> &'static [&'static str] {
        match self {
            Self::Background => &["fill"],
            Self::Device => &[
                "model",
                "orientation",
                "frame",
                "bezel",
                "color",
                "fold",
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
                "stroke",
                "glow",
                "shadow",
                "hyphenate",
                "langs",
            ],
            Self::Image => &["asset", "fit", "shadow"],
            Self::Shape => &["shape", "radius", "fill", "stroke", "shadow"],
            Self::Group => &["children"],
        }
    }
}

/// Fields every element kind has.
pub const COMMON_FIELDS: [&str; 13] = [
    "page",
    "lock_page",
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

/// Fold state of a foldable device model (`fold: open|folded`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Fold {
    /// Unfolded: the large inner display.
    #[default]
    Open,
    /// Closed: the outer display.
    Folded,
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

/// Outline of a shape or of text glyphs (`stroke: { color: "@ink", width: 4 }`).
/// Width in lu; `0` draws nothing (and switches off a text style's outline
/// in an element or span override).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(remote = "Self", deny_unknown_fields)]
pub struct Stroke {
    pub color: ColorRef,
    #[serde(serialize_with = "num")]
    pub width: f64,
    /// Text only: where the outline sits relative to the glyph edge
    /// (default `outside`). Shapes always stroke centred on the path.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position: Option<StrokePosition>,
    /// Corner join (default `round` for text, `miter` for shapes).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub join: Option<StrokeJoin>,
}
flow_style!(Stroke);

impl Stroke {
    /// A stroke with default position and join.
    pub fn new(color: ColorRef, width: f64) -> Self {
        Self {
            color,
            width,
            position: None,
            join: None,
        }
    }
}

/// Position of a text outline.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StrokePosition {
    /// Entirely outside the glyph: drawn behind the fill with twice the
    /// width, so the glyph shapes stay intact.
    #[default]
    Outside,
    /// Centred on the glyph edge, drawn over the fill.
    Center,
}

/// Corner join of an outline.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StrokeJoin {
    #[default]
    Round,
    Miter,
    Bevel,
}

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
            /// Per-language overrides (text only; not patchable).
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub langs: Option<BTreeMap<Lang, ElementPatch>>,
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
                if self.langs.is_some() { out.push("langs"); }
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

            /// Names of all patchable fields (YAML names, file order).
            pub fn field_names() -> Vec<&'static str> {
                vec![$( field_name(stringify!($field)) ),*]
            }

            /// Is `name` a patchable field?
            pub fn is_field_name(name: &str) -> bool {
                Self::field_names().contains(&name)
            }

            /// Remove field `name` from the patch. Returns whether it was set
            /// (`false` also for unknown names).
            pub fn clear_field(&mut self, name: &str) -> bool {
                $(
                    if name == field_name(stringify!($field)) {
                        return self.$field.take().is_some();
                    }
                )*
                false
            }

            /// A patch with only the fields of `self` named in `names`.
            #[must_use]
            pub fn only(&self, names: &[&str]) -> ElementPatch {
                ElementPatch {
                    $( $field: if names.contains(&field_name(stringify!($field))) {
                        self.$field.clone()
                    } else {
                        None
                    }, )*
                }
            }

            /// Names of the fields set to `null` (reset).
            pub fn null_fields(&self) -> Vec<&'static str> {
                let mut out = Vec::new();
                $( if matches!(self.$field, Some(None)) { out.push(field_name(stringify!($field))); } )*
                out
            }

            /// Remove the fields set to `null`; returns their names. In an
            /// override a `null` would mean "reset to the kind's default",
            /// which is never what an override is for: overrides only set
            /// values (ADR 0009 amendment, 2026-11-10).
            pub fn drop_nulls(&mut self) -> Vec<&'static str> {
                let mut out = Vec::new();
                $(
                    if matches!(self.$field, Some(None)) {
                        self.$field = None;
                        out.push(field_name(stringify!($field)));
                    }
                )*
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

impl ElementPatch {
    /// Rename style references set by this patch (`kind` `from` → `to`):
    /// colours (`@name` in `color`, `fill`, `stroke`, `glow`), the text
    /// style and the shadow preset.
    pub fn rename_style_refs(&mut self, kind: super::style::StyleKind, from: &str, to: &str) {
        use super::style::StyleKind;
        let rename_str = |v: &mut Option<Option<String>>| {
            if let Some(Some(s)) = v
                && s == from
            {
                to.clone_into(s);
            }
        };
        match kind {
            StyleKind::Color => {
                let named = |c: &mut ColorRef| {
                    if matches!(c, ColorRef::Named(n) if n == from) {
                        *c = ColorRef::Named(to.to_owned());
                    }
                };
                if let Some(Some(c)) = &mut self.color
                    && c.strip_prefix('@') == Some(from)
                {
                    *c = format!("@{to}");
                }
                if let Some(Some(f)) = &mut self.fill {
                    f.colors_mut().into_iter().for_each(named);
                }
                if let Some(Some(s)) = &mut self.stroke {
                    named(&mut s.color);
                }
                if let Some(Some(g)) = &mut self.glow {
                    named(&mut g.color);
                }
            }
            StyleKind::Text => rename_str(&mut self.style),
            StyleKind::Shadow => rename_str(&mut self.shadow),
            StyleKind::Span => {}
        }
    }
}

fn field_name(s: &'static str) -> &'static str {
    s.strip_prefix("r#").unwrap_or(s)
}

element_fields! {
    page: u32,
    lock_page: bool,
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
    bezel: String,
    color: String,
    fold: Fold,
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
    hyphenate: bool,
    // image
    asset: String,
    // shape
    shape: ShapeKind,
    #[serde(serialize_with = "num")]
    radius: f64,
    stroke: Stroke,
    // text
    glow: Glow,
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

/// A language override may only set [`LANG_OVERRIDE_FIELDS`], must not be
/// empty and must give a valid text element.
fn check_lang_override(w: &ElementWire, lang: &Lang, patch: &ElementPatch) -> Result<(), String> {
    if patch.is_empty() {
        return Err(format!(
            "element `{}`: language override `{lang}` is empty",
            w.id
        ));
    }
    if let Some(f) = patch
        .fields()
        .into_iter()
        .find(|f| !LANG_OVERRIDE_FIELDS.contains(f))
    {
        return Err(format!(
            "element `{}`: field `{f}` cannot be overridden per language (`langs.{lang}`)",
            w.id
        ));
    }
    let mut base = w.clone();
    base.langs = None;
    patch.apply_to_wire(&mut base);
    Element::try_from(base).map_err(|e| format!("language override `{lang}`: {e}"))?;
    Ok(())
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
        if let Some(st) = &w.stroke
            && !(st.width >= 0.0 && st.width.is_finite())
        {
            return Err(format!("element `{}`: stroke width must be ≥ 0", w.id));
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
                if w.bezel.as_deref().is_some_and(|b| b.trim().is_empty()) {
                    return Err(format!("element `{}`: `bezel` must not be empty", w.id));
                }
                ElementKind::Device(DeviceProps {
                    model,
                    orientation: w.orientation,
                    frame: w.frame.clone(),
                    bezel: w.bezel.clone(),
                    color: w.color.clone(),
                    fold: w.fold,
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
                if let Some(g) = &w.glow {
                    g.check().map_err(|e| format!("element `{}`: {e}", w.id))?;
                }
                let langs = w.langs.clone().unwrap_or_default();
                for (lang, patch) in &langs {
                    check_lang_override(&w, lang, patch)?;
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
                    stroke: w.stroke.clone(),
                    glow: w.glow.clone(),
                    shadow: w.shadow.clone(),
                    hyphenate: w.hyphenate,
                    langs,
                })
            }
            ElementKindTag::Image => ElementKind::Image(ImageProps {
                asset: w.asset.clone().ok_or_else(|| missing("asset"))?,
                fit: fit_mode(w.fit.clone())?,
                shadow: w.shadow.clone(),
            }),
            ElementKindTag::Shape if w.stroke.as_ref().is_some_and(|s| s.position.is_some()) => {
                return Err(format!(
                    "element `{}`: stroke `position` applies to text only",
                    w.id
                ));
            }
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
            lock_page: w.lock_page.unwrap_or(false),
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
            lock_page: e.lock_page.then_some(true),
            at: (!e.at.is_zero()).then_some(e.at),
            width: e.width,
            height: e.height,
            rotation: (e.rotation != 0.0).then_some(e.rotation),
            pivot: e.pivot,
            anchor_y: e.anchor_y,
            model: None,
            orientation: None,
            frame: None,
            bezel: None,
            color: None,
            fold: None,
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
            hyphenate: None,
            asset: None,
            shape: None,
            radius: None,
            stroke: None,
            glow: None,
            fill: None,
            fit: None,
            shadow: None,
            opacity: ((e.opacity - 1.0).abs() > f64::EPSILON).then_some(e.opacity),
            z: (e.z != 0).then_some(e.z),
            clip: e.clip,
            visible: (!e.visible).then_some(false),
            visible_in: e.visible_in,
            langs: None,
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
                w.bezel = p.bezel;
                w.color = p.color;
                w.fold = p.fold;
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
                w.stroke = p.stroke;
                w.glow = p.glow;
                w.shadow = p.shadow;
                w.hyphenate = p.hyphenate;
                w.langs = (!p.langs.is_empty()).then_some(p.langs);
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

    /// Removes `null` fields from the language overrides of this element
    /// and its children (empty overrides are removed); returns where they
    /// were.
    pub fn drop_lang_override_nulls(&mut self) -> Vec<String> {
        let mut out = Vec::new();
        if let ElementKind::Text(t) = &mut self.kind {
            let id = &self.id;
            t.langs.retain(|lang, p| {
                for f in p.drop_nulls() {
                    out.push(format!("element `{id}`, langs.{lang}: {f}"));
                }
                !p.is_empty()
            });
        }
        if let Some(children) = self.children_mut() {
            for c in children {
                out.extend(c.drop_lang_override_nulls());
            }
        }
        out
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
            ElementKind::Text(p) => {
                let mut v: Vec<&ColorRef> = p.color.iter().collect();
                v.extend(p.stroke.as_ref().map(|s| &s.color));
                v.extend(p.glow.as_ref().map(|g| &g.color));
                v
            }
            ElementKind::Shape(p) => {
                let mut v: Vec<&ColorRef> = p.fill.iter().flat_map(Fill::colors).collect();
                v.extend(p.stroke.as_ref().map(|s| &s.color));
                v
            }
            ElementKind::Device(_) | ElementKind::Image(_) | ElementKind::Group(_) => Vec::new(),
        }
    }

    /// Mutable colour references of this element (not its children, not
    /// its language overrides).
    pub fn color_refs_mut(&mut self) -> Vec<&mut ColorRef> {
        match &mut self.kind {
            ElementKind::Background(p) => p.fill.colors_mut(),
            ElementKind::Text(p) => {
                let mut v: Vec<&mut ColorRef> = p.color.iter_mut().collect();
                v.extend(p.stroke.as_mut().map(|s| &mut s.color));
                v.extend(p.glow.as_mut().map(|g| &mut g.color));
                v
            }
            ElementKind::Shape(p) => {
                let mut v: Vec<&mut ColorRef> =
                    p.fill.iter_mut().flat_map(Fill::colors_mut).collect();
                v.extend(p.stroke.as_mut().map(|s| &mut s.color));
                v
            }
            ElementKind::Device(_) | ElementKind::Image(_) | ElementKind::Group(_) => Vec::new(),
        }
    }

    /// Mutable shadow preset reference of this element.
    pub fn shadow_mut(&mut self) -> Option<&mut Option<String>> {
        match &mut self.kind {
            ElementKind::Device(p) => Some(&mut p.shadow),
            ElementKind::Image(p) => Some(&mut p.shadow),
            ElementKind::Shape(p) => Some(&mut p.shadow),
            ElementKind::Text(p) => Some(&mut p.shadow),
            _ => None,
        }
    }

    /// Shadow preset used by this element.
    pub fn shadow(&self) -> Option<&str> {
        match &self.kind {
            ElementKind::Device(p) => p.shadow.as_deref(),
            ElementKind::Image(p) => p.shadow.as_deref(),
            ElementKind::Shape(p) => p.shadow.as_deref(),
            ElementKind::Text(p) => p.shadow.as_deref(),
            _ => None,
        }
    }

    /// Text properties (text elements only).
    pub fn text_props(&self) -> Option<&TextProps> {
        match &self.kind {
            ElementKind::Text(t) => Some(t),
            _ => None,
        }
    }

    /// The element with its language override for `lang` applied (see
    /// [`TextProps::lang_override`]); the matched key, or `None` if no
    /// override applies. An override that does not apply cleanly is
    /// returned as error.
    pub fn for_lang(&self, lang: &str) -> Result<Option<(Lang, Element)>, String> {
        let Some((key, patch)) = self.text_props().and_then(|t| t.lang_override(lang)) else {
            return Ok(None);
        };
        let (e, _) = self.patched(&patch)?;
        Ok(Some((key.clone(), e)))
    }

    /// The element once per language override (with that override applied).
    pub fn lang_variants(&self) -> Vec<(Lang, Result<Element, String>)> {
        self.text_props()
            .map(|t| {
                t.langs
                    .iter()
                    .map(|(l, p)| (l.clone(), self.patched(p).map(|(e, _)| e)))
                    .collect()
            })
            .unwrap_or_default()
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
    fn text_effects_fields() {
        let y = "id: t\nkind: text\npage: 0\ntext: a.b\nstroke: {color: \"@ink\", width: 4, position: center}\nglow: {color: \"#FFFFFF\", radius: 12, opacity: 0.5}\nshadow: soft\n";
        let e = parse(y).unwrap();
        let ElementKind::Text(t) = &e.kind else {
            panic!()
        };
        assert_eq!(
            t.stroke.as_ref().unwrap().position,
            Some(StrokePosition::Center)
        );
        assert_eq!(t.glow.as_ref().unwrap().opacity, Some(0.5));
        assert_eq!(e.shadow(), Some("soft"));
        assert_eq!(e.color_refs().len(), 2);
        assert_eq!(crate::io::to_yaml(&e).unwrap(), y);
        let err =
            parse("id: t\nkind: text\ntext: a\nglow: {color: \"#FFF\", radius: 1, opacity: 2}\n")
                .unwrap_err();
        assert!(err.contains("glow opacity"), "{err}");
        let err = parse("id: s\nkind: shape\nshape: rect\nstroke: {color: \"#000\", width: 1, position: outside}\n")
            .unwrap_err();
        assert!(err.contains("text only"), "{err}");
        let err = parse("id: s\nkind: shape\nshape: rect\nstroke: {color: \"#000\", width: -1}\n")
            .unwrap_err();
        assert!(err.contains("stroke width"), "{err}");
        assert!(
            parse("id: i\nkind: image\nasset: a.png\nglow: {color: \"#000\", radius: 1}\n")
                .is_err()
        );
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
