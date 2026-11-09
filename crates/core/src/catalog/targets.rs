//! Target presets and store rules (`data/targets.yaml`, KONZEPT §6.4).

use std::collections::BTreeMap;

use serde::Deserialize;

use super::Catalog;
use crate::model::{Aspect, ImageFormat, PixelSize, StoreSlot, TargetKind, is_valid_id};

/// `targets.yaml`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TargetsFile {
    /// Date the values were last checked (`YYYY-MM-DD`).
    pub checked: String,
    /// Source key (`A1`) → source.
    pub sources: BTreeMap<String, Source>,
    /// Known store API slot values.
    pub store_slots: StoreSlots,
    /// Rule set id → rules.
    pub rules: BTreeMap<String, StoreRules>,
    pub presets: Vec<TargetPreset>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Source {
    pub url: String,
    pub kind: SourceKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceKind {
    /// Official Apple/Google page or API reference.
    Primary,
    /// Third party (fastlane, guides).
    Secondary,
    /// Developer forum thread.
    Forum,
    /// Own measurement.
    Measured,
    /// Common practice, no rule.
    Convention,
}

/// Known store API values for store slots.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StoreSlots {
    /// App Store Connect `ScreenshotDisplayType` values.
    pub apple: Vec<String>,
    /// Android Publisher `AppImageType` values.
    pub google: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Store {
    Apple,
    Google,
    /// Not a store asset (banners).
    None,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Platform {
    Ios,
    Ipados,
    Macos,
    Android,
    Chromeos,
    Web,
}

/// Image count per set: `{min, max}`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Count {
    pub min: u32,
    pub max: u32,
}

/// Whether only the preset sizes are accepted or any size within limits.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SizePolicy {
    /// The image must have exactly one of the preset's sizes.
    Exact,
    /// Any size within `min_side`/`max_side`/`max_ratio`/`aspects`.
    Range,
}

/// A store's rules for one kind of image set.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StoreRules {
    pub label: String,
    pub store: Store,
    #[serde(default)]
    pub count: Option<Count>,
    pub formats: Vec<ImageFormat>,
    /// `false` = alpha channel / transparency not allowed.
    pub alpha: bool,
    pub sizes: SizePolicy,
    #[serde(default)]
    pub min_side: Option<u32>,
    #[serde(default)]
    pub max_side: Option<u32>,
    /// Long side / short side must not exceed this.
    #[serde(default)]
    pub max_ratio: Option<f64>,
    /// Allowed aspect ratios (empty = any).
    #[serde(default)]
    pub aspects: Vec<Aspect>,
    /// Maximum file size in bytes (`None` = not documented).
    #[serde(default)]
    pub max_file_size: Option<u64>,
    #[serde(default)]
    pub note: Option<String>,
    #[serde(default)]
    pub sources: Vec<String>,
}

/// Tolerance for `aspects` (pixel rounding, e.g. 1080x1920 vs 16:9 exact).
const RULE_ASPECT_TOLERANCE: f64 = 0.005;

impl StoreRules {
    /// Problems of an image size against the side/ratio/aspect limits
    /// (empty = fine). The `exact` policy is checked by
    /// [`TargetPreset::has_size`].
    pub fn check_size(&self, size: PixelSize) -> Vec<String> {
        let mut problems = Vec::new();
        let (long, short) = (size.w.max(size.h), size.w.min(size.h));
        if let Some(min) = self.min_side
            && short < min
        {
            problems.push(format!(
                "{}x{}: sides must be at least {min} px",
                size.w, size.h
            ));
        }
        if let Some(max) = self.max_side
            && long > max
        {
            problems.push(format!(
                "{}x{}: sides must be at most {max} px",
                size.w, size.h
            ));
        }
        if let Some(r) = self.max_ratio
            && f64::from(long) > r * f64::from(short)
        {
            problems.push(format!(
                "{}x{}: long side must be at most {r}x the short side",
                size.w, size.h
            ));
        }
        if !self.aspects.is_empty() {
            let ratio = f64::from(size.w) / f64::from(size.h);
            let ok = self.aspects.iter().any(|a| {
                let want = f64::from(a.w) / f64::from(a.h);
                (ratio / want - 1.0).abs() <= RULE_ASPECT_TOLERANCE
            });
            if !ok {
                let list: Vec<String> = self.aspects.iter().map(ToString::to_string).collect();
                problems.push(format!(
                    "{}x{}: aspect ratio must be one of {}",
                    size.w,
                    size.h,
                    list.join(", ")
                ));
            }
        }
        problems
    }
}

/// Export sizes of a preset by orientation; the first one is the default.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PresetSizes {
    #[serde(default)]
    pub portrait: Vec<PixelSize>,
    #[serde(default)]
    pub landscape: Vec<PixelSize>,
}

