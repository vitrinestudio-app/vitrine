//! The project model (KONZEPT §3).
//!
//! Conventions (ADR 0001, ADR 0002): a page is 1000 lu wide, y points down,
//! rotation in degrees with positive = clockwise, element positions are
//! relative to the element's home page (0-based `page` index), colours are
//! `#RRGGBB[AA]` or `@name` references into `styles.colors`.

mod canvas;
mod checks;
mod color;
mod common;
mod element;
mod fill;
pub(crate) mod fmt;
mod ids;
mod import;
mod store;
mod style;
mod target;

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

pub use canvas::{Canvas, PageSpec, Variant};
pub use checks::Checks;
pub use color::{ColorRef, Rgba};
pub use common::{AnchorY, Aspect, Clip, FitMode, PixelSize, Point, VisibleIn};
pub use element::{
    Align, Appearance, BackgroundProps, COMMON_FIELDS, Chrome, DeviceProps, Element, ElementKind,
    ElementKindTag, ElementPatch, ElementWire, FitValue, Fold, GROUP_SCALE_RANGE, GroupProps,
    ImageProps, LANG_OVERRIDE_FIELDS, Orientation, ShapeKind, ShapeProps, Stroke, StrokeJoin,
    StrokePosition, TextBox, TextFit, TextProps, VAlign,
};
pub use fill::{Fill, GradientStop, ImageFillMode};
pub use ids::{CanvasId, ElementId, Lang, StringKey, TargetId, VariantId, is_valid_id};
pub use import::{ImportMapping, ImportSettings};
pub use store::{
    ApplePlatform, AppleStore, AppleUploadFlowSetting, GoogleStore, StoreKind, StoreLocaleMaps,
    StoreSettings,
};
pub use style::{
    BUILTIN_SHADOWS, FontSettings, FrameStyle, Glow, ShadowLayer, ShadowPreset, SpanStyle,
    StyleKind, Styles, TextStyle, TextTransform, is_valid_family_name,
};
pub use target::{
    AppleSlot, BannerFit, CreativePlacement, GalleryRef, Gaps, GoogleSlot, ImageFormat, Region,
    RegionRect, SafeArea, StoreSlot, Target, TargetKind,
};

use fmt::flow_seq;

/// Translations: language → (key → text). Texts may contain the inline
/// markup of §3.5; it is parsed by `core::layout`, not here.
pub type Strings = BTreeMap<Lang, BTreeMap<StringKey, String>>;

/// A whole project: `project.yaml` plus the `i18n/*.yaml` strings.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Project {
    /// Always [`crate::PROJECT_FORMAT`].
    pub format: String,
    /// Incremented on every save; used for optimistic locking (§8.3).
    #[serde(default)]
    pub revision: u64,
    pub name: String,
    pub languages: Languages,
    #[serde(default, skip_serializing_if = "Styles::is_empty")]
    pub styles: Styles,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub canvases: Vec<Canvas>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub targets: Vec<Target>,
    /// Store mapping: apps, account references, locale map (Phase 5).
    #[serde(default, skip_serializing_if = "StoreSettings::is_empty")]
    pub store: StoreSettings,
    /// Screenshot import mappings (`core::import`).
    #[serde(default, skip_serializing_if = "ImportSettings::is_empty")]
    pub import: ImportSettings,
    /// Design-check thresholds and codes switched off (`checks:`).
    #[serde(default, skip_serializing_if = "Checks::is_empty")]
    pub checks: Checks,
    /// Translations, stored in `i18n/<lang>.yaml`, not in `project.yaml`.
    #[serde(skip)]
    pub strings: Strings,
    /// Override fields that were `null` in the loaded file and were
    /// dropped (see [`Project::drop_override_nulls`]); reported by
    /// `core::validate` as info until the next save.
    #[serde(skip)]
    pub dropped_override_nulls: Vec<String>,
}

/// Project languages (§5).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Languages {
    /// Default language: fallback for texts and screenshots.
    pub default: Lang,
    /// All languages of the project (including the default).
    #[serde(serialize_with = "flow_seq")]
    pub available: Vec<Lang>,
    /// Extra fallbacks in addition to the automatic chain `de-AT → de → default`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub fallback: BTreeMap<Lang, Lang>,
}

impl Languages {
    pub fn new(default: Lang) -> Self {
        Self {
            available: vec![default.clone()],
            default,
            fallback: BTreeMap::new(),
        }
    }

