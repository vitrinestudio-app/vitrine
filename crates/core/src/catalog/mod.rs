//! Built-in catalogs (KONZEPT §4.1, §3.3, §6.4): device models, target
//! presets with store rules, and store gallery profiles.
//!
//! The data lives in `crates/core/data/*.yaml` and is embedded at build time
//! (`include_str!`), parsed once on first use. Store-specific values are
//! never hard-coded in Rust; every value in the files carries a source and a
//! check date. [`check`] cross-checks the three files (unit-tested, so a bad
//! edit fails `cargo test`).

mod devices;
mod galleries;
mod store_locales;
mod targets;

use std::sync::LazyLock;

pub use devices::{
    BrowserChrome, BrowserTheme, Button, ColorVariant, Cutout, CutoutKind, CutoutShape, Device,
    DeviceClass, DevicesFile, Edge, Laptop, LaptopBase, MIN_CHROME_SCALE, Rect, Screenshot, Size,
    Stand, StandFoot,
};
pub use galleries::{Basis, CornerRadius, GalleriesFile, GalleryProfile, Measured};
pub use store_locales::{LocaleSource, StoreLocale, StoreLocales, StoreLocalesFile, store_locales};
pub use targets::{
    Count, Platform, PresetSizes, PresetStatus, Requirement, SizePolicy, Source, SourceKind, Store,
    StoreRules, StoreSlots, TargetPreset, TargetsFile,
};

use crate::model::Orientation;

const DEVICES_YAML: &str = include_str!("../../data/devices.yaml");
const TARGETS_YAML: &str = include_str!("../../data/targets.yaml");
const GALLERIES_YAML: &str = include_str!("../../data/galleries.yaml");

/// Maximum relative deviation between a display and a screenshot aspect
/// ratio that still counts as a match (KONZEPT §4.3: about 2 %).
pub const ASPECT_TOLERANCE: f64 = 0.02;

/// All three catalogs.
#[derive(Debug, Clone)]
pub struct Catalog {
    pub devices: DevicesFile,
    pub targets: TargetsFile,
    pub galleries: GalleriesFile,
}

impl Catalog {
    /// Parse the three files (no cross checks; see [`Catalog::check`]).
    pub fn parse(devices: &str, targets: &str, galleries: &str) -> Result<Self, String> {
        Ok(Self {
            devices: crate::io::from_yaml(devices).map_err(|e| format!("devices.yaml: {e}"))?,
            targets: crate::io::from_yaml(targets).map_err(|e| format!("targets.yaml: {e}"))?,
            galleries: crate::io::from_yaml(galleries)
                .map_err(|e| format!("galleries.yaml: {e}"))?,
        })
    }

    pub fn device(&self, id: &str) -> Option<&Device> {
        self.devices.devices.iter().find(|d| d.id == id)
    }

    pub fn target_preset(&self, id: &str) -> Option<&TargetPreset> {
        self.targets.presets.iter().find(|p| p.id == id)
    }

    pub fn gallery(&self, id: &str) -> Option<&GalleryProfile> {
        self.galleries.galleries.iter().find(|g| g.id == id)
    }

    pub fn store_rules(&self, id: &str) -> Option<&StoreRules> {
        self.targets.rules.get(id)
    }

    /// All consistency problems of the catalog (empty = fine).
    pub fn check(&self) -> Vec<String> {
        let mut errors = Vec::new();
        galleries::check(&self.galleries, &mut errors);
        targets::check(self, &mut errors);
        devices::check(self, &mut errors);
        errors
    }
}

static CATALOG: LazyLock<Catalog> = LazyLock::new(|| {
    Catalog::parse(DEVICES_YAML, TARGETS_YAML, GALLERIES_YAML)
        .unwrap_or_else(|e| panic!("built-in catalog is invalid: {e}"))
});

/// The built-in catalog (parsed on first use).
pub fn catalog() -> &'static Catalog {
    &CATALOG
}

/// Device model by id (`iphone-island`).
pub fn device(id: &str) -> Option<&'static Device> {
    catalog().device(id)
}

/// All device models in catalog order.
pub fn devices() -> &'static [Device] {
    &catalog().devices.devices
}

