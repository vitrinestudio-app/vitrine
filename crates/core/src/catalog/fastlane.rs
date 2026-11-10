//! fastlane folder conventions (`data/fastlane.yaml`): what snapshot,
//! frameit and screengrab write and what deliver and supply read. Used by
//! the screenshot import (`core::import`) and the `fastlane` export layout
//! (`core::fastlane`).

use std::collections::BTreeMap;
use std::sync::LazyLock;

use serde::Deserialize;

use super::Source;

const FASTLANE_YAML: &str = include_str!("../../data/fastlane.yaml");

/// `fastlane.yaml`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FastlaneFile {
    /// Date the values were last checked (`YYYY-MM-DD`).
    pub checked: String,
    pub sources: BTreeMap<String, Source>,
    pub deliver: Deliver,
    pub supply: Supply,
    pub snapshot: Snapshot,
    pub screengrab: Screengrab,
    pub frameit: Frameit,
}

/// deliver (iOS upload).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Deliver {
    pub screenshots_dir: String,
    pub extensions: Vec<String>,
    pub max_per_set: usize,
    pub imessage_dir: String,
    pub special_dirs: Vec<String>,
    pub framed_suffix: String,
    pub display_types: Vec<DisplayTypeSizes>,
    pub imessage_types: Vec<String>,
    pub ambiguous: Vec<Ambiguous>,
    pub locales: Vec<String>,
    pub sources: Vec<String>,
}

/// One display type and the pixel sizes deliver maps to it.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DisplayTypeSizes {
    #[serde(rename = "type")]
    pub display_type: String,
    pub sizes: Vec<[u32; 2]>,
}

/// Sizes shared by several display types, told apart by the file path.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ambiguous {
    pub sizes: Vec<[u32; 2]>,
    pub default: String,
    /// Display type → alternative word lists; one list matches when the
    /// lowercased path contains all its words.
    pub markers: BTreeMap<String, Vec<Vec<String>>>,
}

