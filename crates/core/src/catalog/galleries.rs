//! Store gallery profiles (`data/galleries.yaml`, KONZEPT §3.3).

use serde::Deserialize;

use crate::model::{PixelSize, Rgba, is_valid_id};

/// `galleries.yaml`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GalleriesFile {
    /// Date the values were last checked (`YYYY-MM-DD`).
    pub checked: String,
    pub galleries: Vec<GalleryProfile>,
}

/// What 1000 lu of a profile refer to.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Basis {
    /// 1000 lu = width of one exported image (App Store: fixed widths).
    #[default]
    Width,
    /// 1000 lu = height of one exported image (Google Play: fixed height).
    Height,
}

/// How a profile's values were obtained. Written as `false` (placeholder,
/// not yet measured), `web-css` or `fixed`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Measured {
    /// Not yet measured: placeholder guess.
    No,
    /// Measured from the store website's CSS / layout.
    WebCss,
    /// By definition (e.g. the `none` profile).
    Fixed,
}

impl<'de> Deserialize<'de> for Measured {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl serde::de::Visitor<'_> for V {
            type Value = Measured;
            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("`false`, `web-css` or `fixed`")
            }
            fn visit_bool<E: serde::de::Error>(self, v: bool) -> Result<Measured, E> {
                if v {
                    Err(E::custom(
                        "`measured: true` is ambiguous; use `web-css` or `fixed`",
                    ))
                } else {
                    Ok(Measured::No)
                }
            }
            fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Measured, E> {
                match v {
                    "web-css" => Ok(Measured::WebCss),
                    "fixed" => Ok(Measured::Fixed),
                    _ => Err(E::custom(format!(
                        "unknown `measured` value `{v}`; expected `false`, `web-css` or `fixed`"
                    ))),
                }
            }
        }
        d.deserialize_any(V)
    }
}

/// Corner radius in lu, possibly elliptical (`x` along the width, `y` along
/// the height). Written as a number or `{x, y}`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CornerRadius {
    pub x: f64,
    pub y: f64,
}

impl CornerRadius {
    pub const fn circular(r: f64) -> Self {
        Self { x: r, y: r }
    }

    pub fn is_circular(&self) -> bool {
        (self.x - self.y).abs() < f64::EPSILON
    }
}

impl<'de> Deserialize<'de> for CornerRadius {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Xy {
            x: f64,
            y: f64,
        }
        struct V;
        impl<'de> serde::de::Visitor<'de> for V {
            type Value = CornerRadius;
            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("a radius in lu or `{x, y}`")
            }
            fn visit_f64<E: serde::de::Error>(self, v: f64) -> Result<CornerRadius, E> {
                Ok(CornerRadius::circular(v))
            }
            fn visit_i64<E: serde::de::Error>(self, v: i64) -> Result<CornerRadius, E> {
                Ok(CornerRadius::circular(v as f64))
            }
            fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<CornerRadius, E> {
                Ok(CornerRadius::circular(v as f64))
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                map: A,
            ) -> Result<CornerRadius, A::Error> {
                let xy = Xy::deserialize(serde::de::value::MapAccessDeserializer::new(map))?;
                Ok(CornerRadius { x: xy.x, y: xy.y })
            }
        }
        d.deserialize_any(V)
    }
}

/// A store gallery profile: gap between exported images and their corner
/// radius as the store shows them, in lu of one exported image (see
/// [`Basis`]).
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GalleryProfile {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub basis: Basis,
    pub gap: f64,
    pub radius: CornerRadius,
    /// Page colour behind the gallery (preview only).
    pub background: Rgba,
    pub measured: Measured,
    pub source: String,
    #[serde(default)]
    pub note: Option<String>,
}

impl GalleryProfile {
    /// Factor from profile lu to page lu (page width = 1000 lu) for an
    /// exported image of `image` pixels holding `group` pages side by side.
    pub fn page_lu_per_unit(&self, image: PixelSize, group: u32) -> f64 {
        let group = f64::from(group.max(1));
        match self.basis {
            Basis::Width => group,
            Basis::Height => group * f64::from(image.h) / f64::from(image.w),
        }
    }

    /// A gap `value` of this profile (its own `gap` or a target override,
    /// both in profile lu) converted to page lu on the canvas.
    pub fn gap_on_canvas(&self, value: f64, image: PixelSize, group: u32) -> f64 {
        value * self.page_lu_per_unit(image, group)
    }
}

pub(super) fn check(file: &GalleriesFile, errors: &mut Vec<String>) {
    let mut seen = std::collections::BTreeSet::new();
    for g in &file.galleries {
        let id = &g.id;
        if !is_valid_id(id) {
            errors.push(format!("galleries: invalid id `{id}`"));
        }
        if !seen.insert(id.as_str()) {
            errors.push(format!("galleries: duplicate gallery id `{id}`"));
        }
        if !(g.gap.is_finite() && g.gap >= 0.0) {
            errors.push(format!("gallery {id}: gap must be >= 0"));
        }
        if !(g.radius.x >= 0.0 && g.radius.y >= 0.0) {
            errors.push(format!("gallery {id}: radius must be >= 0"));
        }
        if g.source.trim().is_empty() {
            errors.push(format!("gallery {id}: source is missing"));
        }
        match g.measured {
            Measured::No if g.note.as_deref().is_none_or(str::is_empty) => {
                errors.push(format!("gallery {id}: placeholder needs a note"));
            }
            Measured::WebCss
                if !g.source.contains("apps.apple.com")
                    && !g.source.contains("play.google.com") =>
            {
                errors.push(format!(
                    "gallery {id}: web-css measurement needs the store page as source"
                ));
            }
            _ => {}
        }
    }
}
