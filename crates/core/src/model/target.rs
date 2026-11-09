//! Output targets: render a strip into a concrete pixel size, optionally
//! with device swaps, grouping, a variant and a store slot.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::common::PixelSize;
use super::fmt::{flow_map, flow_seq, flow_style, num};
use super::ids::{Lang, StripId, TargetId, VariantId, is_valid_id};

/// Pages export (one image per page or page group) or one banner image.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TargetKind {
    #[default]
    Pages,
    Banner,
}

impl TargetKind {
    #[allow(clippy::trivially_copy_pass_by_ref)] // serde passes references
    fn is_pages(&self) -> bool {
        *self == Self::Pages
    }
}

/// Banner fit: crop (`cover`) or extend the background (`contain`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BannerFit {
    Cover,
    Contain,
}

/// Gaps in a banner: none (seamless) or as in the store gallery.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Gaps {
    None,
    AsStore,
}

/// Export image format.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageFormat {
    Png,
    Jpeg,
}

/// An output target.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Target {
    pub id: TargetId,
    #[serde(default, skip_serializing_if = "TargetKind::is_pages")]
    pub kind: TargetKind,
    pub strip: StripId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub variant: Option<VariantId>,
    /// Pages per export image (`group: 2` = two portrait pages side by side).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<u32>,
    pub size: PixelSize,
    /// Screenshot variant folder (`ios-phone`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub screenshots: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gallery: Option<GalleryRef>,
    /// Device swap: catalog model → replacement model.
    #[serde(
        default,
        skip_serializing_if = "BTreeMap::is_empty",
        serialize_with = "flow_map"
    )]
    pub device_map: BTreeMap<String, String>,
    /// Banner only: part of the strip (default: whole strip).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub region: Option<Region>,
    /// Banner only (default `cover`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fit: Option<BannerFit>,
    /// Banner only (default `none`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gaps: Option<Gaps>,
    /// Default `png`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<ImageFormat>,
    /// JPEG quality 1..=100.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quality: Option<u8>,
    /// Languages to export (default: all available).
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        serialize_with = "flow_seq"
    )]
    pub langs: Option<Vec<Lang>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub store: Option<StoreSlot>,
}

impl Target {
    pub fn new(id: TargetId, strip: StripId, size: PixelSize) -> Self {
        Self {
            id,
            kind: TargetKind::Pages,
            strip,
            variant: None,
            group: None,
            size,
            screenshots: None,
            gallery: None,
            device_map: BTreeMap::new(),
            region: None,
            fit: None,
            gaps: None,
            format: None,
            quality: None,
            langs: None,
            store: None,
        }
    }

    /// Pages per export image (default 1).
    pub fn group_size(&self) -> u32 {
        self.group.unwrap_or(1)
    }

    /// Check the target on its own (no cross references).
    pub fn check(&self) -> Result<(), String> {
        let id = &self.id;
        if self.size.w == 0 || self.size.h == 0 {
            return Err(format!("target `{id}`: size must be positive"));
        }
        if self.group == Some(0) {
            return Err(format!("target `{id}`: group must be >= 1"));
        }
        if let Some(q) = self.quality
            && !(1..=100).contains(&q)
        {
            return Err(format!("target `{id}`: quality must be within 1..=100"));
        }
        for (from, to) in &self.device_map {
            if !is_valid_id(from) || !is_valid_id(to) {
                return Err(format!(
                    "target `{id}`: invalid device_map entry `{from}: {to}`"
                ));
            }
        }
        match self.kind {
            TargetKind::Pages => {
                for (set, name) in [
                    (self.region.is_some(), "region"),
                    (self.fit.is_some(), "fit"),
                    (self.gaps.is_some(), "gaps"),
                ] {
                    if set {
                        return Err(format!(
                            "target `{id}`: `{name}` is only allowed on banner targets"
                        ));
                    }
                }
            }
            TargetKind::Banner => {
                if self.group.is_some() {
                    return Err(format!(
                        "target `{id}`: `group` is not allowed on banner targets"
                    ));
                }
                if let Some(Region::Pages([a, b])) = self.region
                    && a > b
                {
                    return Err(format!("target `{id}`: region pages must be ascending"));
                }
            }
        }
        if let Some(g) = &self.gallery {
            g.check().map_err(|e| format!("target `{id}`: {e}"))?;
        }
        Ok(())
    }
}

