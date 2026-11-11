//! App Asset Library mapping (`data/asset-library.yaml`, ADR 0019): which
//! placement type, display class and platforms a Vitrine Studio store slot
//! becomes. Placement *groups* are not here – they are runtime strings from
//! App Store Connect's reference data and resolved by the upload.

use std::collections::BTreeMap;
use std::sync::LazyLock;

use serde::{Deserialize, Serialize};

use super::Source;
use crate::model::{AppleSlot, CreativePlacement};

const ASSET_LIBRARY_YAML: &str = include_str!("../../data/asset-library.yaml");

/// `asset-library.yaml`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssetLibraryFile {
    /// Date the enums were last checked (`YYYY-MM-DD`).
    pub checked: String,
    pub sources: BTreeMap<String, Source>,
    /// Legacy display type → placement.
    pub screenshots: BTreeMap<String, ScreenshotPlacement>,
    /// `header` / `search_results` / `universal` → placements.
    pub creative: BTreeMap<String, CreativeMapping>,
    /// Placement type → asset category.
    pub category: BTreeMap<String, String>,
}

/// Placement of a screenshot slot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScreenshotPlacement {
    /// `AppAssetLibraryPlacementType` (`APP_SCREENSHOT`, …).
    pub placement_type: String,
    /// `AppAssetLibraryDisplayClass`.
    pub display_class: String,
    /// `AppAssetLibraryPlacementPlatform` values.
    pub platforms: Vec<String>,
    /// The set-based API cannot upload it.
    #[serde(default)]
    pub asset_library_only: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// Placements of a creative asset slot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreativeMapping {
    pub placement_types: Vec<String>,
    pub platforms: Vec<String>,
}

/// What a slot needs from the asset library (before the group lookup).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SlotPlacements {
    /// One entry per placement type (a universal asset has two).
    pub placement_types: Vec<String>,
    /// Display class the group must have (`None` = any; creative assets).
    pub display_class: Option<String>,
    /// Platforms the group may have.
    pub platforms: Vec<String>,
    /// Asset category for the reservation.
    pub category: String,
    /// Creative assets get one placement per matching group; screenshots
    /// exactly one group.
    pub every_group: bool,
    /// Only the asset library can upload it (Duo, creative assets).
    pub asset_library_only: bool,
}

/// Placement types the set-based API knows nothing about.
const CREATIVE_TYPES: [&str; 2] = [
    "PRODUCT_PAGE_HEADER_ASSET",
    "APP_STORE_SEARCH_RESULTS_ASSET",
];

impl AssetLibraryFile {
    pub fn parse(yaml: &str) -> Result<Self, String> {
        let f: Self = crate::io::from_yaml(yaml).map_err(|e| format!("asset-library.yaml: {e}"))?;
        let errors = f.check();
        if errors.is_empty() {
            Ok(f)
        } else {
            Err(errors.join("; "))
        }
    }

    /// Consistency problems (empty = fine).
    pub fn check(&self) -> Vec<String> {
        let mut errors = Vec::new();
        for (k, s) in &self.screenshots {
            if !self.category.contains_key(&s.placement_type) {
                errors.push(format!(
                    "screenshots.{k}: no category for `{}`",
                    s.placement_type
                ));
            }
            if s.platforms.is_empty() {
                errors.push(format!("screenshots.{k}: no platforms"));
            }
        }
        for name in CreativePlacement::ALL.map(CreativePlacement::as_str) {
            match self.creative.get(name) {
                None => errors.push(format!("creative.{name} is missing")),
                Some(c) => {
                    for t in &c.placement_types {
                        if !self.category.contains_key(t) {
                            errors.push(format!("creative.{name}: no category for `{t}`"));
                        }
                    }
                    if c.placement_types.is_empty() || c.platforms.is_empty() {
                        errors.push(format!("creative.{name}: empty"));
                    }
                }
            }
        }
        errors
    }

    /// The asset-library placements of an App Store slot (`None`: the
    /// display type is not mapped).
    pub fn slot(&self, slot: &AppleSlot) -> Option<SlotPlacements> {
        match slot.placement {
            None => {
                let s = self.screenshots.get(&slot.display_type)?;
                Some(SlotPlacements {
                    placement_types: vec![s.placement_type.clone()],
                    display_class: Some(s.display_class.clone()),
                    platforms: s.platforms.clone(),
                    category: self.category.get(&s.placement_type)?.clone(),
                    every_group: false,
                    asset_library_only: s.asset_library_only,
                })
            }
            Some(p) => {
                let c = self.creative.get(p.as_str())?;
                let first = c.placement_types.first()?;
                Some(SlotPlacements {
                    placement_types: c.placement_types.clone(),
                    display_class: None,
                    platforms: c.platforms.clone(),
                    category: self.category.get(first)?.clone(),
                    every_group: true,
                    asset_library_only: c
                        .placement_types
                        .iter()
                        .any(|t| CREATIVE_TYPES.contains(&t.as_str())),
                })
            }
        }
    }

    /// Only the App Asset Library can upload this slot.
    pub fn asset_library_only(&self, slot: &AppleSlot) -> bool {
        slot.is_creative() || self.slot(slot).is_some_and(|s| s.asset_library_only)
    }
}

static ASSET_LIBRARY: LazyLock<AssetLibraryFile> = LazyLock::new(|| {
    AssetLibraryFile::parse(ASSET_LIBRARY_YAML)
        .unwrap_or_else(|e| panic!("built-in asset library mapping is invalid: {e}"))
});

/// The built-in asset library mapping (parsed on first use).
pub fn asset_library() -> &'static AssetLibraryFile {
    &ASSET_LIBRARY
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn built_in_mapping_is_consistent() {
        let f = asset_library();
        assert_eq!(f.check(), Vec::<String>::new());
        let s = f.slot(&AppleSlot::screenshots("APP_IPHONE_67")).unwrap();
        assert_eq!(s.placement_types, ["APP_SCREENSHOT"]);
        assert_eq!(
            s.display_class.as_deref(),
            Some("IPHONE_DYNAMIC_ISLAND_LARGE_DISPLAY")
        );
        assert_eq!(s.category, "APP_SCREENSHOTS_AND_PREVIEWS");
        assert!(!s.asset_library_only);
        assert!(f.asset_library_only(&AppleSlot::screenshots("APP_IPHONE_DUO")));
        let header = AppleSlot::creative(CreativePlacement::Header);
        let h = f.slot(&header).unwrap();
        assert_eq!(h.placement_types, ["PRODUCT_PAGE_HEADER_ASSET"]);
        assert_eq!(h.category, "CREATIVE_ASSETS");
        assert!(h.every_group && h.asset_library_only);
        let both = AppleSlot::creative(CreativePlacement::Universal);
        assert_eq!(f.slot(&both).unwrap().placement_types.len(), 2);
        assert!(f.slot(&AppleSlot::screenshots("APP_NOPE")).is_none());
        // Every display type of the targets catalog is mapped (creative
        // ones through their placement).
        for t in &crate::catalog::catalog().targets.store_slots.apple {
            if CreativePlacement::ALL.iter().any(|p| p.display_type() == t) {
                continue;
            }
            assert!(
                f.slot(&AppleSlot::screenshots(t.clone())).is_some(),
                "{t} is not mapped"
            );
        }
        for p in &crate::catalog::catalog().targets.presets {
            if let Some(a) = p.slot.as_ref().and_then(|s| s.apple.as_ref()) {
                assert!(f.slot(a).is_some(), "preset {} is not mapped", p.id);
            }
        }
    }
}