/// supply (Android upload).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Supply {
    pub metadata_dir: String,
    pub images_dir: String,
    pub extensions: Vec<String>,
    /// Folder name → screenshot variant for the import (`None` = by size).
    pub screenshot_types: BTreeMap<String, Option<String>>,
    pub image_types: Vec<String>,
    pub docs_only_image_types: Vec<String>,
    pub max_per_type: usize,
    pub sources: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Snapshot {
    pub report: String,
    pub sources: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Screengrab {
    pub timestamp_min_digits: usize,
    pub sources: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Frameit {
    pub framed_suffix: String,
    pub config_files: Vec<String>,
    pub sources: Vec<String>,
}

impl Deliver {
    /// The display type deliver assigns to a `w × h` image at `path`
    /// (`iMessage/<locale>/…` gives the `IMESSAGE_` type); `None` = deliver
    /// rejects the size.
    pub fn display_type(&self, w: u32, h: u32, path: &str, imessage: bool) -> Option<String> {
        let size = [w, h];
        let lower = path.to_lowercase();
        let base = if let Some(a) = self.ambiguous.iter().find(|a| a.sizes.contains(&size)) {
            a.markers
                .iter()
                .find(|(_, lists)| {
                    lists
                        .iter()
                        .any(|words| words.iter().all(|w| lower.contains(&w.to_lowercase())))
                })
                .map_or_else(|| a.default.clone(), |(t, _)| t.clone())
        } else {
            self.display_types
                .iter()
                .find(|d| d.sizes.contains(&size))?
                .display_type
                .clone()
        };
        if imessage {
            self.imessage_types
                .contains(&base)
                .then(|| format!("IMESSAGE_{base}"))
        } else {
            Some(base)
        }
    }

    /// File-name marker that makes deliver pick `display_type` for an
    /// ambiguous size (`None` = no marker needed or none exists).
    pub fn marker_for(&self, w: u32, h: u32, display_type: &str) -> Option<&[String]> {
        let base = display_type
            .strip_prefix("IMESSAGE_")
            .unwrap_or(display_type);
        let a = self.ambiguous.iter().find(|a| a.sizes.contains(&[w, h]))?;
        if a.default == base {
            return None;
        }
        a.markers.get(base)?.first().map(Vec::as_slice)
    }

    /// deliver's spelling of a locale folder (case-insensitive), if valid.
    pub fn locale(&self, code: &str) -> Option<&str> {
        self.locales
            .iter()
            .find(|l| l.eq_ignore_ascii_case(code))
            .map(String::as_str)
    }

    /// Whether a display type is a screenshot set deliver knows.
    pub fn knows(&self, display_type: &str) -> bool {
        let base = display_type
            .strip_prefix("IMESSAGE_")
            .filter(|b| self.imessage_types.iter().any(|t| t == b));
        let base = base.unwrap_or(display_type);
        self.display_types.iter().any(|d| d.display_type == base)
    }
}

impl Supply {
    /// Case-insensitive match of a screenshot folder name.
    pub fn screenshot_type(&self, name: &str) -> Option<&str> {
        self.screenshot_types
            .keys()
            .find(|k| k.eq_ignore_ascii_case(name))
            .map(String::as_str)
    }

    /// Case-insensitive match of a single-image name (`featureGraphic`),
    /// including the ones only the docs list.
    pub fn image_type(&self, name: &str) -> Option<&str> {
        self.image_types
            .iter()
            .chain(&self.docs_only_image_types)
            .find(|k| k.eq_ignore_ascii_case(name))
            .map(String::as_str)
    }

    /// Import variant of a screenshot folder.
    pub fn variant(&self, screenshot_type: &str) -> Option<&str> {
        self.screenshot_types
            .get(screenshot_type)
            .and_then(Option::as_deref)
    }
}

impl FastlaneFile {
    pub fn parse(text: &str) -> Result<Self, String> {
        crate::io::from_yaml(text).map_err(|e| format!("fastlane.yaml: {e}"))
    }

    /// Consistency problems (empty = fine).
    pub fn check(&self) -> Vec<String> {
        let mut errors = Vec::new();
        let lists = [
            ("deliver", &self.deliver.sources),
            ("supply", &self.supply.sources),
            ("snapshot", &self.snapshot.sources),
            ("screengrab", &self.screengrab.sources),
            ("frameit", &self.frameit.sources),
        ];
        for (what, sources) in lists {
            if sources.is_empty() {
                errors.push(format!("{what}: no sources"));
            }
            for s in sources {
                if !self.sources.contains_key(s) {
                    errors.push(format!("{what}: unknown source `{s}`"));
                }
            }
        }
        let d = &self.deliver;
        for t in &d.imessage_types {
            if !d.display_types.iter().any(|x| &x.display_type == t) {
                errors.push(format!("deliver: imessage type `{t}` has no sizes"));
            }
        }
        for a in &d.ambiguous {
            for t in a.markers.keys().chain([&a.default]) {
                let Some(x) = d.display_types.iter().find(|x| &x.display_type == t) else {
                    errors.push(format!("deliver: ambiguous type `{t}` has no sizes"));
                    continue;
                };
                for s in &a.sizes {
                    if !x.sizes.contains(s) {
                        errors.push(format!("deliver: `{t}` lacks ambiguous size {s:?}"));
                    }
                }
            }
        }
        // Every size outside `ambiguous` must be unique.
        let mut seen: BTreeMap<[u32; 2], &str> = BTreeMap::new();
        for x in &d.display_types {
            for s in &x.sizes {
                if d.ambiguous.iter().any(|a| a.sizes.contains(s)) {
                    continue;
                }
                if let Some(prev) = seen.insert(*s, &x.display_type) {
                    errors.push(format!(
                        "deliver: size {s:?} in `{prev}` and `{}` but not ambiguous",
                        x.display_type
                    ));
                }
            }
        }
        errors
    }
}

static FASTLANE: LazyLock<FastlaneFile> = LazyLock::new(|| {
    FastlaneFile::parse(FASTLANE_YAML)
        .unwrap_or_else(|e| panic!("built-in fastlane conventions are invalid: {e}"))
});

/// The built-in fastlane conventions (parsed on first use).
pub fn fastlane() -> &'static FastlaneFile {
    &FASTLANE
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::store_locales;

    #[test]
    fn built_in_file_is_consistent() {
        let f = fastlane();
        assert_eq!(f.check(), Vec::<String>::new());
        // deliver's locale folders are exactly the App Store Connect codes.
        let mut ours: Vec<&str> = store_locales()
            .apple
            .locales
            .iter()
            .map(|l| l.code.as_str())
            .collect();
        let mut theirs: Vec<&str> = f.deliver.locales.iter().map(String::as_str).collect();
        ours.sort_unstable();
        theirs.sort_unstable();
        assert_eq!(ours, theirs);
    }

    #[test]
    fn deliver_display_types() {
        let d = &fastlane().deliver;
        let t = |w, h, p: &str| d.display_type(w, h, p, false);
        assert_eq!(t(1320, 2868, "x.png").as_deref(), Some("APP_IPHONE_67"));
        assert_eq!(t(2868, 1320, "x.png").as_deref(), Some("APP_IPHONE_67"));
        assert_eq!(t(1206, 2622, "x.png").as_deref(), Some("APP_IPHONE_61"));
        assert_eq!(
            t(2064, 2752, "x.png").as_deref(),
            Some("APP_IPAD_PRO_3GEN_129")
        );
        assert_eq!(
            t(2048, 2732, "x.png").as_deref(),
            Some("APP_IPAD_PRO_3GEN_129")
        );
        assert_eq!(
            t(2048, 2732, "de-DE/01_APP_IPAD_PRO_129.png").as_deref(),
            Some("APP_IPAD_PRO_129")
        );
        assert_eq!(
            t(2048, 2732, "iPad Pro (12.9-inch) (2nd generation)-01.png").as_deref(),
            Some("APP_IPAD_PRO_129")
        );
        assert_eq!(t(3840, 2160, "tv.png").as_deref(), Some("APP_APPLE_TV"));
        assert_eq!(
            t(3840, 2160, "vision-1.png").as_deref(),
            Some("APP_APPLE_VISION_PRO")
        );
        assert_eq!(t(2880, 1800, "x.png").as_deref(), Some("APP_DESKTOP"));
        assert_eq!(t(1080, 2160, "x.png"), None);
        assert_eq!(
            d.display_type(1320, 2868, "x.png", true).as_deref(),
            Some("IMESSAGE_APP_IPHONE_67")
        );
        assert_eq!(d.display_type(410, 502, "x.png", true), None);
        assert_eq!(
            d.marker_for(2732, 2048, "APP_IPAD_PRO_129"),
            Some(&["app_ipad_pro_129".to_owned()][..])
        );
        assert_eq!(d.marker_for(2732, 2048, "APP_IPAD_PRO_3GEN_129"), None);
        assert_eq!(d.locale("DE-de"), Some("de-DE"));
        assert!(d.knows("IMESSAGE_APP_IPHONE_67") && !d.knows("APP_IPHONE_99"));
    }

    #[test]
    fn supply_types() {
        let s = &fastlane().supply;
        assert_eq!(
            s.screenshot_type("phonescreenshots"),
            Some("phoneScreenshots")
        );
        assert_eq!(s.variant("tenInchScreenshots"), Some("android-tablet"));
        assert_eq!(s.variant("tvScreenshots"), None);
        assert_eq!(s.image_type("featuregraphic"), Some("featureGraphic"));
        assert_eq!(s.image_type("promoGraphic"), Some("promoGraphic"));
    }
}
