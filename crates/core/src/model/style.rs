//! Project-wide style tokens (§3.6): colours, text styles, span styles,
//! shadow presets and the device frame style.

use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};

use super::color::{ColorRef, Rgba};
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
        }
        for s in self.spans.values() {
            out.extend(s.color.as_ref());
            out.extend(s.background.as_ref());
        }
        for p in self.shadows.values() {
            out.extend(p.layers.iter().map(|l| &l.color));
        }
        out
    }
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
}
flow_style!(SpanStyle);

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
/// or an imported frame package (`imported:apple`).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum FrameStyle {
    Stylized,
    Detailed,
    Imported(String),
}

impl FrameStyle {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s {
            "stylized" => Ok(Self::Stylized),
            "detailed" => Ok(Self::Detailed),
            _ => match s.strip_prefix("imported:") {
                Some(src) if super::ids::is_valid_id(src) => Ok(Self::Imported(src.to_owned())),
                _ => Err(format!(
                    "invalid frame style `{s}`: expected `stylized`, `detailed` or `imported:<source>`"
                )),
            },
        }
    }
}

impl fmt::Display for FrameStyle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Stylized => f.write_str("stylized"),
            Self::Detailed => f.write_str("detailed"),
            Self::Imported(s) => write!(f, "imported:{s}"),
        }
    }
}

string_serde!(FrameStyle, "a frame style");