/// Gallery profile reference: a preset id from `galleries.yaml`, optionally
/// with overridden gap and corner radius (lu). Written as plain string when
/// nothing is overridden: `gallery: appstore-iphone`, otherwise
/// `gallery: { preset: appstore-iphone, gap: 30 }`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GalleryRef {
    pub preset: Option<String>,
    pub gap: Option<f64>,
    pub radius: Option<f64>,
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "GalleryRef", deny_unknown_fields)]
struct GalleryRefMap {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    preset: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        serialize_with = "num"
    )]
    gap: Option<f64>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        serialize_with = "num"
    )]
    radius: Option<f64>,
}

impl GalleryRef {
    pub fn preset(id: &str) -> Self {
        Self {
            preset: Some(id.to_owned()),
            ..Self::default()
        }
    }

    fn check(&self) -> Result<(), String> {
        if let Some(p) = &self.preset
            && !is_valid_id(p)
        {
            return Err(format!("invalid gallery preset `{p}`"));
        }
        if self.gap.is_some_and(|g| g < 0.0) || self.radius.is_some_and(|r| r < 0.0) {
            return Err("gallery gap and radius must not be negative".into());
        }
        Ok(())
    }

    /// Effective gap and radius given the referenced profile (if found).
    pub fn resolve(&self, profile: Option<&GalleryProfile>) -> (f64, f64) {
        (
            self.gap.or(profile.map(|p| p.gap)).unwrap_or(0.0),
            self.radius.or(profile.map(|p| p.radius)).unwrap_or(0.0),
        )
    }
}

impl Serialize for GalleryRef {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            Self {
                preset: Some(p),
                gap: None,
                radius: None,
            } => s.serialize_str(p),
            other => {
                struct Inner<'a>(&'a GalleryRef);
                impl Serialize for Inner<'_> {
                    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                        GalleryRefMap::serialize(self.0, s)
                    }
                }
                flow_map(&Inner(other), s)
            }
        }
    }
}

impl<'de> Deserialize<'de> for GalleryRef {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> serde::de::Visitor<'de> for V {
            type Value = GalleryRef;
            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("a gallery preset id or `{ preset, gap, radius }`")
            }
            fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<GalleryRef, E> {
                Ok(GalleryRef::preset(v))
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                map: A,
            ) -> Result<GalleryRef, A::Error> {
                GalleryRefMap::deserialize(serde::de::value::MapAccessDeserializer::new(map))
            }
        }
        d.deserialize_any(V)
    }
}

/// A store gallery profile (catalog data, `galleries.yaml`): gap between
/// export images and corner radius, both in lu relative to the page width.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GalleryProfile {
    pub id: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    #[serde(serialize_with = "num")]
    pub gap: f64,
    #[serde(serialize_with = "num")]
    pub radius: f64,
    /// `false` while the values are placeholders (`not yet measured`).
    #[serde(default)]
    pub measured: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

/// Banner region: a page range (inclusive, `{ pages: [0, 2] }`) or a free
/// rectangle relative to a page (`{ rect: { page: 0, x: …, y: …, width: …, height: … } }`).
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Region {
    Pages([u32; 2]),
    Rect(RegionRect),
}

impl Serialize for Region {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        // Written by hand: serde-saphyr ignores `FlowMap` around enum variants.
        struct Entry<'a>(&'a Region);
        impl Serialize for Entry<'_> {
            fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                use serde::ser::SerializeMap;
                let mut m = s.serialize_map(Some(1))?;
                match self.0 {
                    Region::Pages(p) => m.serialize_entry("pages", p)?,
                    Region::Rect(r) => m.serialize_entry("rect", r)?,
                }
                m.end()
            }
        }
        flow_map(&Entry(self), s)
    }
}

/// Free rectangle in lu, relative to the top-left corner of `page`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegionRect {
    #[serde(default)]
    pub page: u32,
    #[serde(serialize_with = "num")]
    pub x: f64,
    #[serde(serialize_with = "num")]
    pub y: f64,
    #[serde(serialize_with = "num")]
    pub width: f64,
    #[serde(serialize_with = "num")]
    pub height: f64,
}

/// Store slot of a target. The values (display types, image types) are
/// checked against the catalog / store APIs later; here they are strings.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(remote = "Self", deny_unknown_fields)]
pub struct StoreSlot {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub apple: Option<AppleSlot>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub google: Option<GoogleSlot>,
}
flow_style!(StoreSlot);

/// App Store Connect screenshot set.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AppleSlot {
    /// `screenshotDisplayType`, e.g. `APP_IPHONE_67`.
    pub display_type: String,
}

/// Google Play image type.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GoogleSlot {
    /// e.g. `phoneScreenshots`, `featureGraphic`.
    pub image_type: String,
}
