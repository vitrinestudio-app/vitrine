//! Project-wide style tokens (§3.6): colours, text styles, span styles,
//! shadow presets and the device frame style.

use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};

use super::color::{ColorRef, Rgba};
use super::element::Stroke;
use super::fmt::{flow_style, is_zero, num, string_serde};

/// Built-in shadow preset names that are always available, even when not
/// defined in `styles.shadows`.
pub const BUILTIN_SHADOWS: [&str; 3] = ["soft", "hard", "none"];

/// Style tokens. Every map is written sorted by name.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Styles {
    /// Named palette colours, referenced as `@name`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub colors: BTreeMap<String, Rgba>,
    /// Named text styles (`headline`, `sub` …).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub text: BTreeMap<String, TextStyle>,
    /// Named inline span styles used as `[name]…[/]` in translations.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub spans: BTreeMap<String, SpanStyle>,
    /// Shadow presets; `soft`, `hard` and `none` exist built in and may be
    /// redefined here.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub shadows: BTreeMap<String, ShadowPreset>,
    /// Default device frame style of the project (default `detailed`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frame: Option<FrameStyle>,
    /// Per-family font settings: fallback families for characters the
    /// family lacks (CJK, Cyrillic …, KONZEPT §5.1), by family name.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub fonts: BTreeMap<String, FontSettings>,
}

/// Settings of one font family (`styles.fonts.<family>`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(remote = "Self", deny_unknown_fields)]
pub struct FontSettings {
    /// Families tried in order for characters missing in this family,
    /// before the default family (Inter).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fallback: Vec<String>,
}
flow_style!(FontSettings);

/// Is `name` usable as a font family name (non-empty, no control
/// characters, no leading/trailing spaces)?
pub fn is_valid_family_name(name: &str) -> bool {
    !name.is_empty() && name.trim() == name && !name.chars().any(char::is_control)
}

impl Styles {
    pub fn is_empty(&self) -> bool {
        self == &Self::default()
    }

    /// Does a shadow preset with this name exist (project or built-in)?
    pub fn has_shadow(&self, name: &str) -> bool {
        self.shadows.contains_key(name) || BUILTIN_SHADOWS.contains(&name)
    }

    /// All colour references used inside the style definitions.
    pub fn color_refs(&self) -> Vec<&ColorRef> {
        let mut out = Vec::new();
        for t in self.text.values() {
            out.extend(t.color.as_ref());
            out.extend(t.stroke.as_ref().map(|s| &s.color));
            out.extend(t.glow.as_ref().map(|g| &g.color));
        }
        for s in self.spans.values() {
            out.extend(s.color.as_ref());
            out.extend(s.background.as_ref());
            out.extend(s.stroke.as_ref().map(|s| &s.color));
            out.extend(s.glow.as_ref().map(|g| &g.color));
        }
        for p in self.shadows.values() {
            out.extend(p.layers.iter().map(|l| &l.color));
        }
        out
    }

    /// Mutable access to every colour reference inside the style
    /// definitions (renaming a palette colour).
    pub fn color_refs_mut(&mut self) -> Vec<&mut ColorRef> {
        let mut out = Vec::new();
        for t in self.text.values_mut() {
            out.extend(t.color.as_mut());
            out.extend(t.stroke.as_mut().map(|s| &mut s.color));
            out.extend(t.glow.as_mut().map(|g| &mut g.color));
        }
        for s in self.spans.values_mut() {
            out.extend(s.color.as_mut());
            out.extend(s.background.as_mut());
            out.extend(s.stroke.as_mut().map(|s| &mut s.color));
            out.extend(s.glow.as_mut().map(|g| &mut g.color));
        }
        for p in self.shadows.values_mut() {
            out.extend(p.layers.iter_mut().map(|l| &mut l.color));
        }
        out
    }

    /// Shadow preset names referenced by text and span styles.
    pub fn shadow_refs(&self) -> impl Iterator<Item = &str> {
        self.text
            .values()
            .filter_map(|t| t.shadow.as_deref())
            .chain(self.spans.values().filter_map(|s| s.shadow.as_deref()))
    }

    /// The fallback lists of `styles.fonts` (family → fallbacks), for
    /// layout.
    pub fn font_fallbacks(&self) -> BTreeMap<String, Vec<String>> {
        self.fonts
            .iter()
            .filter(|(_, f)| !f.fallback.is_empty())
            .map(|(k, f)| (k.clone(), f.fallback.clone()))
            .collect()
    }

