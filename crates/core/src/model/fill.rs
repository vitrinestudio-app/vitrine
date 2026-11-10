//! Fills for backgrounds and shapes: colour, gradients, image.

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::color::ColorRef;
use super::common::{FitMode, Point};
use super::fmt::{flow_seq, num};

/// A gradient stop `[colour, position]` with position in `0..=1`.
#[derive(Clone, Debug, PartialEq)]
pub struct GradientStop {
    pub color: ColorRef,
    pub pos: f64,
}

impl GradientStop {
    pub fn new(color: ColorRef, pos: f64) -> Self {
        Self { color, pos }
    }
}

impl Serialize for GradientStop {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeTuple;
        let mut t = s.serialize_tuple(2)?;
        t.serialize_element(&self.color)?;
        t.serialize_element(&super::fmt::NumRef(&self.pos))?;
        t.end()
    }
}

impl<'de> Deserialize<'de> for GradientStop {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let (color, pos) = <(ColorRef, f64)>::deserialize(d)?;
        Ok(Self { color, pos })
    }
}

/// How an image fill covers its area.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageFillMode {
    Cover,
    Contain,
    Stretch,
    Tile,
}

impl From<FitMode> for ImageFillMode {
    fn from(f: FitMode) -> Self {
        match f {
            FitMode::Cover => Self::Cover,
            FitMode::Contain => Self::Contain,
            FitMode::Stretch => Self::Stretch,
        }
    }
}

/// A fill. In YAML a plain colour string (`"#FFFFFF"`, `"@primary"`) is a
/// shorthand for `{ kind: color, color: … }`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(
    remote = "Self",
    tag = "kind",
    rename_all = "kebab-case",
    deny_unknown_fields
)]
pub enum Fill {
    /// Solid colour.
    Color { color: ColorRef },
    /// Linear gradient. `angle` in degrees, CSS convention: 0 = to top,
    /// 90 = to right, 180 (default) = to bottom.
    LinearGradient {
        #[serde(default = "default_angle", serialize_with = "num")]
        angle: f64,
        #[serde(serialize_with = "flow_seq")]
        stops: Vec<GradientStop>,
    },
    /// Radial gradient. `center` in fractions of the filled area (default
    /// centre), `radius` as fraction of the area's larger side (default 0.5).
    RadialGradient {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        center: Option<Point>,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            serialize_with = "num"
        )]
        radius: Option<f64>,
        #[serde(serialize_with = "flow_seq")]
        stops: Vec<GradientStop>,
    },
    /// Image asset (path relative to the project folder).
    Image {
        asset: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        fit: Option<ImageFillMode>,
        /// Focus point in fractions of the image (default centre).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        focus: Option<Point>,
    },
}

fn default_angle() -> f64 {
    180.0
}

impl Fill {
    /// All colours used by this fill.
    pub fn colors(&self) -> Vec<&ColorRef> {
        match self {
            Self::Color { color } => vec![color],
            Self::LinearGradient { stops, .. } | Self::RadialGradient { stops, .. } => {
                stops.iter().map(|s| &s.color).collect()
            }
            Self::Image { .. } => Vec::new(),
        }
    }

    /// Mutable access to all colours of this fill (renaming a colour).
    pub fn colors_mut(&mut self) -> Vec<&mut ColorRef> {
        match self {
            Self::Color { color } => vec![color],
            Self::LinearGradient { stops, .. } | Self::RadialGradient { stops, .. } => {
                stops.iter_mut().map(|s| &mut s.color).collect()
            }
            Self::Image { .. } => Vec::new(),
        }
    }
}

impl Serialize for Fill {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Color { color } => color.serialize(s),
            other => Fill::serialize(other, s),
        }
    }
}

impl<'de> Deserialize<'de> for Fill {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> serde::de::Visitor<'de> for V {
            type Value = Fill;
            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("a colour string or a fill map with `kind`")
            }
            fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Fill, E> {
                ColorRef::parse(v)
                    .map(|color| Fill::Color { color })
                    .map_err(E::custom)
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(self, map: A) -> Result<Fill, A::Error> {
                Fill::deserialize(serde::de::value::MapAccessDeserializer::new(map))
            }
        }
        d.deserialize_any(V)
    }
}
