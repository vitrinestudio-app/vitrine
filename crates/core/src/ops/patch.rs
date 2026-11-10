//! Patch types for strips and targets. Element patches are
//! [`crate::model::ElementPatch`].
//!
//! Every field is `Option<FieldType>`: `None` = unchanged. For fields that
//! are optional in the model the type is `Option<Option<T>>`, where
//! `Some(None)` (JSON/YAML `null`) resets the field.

#![allow(clippy::option_option)]

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::model::fmt::double_option;
use crate::model::{
    Aspect, BannerFit, Fill, GalleryRef, Gaps, ImageFormat, Lang, PixelSize, Region, SafeArea,
    StoreSlot, Strip, StripId, Target, TargetKind, VariantId,
};

/// Swap patch values into the target and record the old values in `inv`.
macro_rules! swap_fields {
    ($patch:expr, $target:expr, $inv:expr; $($field:ident),* $(,)?) => {
        $(
            if let Some(v) = &$patch.$field {
                $inv.$field = Some(std::mem::replace(&mut $target.$field, v.clone()));
            }
        )*
    };
}

/// Partial update of a strip's page geometry and backgrounds.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StripPatch {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aspect: Option<Aspect>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub count: Option<u32>,
    /// Page ids (`[]` removes all explicit ids).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ids: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backgrounds: Option<Vec<Fill>>,
}

impl StripPatch {
    /// Apply to a strip, return the inverse patch.
    pub(crate) fn apply(&self, s: &mut Strip) -> StripPatch {
        let mut inv = StripPatch::default();
        if let Some(a) = self.aspect {
            inv.aspect = Some(std::mem::replace(&mut s.page.aspect, a));
        }
        if let Some(c) = self.count {
            inv.count = Some(std::mem::replace(&mut s.page.count, c));
        }
        if let Some(ids) = &self.ids {
            inv.ids = Some(std::mem::replace(&mut s.page.ids, ids.clone()));
        }
        if let Some(b) = &self.backgrounds {
            inv.backgrounds = Some(std::mem::replace(&mut s.backgrounds, b.clone()));
        }
        inv
    }
}

/// Partial update of a target (all fields except `id`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TargetPatch {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<TargetKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strip: Option<StripId>,
    #[serde(
        default,
        deserialize_with = "double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub variant: Option<Option<VariantId>>,
    #[serde(
        default,
        deserialize_with = "double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub group: Option<Option<u32>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<PixelSize>,
    #[serde(
        default,
        deserialize_with = "double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub screenshots: Option<Option<String>>,
    #[serde(
        default,
        deserialize_with = "double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub gallery: Option<Option<GalleryRef>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device_map: Option<BTreeMap<String, String>>,
    #[serde(
        default,
        deserialize_with = "double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub region: Option<Option<Region>>,
    #[serde(
        default,
        deserialize_with = "double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub fit: Option<Option<BannerFit>>,
    #[serde(
        default,
        deserialize_with = "double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub gaps: Option<Option<Gaps>>,
    #[serde(
        default,
        deserialize_with = "double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub format: Option<Option<ImageFormat>>,
    #[serde(
        default,
        deserialize_with = "double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub quality: Option<Option<u8>>,
    #[serde(
        default,
        deserialize_with = "double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub langs: Option<Option<Vec<Lang>>>,
    #[serde(
        default,
        deserialize_with = "double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub store: Option<Option<StoreSlot>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub screenshot_only: Option<bool>,
    #[serde(
        default,
        deserialize_with = "double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub safe_area: Option<Option<SafeArea>>,
}

impl TargetPatch {
    /// Apply to a target, return the inverse patch.
    pub(crate) fn apply(&self, t: &mut Target) -> TargetPatch {
        let mut inv = TargetPatch::default();
        swap_fields!(self, t, inv;
            kind, strip, variant, group, size, screenshots, gallery, device_map,
            region, fit, gaps, format, quality, langs, store, screenshot_only, safe_area);
        inv
    }
}