    /// Checks font family names in `fonts`.
    pub fn check_fonts(&self) -> Result<(), String> {
        for (family, f) in &self.fonts {
            for name in std::iter::once(family).chain(&f.fallback) {
                if !is_valid_family_name(name) {
                    return Err(format!("invalid font family name `{name}`"));
                }
            }
            if f.fallback.iter().any(|x| x.eq_ignore_ascii_case(family)) {
                return Err(format!("font `{family}` lists itself as fallback"));
            }
        }
        Ok(())
    }

    /// Checks value ranges of the text effects (outline width, glow).
    pub fn check_effects(&self) -> Result<(), String> {
        let named = self
            .text
            .iter()
            .map(|(n, t)| (n, t.stroke.as_ref(), t.glow.as_ref()))
            .chain(
                self.spans
                    .iter()
                    .map(|(n, s)| (n, s.stroke.as_ref(), s.glow.as_ref())),
            );
        for (name, stroke, glow) in named {
            if let Some(s) = stroke
                && !(s.width >= 0.0 && s.width.is_finite())
            {
                return Err(format!("style `{name}`: stroke width must be ≥ 0"));
            }
            if let Some(g) = glow {
                g.check().map_err(|e| format!("style `{name}`: {e}"))?;
            }
        }
        let nonneg = |v: f64| v >= 0.0 && v.is_finite();
        for (name, s) in &self.spans {
            if let Some(p) = s.padding {
                let (x, y) = p.xy();
                if !nonneg(x) || !nonneg(y) {
                    return Err(format!("span style `{name}`: padding must be ≥ 0"));
                }
            }
            if s.radius.is_some_and(|r| !nonneg(r)) {
                return Err(format!("span style `{name}`: radius must be ≥ 0"));
            }
        }
        Ok(())
    }
}

/// Which style map a name belongs to (renaming styles).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StyleKind {
    /// `styles.colors`, referenced as `@name`.
    Color,
    /// `styles.text`.
    Text,
    /// `styles.spans`, referenced as `[name]…[/]` in translations.
    Span,
    /// `styles.shadows`.
    Shadow,
}

/// Text transform.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextTransform {
    None,
    Uppercase,
    Lowercase,
}

/// A text style. Sizes in lu; `line_height` is a factor of the font size;
/// `tracking` is letter spacing in em.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(remote = "Self", deny_unknown_fields)]
pub struct TextStyle {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weight: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub italic: Option<bool>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        serialize_with = "num"
    )]
    pub size: Option<f64>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        serialize_with = "num"
    )]
    pub line_height: Option<f64>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        serialize_with = "num"
    )]
    pub tracking: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<ColorRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transform: Option<TextTransform>,
    /// Shadow preset name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shadow: Option<String>,
    /// Glyph outline.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stroke: Option<Stroke>,
    /// Soft glow around the glyphs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub glow: Option<Glow>,
    /// Automatic hyphenation when the text does not fit at
    /// `fit.shrink_to` (ADR 0009). Default `false`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hyphenate: Option<bool>,
}
flow_style!(TextStyle);

/// A span style for inline markup `[name]…[/]`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(remote = "Self", deny_unknown_fields)]
pub struct SpanStyle {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weight: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub italic: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<ColorRef>,
    /// Highlight colour behind the span.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background: Option<ColorRef>,
    /// Space between the text and the edge of the `background` box, lu:
    /// one value or `{ x, y }`. The box grows; the text does not move
    /// (ADR 0031).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub padding: Option<SpanPadding>,
    /// Corner radius of the `background` box, lu (clamped to half its
    /// height: a large value gives a pill).
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        serialize_with = "num"
    )]
    pub radius: Option<f64>,
    /// Glyph outline of the span (replaces the text's outline).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stroke: Option<Stroke>,
    /// Glow of the span (replaces the text's glow).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub glow: Option<Glow>,
    /// Shadow preset of the span (replaces the text's shadow).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shadow: Option<String>,
}
flow_style!(SpanStyle);

/// Padding of a span background (`padding: 8` or `padding: { x: 16, y: 4 }`), lu.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SpanPadding {
    /// The same on all sides.
    All(f64),
    /// Left/right `x`, top/bottom `y`.
    Xy(super::common::Point),
}

impl SpanPadding {
    /// Horizontal and vertical padding.
    pub fn xy(self) -> (f64, f64) {
        match self {
            Self::All(v) => (v, v),
            Self::Xy(p) => (p.x, p.y),
        }
    }
}

impl Serialize for SpanPadding {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::All(v) => num(v, s),
            Self::Xy(p) => p.serialize(s),
        }
    }
}