    pub fn is_available(&self, lang: &str) -> bool {
        self.available.iter().any(|l| l.as_str() == lang)
    }

    /// Check consistency on its own.
    pub fn check(&self) -> Result<(), String> {
        if !self.is_available(self.default.as_str()) {
            return Err(format!(
                "default language `{}` must be listed in `available`",
                self.default
            ));
        }
        let mut seen = std::collections::BTreeSet::new();
        for l in &self.available {
            if !seen.insert(l) {
                return Err(format!("language `{l}` is listed twice"));
            }
        }
        // Fallback entries may name languages outside `available` (as in
        // KONZEPT §3.7, `pt-BR: pt`); they only extend the lookup chain.
        for (from, to) in &self.fallback {
            if from == to {
                return Err(format!("fallback `{from}` points to itself"));
            }
        }
        Ok(())
    }
}

impl Project {
    /// A new empty project.
    pub fn new(name: &str, default_lang: Lang) -> Self {
        Self {
            format: crate::PROJECT_FORMAT.to_owned(),
            revision: 0,
            name: name.to_owned(),
            languages: Languages::new(default_lang),
            styles: Styles::default(),
            canvases: Vec::new(),
            targets: Vec::new(),
            store: StoreSettings::default(),
            import: ImportSettings::default(),
            checks: Checks::default(),
            strings: Strings::new(),
            dropped_override_nulls: Vec::new(),
        }
    }

    /// Removes `null` fields from every variant and language override
    /// (an override only sets values; "back to the base" is the absence
    /// of the field). Returns where they were, e.g. `element `title`,
    /// langs.en: at`.
    pub fn drop_override_nulls(&mut self) -> Vec<String> {
        let mut out = Vec::new();
        for s in &mut self.canvases {
            out.extend(s.drop_override_nulls());
        }
        out
    }

    pub fn canvas(&self, id: &str) -> Option<&Canvas> {
        self.canvases.iter().find(|s| s.id.as_str() == id)
    }

    pub fn target(&self, id: &str) -> Option<&Target> {
        self.targets.iter().find(|t| t.id.as_str() == id)
    }

    /// Find an element by id in any canvas (also inside groups).
    pub fn element(&self, id: &str) -> Option<(&Canvas, &Element)> {
        self.canvases
            .iter()
            .find_map(|s| s.element(id).map(|e| (s, e)))
    }

    /// Text of `key` in `lang` (no fallback; see `core::resolve`).
    pub fn string(&self, lang: &str, key: &str) -> Option<&str> {
        self.strings.get(lang)?.get(key).map(String::as_str)
    }

    /// Structural checks that make the model unambiguous: unique ids,
    /// self-contained target and language rules. Cross references (styles,
    /// colours, page ranges …) are reported by `core::validate`, not here,
    /// so a hand-edited project with a dangling reference still loads.
    pub fn check_structure(&self) -> Result<(), String> {
        if self.format != crate::PROJECT_FORMAT {
            // The product was called Arrangia before 2026-11-11; no
            // migration, just say what to change.
            if self.format.starts_with("arrangia/") {
                return Err(format!(
                    "project from Arrangia (format `{}`) – change `format:` in project.yaml to `{}`",
                    self.format,
                    crate::PROJECT_FORMAT
                ));
            }
            return Err(format!(
                "unsupported format `{}` (expected `{}`)",
                self.format,
                crate::PROJECT_FORMAT
            ));
        }
        self.languages.check()?;
        let mut canvases = std::collections::BTreeSet::new();
        let mut elements = std::collections::BTreeSet::new();
        for s in &self.canvases {
            if !canvases.insert(&s.id) {
                return Err(format!("duplicate canvas id `{}`", s.id));
            }
            s.page
                .check()
                .map_err(|e| format!("canvas `{}`: {e}", s.id))?;
            let mut dup = None;
            s.walk_elements(&mut |e| {
                if !elements.insert(&e.id) && dup.is_none() {
                    dup = Some(e.id.clone());
                }
            });
            if let Some(id) = dup {
                return Err(format!("duplicate element id `{id}`"));
            }
        }
        let mut targets = std::collections::BTreeSet::new();
        for t in &self.targets {
            if !targets.insert(&t.id) {
                return Err(format!("duplicate target id `{}`", t.id));
            }
            t.check()?;
        }
        self.store.check()?;
        self.import.check()?;
        Ok(())
    }
}