/// Target preset by id (`ios-iphone-69`).
pub fn target_preset(id: &str) -> Option<&'static TargetPreset> {
    catalog().target_preset(id)
}

/// All target presets in catalog order.
pub fn target_presets() -> &'static [TargetPreset] {
    &catalog().targets.presets
}

/// Store rule set by id (`apple-screenshots`).
pub fn store_rules(id: &str) -> Option<&'static StoreRules> {
    catalog().store_rules(id)
}

/// Gallery profile by id (`appstore-iphone`).
pub fn gallery(id: &str) -> Option<&'static GalleryProfile> {
    catalog().gallery(id)
}

/// All gallery profiles in catalog order.
pub fn galleries() -> &'static [GalleryProfile] {
    &catalog().galleries.galleries
}

/// Cross-check the built-in catalog (empty = fine).
pub fn check() -> Vec<String> {
    catalog().check()
}

/// Do two aspect ratios (`w / h`) match within [`ASPECT_TOLERANCE`]?
pub fn aspect_matches(a: f64, b: f64) -> bool {
    (a / b - 1.0).abs() <= ASPECT_TOLERANCE
}

/// `w / h` of a size in the given orientation of a natural-portrait or
/// natural-landscape geometry (swapped when the orientations differ).
pub(crate) fn oriented_ratio(w: f64, h: f64, natural: Orientation, want: Orientation) -> f64 {
    if natural == want { w / h } else { h / w }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn built_in_catalog_parses_and_is_consistent() {
        let errors = check();
        assert!(errors.is_empty(), "catalog errors:\n{}", errors.join("\n"));
    }

    #[test]
    fn lookups() {
        let d = device("iphone-island").unwrap();
        assert_eq!(d.class, DeviceClass::Phone);
        assert_eq!(d.screenshot.variant.as_deref(), Some("ios-phone"));
        let p = target_preset("ios-iphone-69").unwrap();
        assert_eq!(
            p.slot
                .as_ref()
                .unwrap()
                .apple
                .as_ref()
                .unwrap()
                .display_type,
            "APP_IPHONE_67"
        );
        assert_eq!(
            p.sizes.portrait[0],
            crate::model::PixelSize::new(1320, 2868)
        );
        assert_eq!(p.gallery.as_deref(), Some("appstore-iphone"));
        assert!(gallery("appstore-iphone").is_some());
        assert!(store_rules("apple-screenshots").is_some());
        assert!(device("nope").is_none());
        assert!(target_preset("nope").is_none());
        assert!(gallery("nope").is_none());
    }

    #[test]
    fn concept_model_ids_exist() {
        for id in [
            "iphone-home",
            "iphone-notch",
            "iphone-island",
            "android-punchhole",
            "ipad-home",
            "ipad-modern",
            "android-tablet",
            "macbook",
            "generic-laptop",
            "browser",
            "imac",
            "generic-monitor",
            "none",
        ] {
            assert!(device(id).is_some(), "missing device `{id}`");
        }
        for id in [
            "appstore-iphone",
            "appstore-ipad",
            "appstore-mac",
            "play-phone",
            "play-tablet",
        ] {
            assert!(gallery(id).is_some(), "missing gallery `{id}`");
        }
    }

    #[test]
    fn placeholders_are_marked() {
        for g in galleries() {
            if g.measured == Measured::No {
                assert!(
                    g.note.as_deref().is_some_and(|n| !n.is_empty()),
                    "placeholder gallery `{}` needs a note",
                    g.id
                );
            }
        }
        assert_eq!(gallery("appstore-iphone").unwrap().measured, Measured::No);
        assert_eq!(
            gallery("appstore-iphone-web").unwrap().measured,
            Measured::WebCss
        );
        // The YAML marks every placeholder with the agreed comment.
        let marked = GALLERIES_YAML
            .lines()
            .filter(|l| l.contains("measured: false"))
            .count();
        let commented = GALLERIES_YAML
            .lines()
            .filter(|l| l.contains("measured: false  # NOT YET MEASURED"))
            .count();
        assert_eq!(marked, commented);
    }

    #[test]
    fn rules_accept_preset_sizes_and_reject_bad_images() {
        use crate::model::PixelSize;
        let ok = |rules: &StoreRules, w, h| rules.check_size(PixelSize::new(w, h)).is_empty();
        let rules = store_rules("google-screenshots").unwrap();
        assert!(ok(rules, 1080, 2160));
        assert!(!ok(rules, 1080, 2400));
        assert!(!ok(rules, 300, 600));
        assert!(!ok(rules, 2160, 4320));
        let large = store_rules("google-large-screens").unwrap();
        assert!(ok(large, 1920, 1080));
        assert!(!ok(large, 1600, 2560));
        let apple = target_preset("ios-iphone-69").unwrap();
        assert!(apple.has_size(PixelSize::new(2868, 1320)));
        assert!(!apple.has_size(PixelSize::new(1284, 2778)));
    }

    #[test]
    fn apple_presets_follow_app_store_connect() {
        // Listed on Apple's help page but rejected by the ASC uploader.
        use crate::model::PixelSize;
        let p69 = target_preset("ios-iphone-69").unwrap();
        assert!(!p69.has_size(PixelSize::new(1260, 2736)));
        assert_eq!(p69.requirement, Some(Requirement::Optional));
        assert_eq!(p69.status, PresetStatus::Verified);
        let p11 = target_preset("ios-ipad-11").unwrap();
        assert!(!p11.has_size(PixelSize::new(1512, 2316)));
        let p63 = target_preset("ios-iphone-63").unwrap();
        assert_eq!(p63.requirement, Some(Requirement::Required));
        assert_eq!(p63.status, PresetStatus::Verified);
        let duo = target_preset("ios-iphone-duo-outer").unwrap();
        assert_eq!(duo.status, PresetStatus::Unverified);
        assert!(catalog().targets.sources["ASC"].note.is_some());
    }

    #[test]
    fn check_reports_broken_data() {
        let devices = DEVICES_YAML
            .replace("x: 3.6, y: 3.6, w: 70.4", "x: 30.6, y: 3.6, w: 70.4")
            .replace("default_color: midnight", "default_color: plaid");
        let targets = TARGETS_YAML.replace("gallery: play-tablet", "gallery: nowhere");
        let galleries = GALLERIES_YAML.replace("id: play-web-mobile", "id: play-web");
        let c = Catalog::parse(&devices, &targets, &galleries).unwrap();
        let errors = c.check().join("\n");
        for needle in [
            "iphone-island: display is not inside the body",
            "default colour `plaid`",
            "unknown gallery `nowhere`",
            "duplicate gallery id `play-web`",
        ] {
            assert!(errors.contains(needle), "missing `{needle}` in:\n{errors}");
        }
    }

    #[test]
    fn gallery_gap_on_canvas() {
        use crate::model::PixelSize;
        let web = gallery("appstore-iphone-web").unwrap();
        let size = PixelSize::new(1290, 2796);
        assert!((web.gap_on_canvas(web.gap, size, 1) - 78.0).abs() < 1e-9);
        // Two pages per image: 1000 lu of the profile = 2000 page lu.
        assert!((web.gap_on_canvas(web.gap, PixelSize::new(2580, 2796), 2) - 156.0).abs() < 1e-9);
        // Height basis: 54 lu of the height of a 9:16 image.
        let play = gallery("play-web").unwrap();
        let g = play.gap_on_canvas(play.gap, PixelSize::new(1080, 1920), 1);
        assert!((g - 54.0 * 1920.0 / 1080.0).abs() < 1e-9);
    }

    #[test]
    fn device_outer_size_includes_base_and_stand() {
        let mb = device("macbook").unwrap();
        let s = mb.outer_size();
        assert!((s.w - 340.0).abs() < 1e-9);
        assert!((s.h - (196.75 + 1.6 + 9.0)).abs() < 1e-9);
        let phone = device("iphone-island").unwrap();
        assert!((phone.outer_size().h - 160.2).abs() < 1e-9);
        assert!((phone.display_ratio(Orientation::Landscape) - 153.0 / 70.4).abs() < 1e-9);
    }
}