impl<'de> Deserialize<'de> for SpanPadding {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> serde::de::Visitor<'de> for V {
            type Value = SpanPadding;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a number (lu) or `{ x, y }`")
            }
            fn visit_f64<E: serde::de::Error>(self, v: f64) -> Result<SpanPadding, E> {
                Ok(SpanPadding::All(v))
            }
            #[allow(clippy::cast_precision_loss)]
            fn visit_i64<E: serde::de::Error>(self, v: i64) -> Result<SpanPadding, E> {
                Ok(SpanPadding::All(v as f64))
            }
            #[allow(clippy::cast_precision_loss)]
            fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<SpanPadding, E> {
                Ok(SpanPadding::All(v as f64))
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                map: A,
            ) -> Result<SpanPadding, A::Error> {
                super::common::Point::deserialize(serde::de::value::MapAccessDeserializer::new(map))
                    .map(SpanPadding::Xy)
            }
        }
        d.deserialize_any(V)
    }
}

/// Soft glow around text glyphs: the glyph shapes, dilated by `spread`,
/// blurred with `radius` (Gaussian sigma) and tinted with `color` ×
/// `opacity`. Lengths in lu. `opacity: 0` switches off a style's glow in an
/// element or span override.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(remote = "Self", deny_unknown_fields)]
pub struct Glow {
    pub color: ColorRef,
    #[serde(serialize_with = "num")]
    pub radius: f64,
    /// `0..=1`, default 1 (multiplied with the colour's alpha).
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        serialize_with = "num"
    )]
    pub opacity: Option<f64>,
    /// Dilation of the glyph shapes before blurring, default 0.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        serialize_with = "num"
    )]
    pub spread: Option<f64>,
}
flow_style!(Glow);

impl Glow {
    /// Range checks (`radius`, `spread` ≥ 0, `opacity` within 0..1).
    pub fn check(&self) -> Result<(), String> {
        let nonneg = |v: f64| v >= 0.0 && v.is_finite();
        if !nonneg(self.radius) {
            return Err("glow radius must be ≥ 0".into());
        }
        if self.spread.is_some_and(|v| !nonneg(v)) {
            return Err("glow spread must be ≥ 0".into());
        }
        if self.opacity.is_some_and(|v| !(0.0..=1.0).contains(&v)) {
            return Err("glow opacity must be within 0..1".into());
        }
        Ok(())
    }
}

/// A shadow preset made of one or more layers (e.g. ambient + contact).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShadowPreset {
    pub layers: Vec<ShadowLayer>,
}

/// One shadow layer: offset `x`/`y` and blur sigma in lu; opacity via the
/// colour's alpha.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(remote = "Self", deny_unknown_fields)]
pub struct ShadowLayer {
    pub color: ColorRef,
    #[serde(serialize_with = "num")]
    pub blur: f64,
    #[serde(default, skip_serializing_if = "is_zero", serialize_with = "num")]
    pub x: f64,
    #[serde(default, skip_serializing_if = "is_zero", serialize_with = "num")]
    pub y: f64,
}
flow_style!(ShadowLayer);

/// Device frame style (§4.1): built-in parametric `stylized` / `detailed`,
/// an imported vendor package (`imported:apple`, ADR 0012) or an own frame
/// pack (`pack:<id>`, ADR 0024).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum FrameStyle {
    Stylized,
    Detailed,
    Imported(String),
    Pack(String),
}

impl FrameStyle {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s {
            "stylized" => Ok(Self::Stylized),
            "detailed" => Ok(Self::Detailed),
            _ => {
                if let Some(src) = s.strip_prefix("imported:")
                    && super::ids::is_valid_id(src)
                {
                    return Ok(Self::Imported(src.to_owned()));
                }
                if let Some(id) = s.strip_prefix("pack:")
                    && super::ids::is_valid_id(id)
                {
                    return Ok(Self::Pack(id.to_owned()));
                }
                Err(format!(
                    "invalid frame style `{s}`: expected `stylized`, `detailed`, `imported:<source>` or `pack:<id>`"
                ))
            }
        }
    }

    /// Key of the frame source in the frame index (`apple`, `pack:clay`)
    /// for frames drawn from image files; `None` for the parametric styles.
    pub fn external_source(&self) -> Option<String> {
        match self {
            Self::Stylized | Self::Detailed => None,
            Self::Imported(s) => Some(s.clone()),
            Self::Pack(id) => Some(format!("pack:{id}")),
        }
    }

    /// Drawn from image files (an imported vendor package or a frame pack)?
    pub fn is_external(&self) -> bool {
        matches!(self, Self::Imported(_) | Self::Pack(_))
    }
}

impl fmt::Display for FrameStyle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Stylized => f.write_str("stylized"),
            Self::Detailed => f.write_str("detailed"),
            Self::Imported(s) => write!(f, "imported:{s}"),
            Self::Pack(id) => write!(f, "pack:{id}"),
        }
    }
}

string_serde!(FrameStyle, "a frame style");