impl PresetSizes {
    pub fn all(&self) -> impl Iterator<Item = PixelSize> + '_ {
        self.portrait.iter().chain(&self.landscape).copied()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Requirement {
    /// Needed to publish.
    Required,
    /// Needed unless another set is provided (see the preset note).
    Conditional,
    Optional,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PresetStatus {
    /// Backed by primary (Apple/Google) sources.
    Verified,
    /// Partly backed only by third-party sources (see note).
    Secondary,
    /// Not confirmed by the store (e.g. undocumented API value).
    Unverified,
    /// Common practice, not a store rule.
    Convention,
}

/// A target preset: sizes, store slot and defaults for a new target.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TargetPreset {
    pub id: String,
    pub label: String,
    pub store: Store,
    pub platform: Platform,
    #[serde(default)]
    pub kind: TargetKind,
    pub sizes: PresetSizes,
    /// Store upload slot (`None` = no API slot, e.g. Chromebook, banners).
    #[serde(default)]
    pub slot: Option<StoreSlot>,
    /// Default gallery profile.
    #[serde(default)]
    pub gallery: Option<String>,
    /// Default device model.
    #[serde(default)]
    pub device: Option<String>,
    /// Default screenshot variant key.
    #[serde(default)]
    pub screenshots: Option<String>,
    /// Rule set id in `rules`.
    #[serde(default)]
    pub rules: Option<String>,
    #[serde(default)]
    pub requirement: Option<Requirement>,
    pub status: PresetStatus,
    #[serde(default)]
    pub note: Option<String>,
    #[serde(default)]
    pub sources: Vec<String>,
}

impl TargetPreset {
    /// Default export size (first portrait size, else first landscape size).
    pub fn default_size(&self) -> Option<PixelSize> {
        self.sizes
            .portrait
            .first()
            .or(self.sizes.landscape.first())
            .copied()
    }

    /// Is `size` one of the preset's sizes (either orientation)?
    pub fn has_size(&self, size: PixelSize) -> bool {
        self.sizes.all().any(|s| s == size)
    }
}

pub(super) fn check(c: &Catalog, errors: &mut Vec<String>) {
    let file = &c.targets;
    for (key, s) in &file.sources {
        if !s.url.starts_with("https://") {
            errors.push(format!("targets: source {key} needs an https URL"));
        }
    }
    for (id, r) in &file.rules {
        if !is_valid_id(id) {
            errors.push(format!("targets: invalid rules id `{id}`"));
        }
        if r.formats.is_empty() {
            errors.push(format!("rules {id}: no formats"));
        }
        if let Some(n) = r.count
            && n.min > n.max
        {
            errors.push(format!("rules {id}: count min > max"));
        }
        if let (Some(a), Some(b)) = (r.min_side, r.max_side)
            && a > b
        {
            errors.push(format!("rules {id}: min_side > max_side"));
        }
        check_sources(&format!("rules {id}"), &r.sources, file, errors);
    }
    let mut seen = std::collections::BTreeSet::new();
    for p in &file.presets {
        let id = &p.id;
        let what = format!("preset {id}");
        if !is_valid_id(id) {
            errors.push(format!("targets: invalid preset id `{id}`"));
        }
        if !seen.insert(id.as_str()) {
            errors.push(format!("targets: duplicate preset id `{id}`"));
        }
        if p.default_size().is_none() {
            errors.push(format!("{what}: no sizes"));
        }
        for s in &p.sizes.portrait {
            if s.w >= s.h {
                errors.push(format!("{what}: {}x{} is not portrait", s.w, s.h));
            }
        }
        for s in &p.sizes.landscape {
            if s.w <= s.h {
                errors.push(format!("{what}: {}x{} is not landscape", s.w, s.h));
            }
        }
        if let Some(g) = &p.gallery
            && c.gallery(g).is_none()
        {
            errors.push(format!("{what}: unknown gallery `{g}`"));
        }
        if let Some(d) = &p.device
            && c.device(d).is_none()
        {
            errors.push(format!("{what}: unknown device `{d}`"));
        }
        match p.rules.as_deref().map(|r| (r, file.rules.get(r))) {
            Some((r, None)) => errors.push(format!("{what}: unknown rules `{r}`")),
            Some((_, Some(rules))) => {
                if rules.store != p.store {
                    errors.push(format!("{what}: rules are for another store"));
                }
                for s in p.sizes.all() {
                    for problem in rules.check_size(s) {
                        errors.push(format!("{what}: {problem}"));
                    }
                }
            }
            None if p.store != Store::None => {
                errors.push(format!("{what}: store presets need `rules`"));
            }
            None => {}
        }
        check_slot(p, file, errors);
        check_sources(&what, &p.sources, file, errors);
        if p.status != PresetStatus::Convention && p.sources.is_empty() {
            errors.push(format!("{what}: needs sources"));
        }
        if matches!(p.status, PresetStatus::Secondary | PresetStatus::Unverified)
            && p.note.as_deref().is_none_or(str::is_empty)
        {
            errors.push(format!("{what}: {:?} status needs a note", p.status));
        }
    }
}

fn check_slot(p: &TargetPreset, file: &TargetsFile, errors: &mut Vec<String>) {
    let Some(slot) = &p.slot else { return };
    let what = format!("preset {}", p.id);
    let (value, known, store) = match (&slot.apple, &slot.google) {
        (Some(a), None) => (&a.display_type, &file.store_slots.apple, Store::Apple),
        (None, Some(g)) => (&g.image_type, &file.store_slots.google, Store::Google),
        _ => {
            errors.push(format!("{what}: slot must name exactly one store"));
            return;
        }
    };
    if store != p.store {
        errors.push(format!("{what}: slot is for another store"));
    }
    if !known.contains(value) && p.status != PresetStatus::Unverified {
        errors.push(format!(
            "{what}: `{value}` is not a known store value (mark the preset unverified)"
        ));
    }
}

fn check_sources(what: &str, keys: &[String], file: &TargetsFile, errors: &mut Vec<String>) {
    for k in keys {
        if !file.sources.contains_key(k) {
            errors.push(format!("{what}: unknown source `{k}`"));
        }
    }
}
