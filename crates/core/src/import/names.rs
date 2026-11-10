//! Name helpers of the screenshot import: slugs, numeric prefixes, file
//! name tokens, fastlane `<Device Name>-<screen>` splitting, variant
//! detection by pixel size.

use crate::catalog;
use crate::model::PixelSize;

/// Slug of a file-name stem: lowercase ASCII letters, digits, `-`, `_`;
/// every other run of characters becomes one `-`. Empty → `file`.
///
/// ```
/// use arrangia_core::import::slugify;
/// assert_eq!(slugify("Home Screen (1)"), "home-screen-1");
/// assert_eq!(slugify("01_home"), "01_home");
/// ```
pub fn slugify(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for c in name.chars() {
        let c = c.to_ascii_lowercase();
        if c.is_ascii_alphanumeric() || c == '_' {
            out.push(c);
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    let out = out.trim_matches(['-', '_']).to_owned();
    if out.is_empty() {
        "file".to_owned()
    } else {
        out
    }
}

/// `01_home` → `home`, `3-settings` → `settings`, `home` → `home`. A name
/// that is only digits is kept.
pub fn strip_numeric_prefix(name: &str) -> &str {
    let digits = name.len() - name.trim_start_matches(|c: char| c.is_ascii_digit()).len();
    if digits == 0 {
        return name;
    }
    let rest = &name[digits..];
    let trimmed = rest.trim_start_matches(['_', '-', ' ', '.']);
    if trimmed.len() == rest.len() || trimmed.is_empty() {
        // `2fa` or `01`: not a numbering prefix.
        return name;
    }
    trimmed
}

/// Comparable form of a key: lowercase ASCII alphanumerics only
/// (`garden_plan` ≙ `garden-plan` ≙ `GardenPlan`).
pub(crate) fn normalized(name: &str) -> String {
    name.chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

/// Byte positions of the `-` in `stem` that may separate a fastlane device
/// name from the screen name: outside parentheses, both sides non-empty,
/// and not right after a purely numeric segment (`…-01-home` splits
/// before `01`, not after it).
fn split_points(stem: &str) -> Vec<usize> {
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut seg_start = 0;
    for (i, c) in stem.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => depth -= 1,
            '-' if depth <= 0 => {
                let seg = &stem[seg_start..i];
                let numeric = !seg.is_empty() && seg.chars().all(|c| c.is_ascii_digit());
                if i > 0 && i + 1 < stem.len() && !stem[..i].trim().is_empty() && !numeric {
                    out.push(i);
                }
                seg_start = i + 1;
            }
            _ => {}
        }
    }
    out
}

/// How many file names start with each possible device-name prefix; lets
/// [`split_fastlane`] keep hyphens that all files of a device share
/// (`iPad Pro 13-inch (M4)-01_home`, `iPad Pro 13-inch (M4)-02_plan`).
#[derive(Clone, Debug, Default)]
pub struct DevicePrefixes(std::collections::BTreeMap<String, usize>);

impl DevicePrefixes {
    pub fn new<'a>(stems: impl IntoIterator<Item = &'a str>) -> Self {
        let mut map = std::collections::BTreeMap::new();
        for stem in stems {
            for i in split_points(stem) {
                *map.entry(stem[..i].to_owned()).or_insert(0) += 1;
            }
        }
        Self(map)
    }

    fn count(&self, prefix: &str) -> usize {
        self.0.get(prefix).copied().unwrap_or(0)
    }
}

/// Splits a fastlane snapshot stem `<Device Name>-<screen>`. Device names
/// in `known` (longest first) win. Otherwise the split is the longest
/// candidate prefix (see [`DevicePrefixes`]) shared by as many files as the
/// shortest one, so `iPad Pro 13-inch (M4)-01_home` next to
/// `iPad Pro 13-inch (M4)-02_plan` gives the device `iPad Pro 13-inch (M4)`;
/// hyphens inside parentheses never split.
pub fn split_fastlane<'a>(
    stem: &'a str,
    known: &[&str],
    prefixes: &DevicePrefixes,
) -> Option<(&'a str, &'a str)> {
    let mut known: Vec<&str> = known.to_vec();
    known.sort_by_key(|k| std::cmp::Reverse(k.len()));
    for k in known {
        if let Some(rest) = stem.strip_prefix(k)
            && let Some(screen) = rest.strip_prefix('-')
            && !screen.is_empty()
        {
            return Some((&stem[..k.len()], screen));
        }
    }
    let points = split_points(stem);
    let first = *points.first()?;
    let base = prefixes.count(&stem[..first]);
    let pick = points
        .iter()
        .rev()
        .copied()
        .find(|&i| prefixes.count(&stem[..i]) >= base)
        .unwrap_or(first);
    Some((&stem[..pick], &stem[pick + 1..]))
}

/// Does `stem` look like a fastlane device-prefixed name (a device name
/// with a space or parenthesis before the separating `-`)?
pub(crate) fn looks_like_fastlane(stem: &str) -> bool {
    split_fastlane(stem, &[], &DevicePrefixes::default())
        .is_some_and(|(d, _)| d.contains(' ') || d.ends_with(')'))
}

/// Screenshot variants (device catalog) whose typical sizes include
/// `w × h` in either orientation, in catalog order.
pub fn variants_for_size(w: u32, h: u32) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for d in catalog::devices() {
        let Some(v) = &d.screenshot.variant else {
            continue;
        };
        let hit = d
            .screenshot
            .sizes
            .iter()
            .any(|s| *s == PixelSize::new(w, h) || *s == PixelSize::new(h, w));
        if hit && !out.contains(v) {
            out.push(v.clone());
        }
    }
    out
}

/// Screenshot variants whose device display aspect matches `w × h` (either
/// orientation) within the catalog tolerance, in catalog order.
pub fn variants_for_aspect(width: u32, height: u32) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    if width == 0 || height == 0 {
        return out;
    }
    let ratio = f64::from(width) / f64::from(height);
    for d in catalog::devices() {
        let (Some(v), Some(a)) = (&d.screenshot.variant, d.screenshot.aspect) else {
            continue;
        };
        let ar = f64::from(a.w) / f64::from(a.h);
        if (catalog::aspect_matches(ratio, ar) || catalog::aspect_matches(ratio, 1.0 / ar))
            && !out.contains(v)
        {
            out.push(v.clone());
        }
    }
    out
}

/// All screenshot variant ids of the device catalog.
pub fn catalog_variants() -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for d in catalog::devices() {
        if let Some(v) = &d.screenshot.variant
            && !out.contains(v)
        {
            out.push(v.clone());
        }
    }
    out
}

/// Tokens of a file name: runs of characters between `-`, `_`, `.`,
/// spaces and parentheses.
pub(crate) fn tokens(stem: &str) -> Vec<String> {
    stem.split(['-', '_', '.', ' ', '(', ')', '+', ','])
        .filter(|t| !t.is_empty())
        .map(str::to_owned)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugs() {
        assert_eq!(slugify("Home Screen (1)"), "home-screen-1");
        assert_eq!(slugify("Über_Uns"), "ber_uns");
        assert_eq!(slugify("  "), "file");
        assert_eq!(slugify("plan--v2"), "plan-v2");
    }

    #[test]
    fn numeric_prefixes() {
        assert_eq!(strip_numeric_prefix("01_home"), "home");
        assert_eq!(strip_numeric_prefix("3-settings"), "settings");
        assert_eq!(strip_numeric_prefix("12 . map"), "map");
        assert_eq!(strip_numeric_prefix("home"), "home");
        assert_eq!(strip_numeric_prefix("2fa"), "2fa");
        assert_eq!(strip_numeric_prefix("01"), "01");
        assert_eq!(strip_numeric_prefix("01_"), "01_");
    }

    #[test]
    fn splits_fastlane_names() {
        let none = DevicePrefixes::default();
        assert_eq!(
            split_fastlane("iPhone 16 Pro Max-01_home", &[], &none),
            Some(("iPhone 16 Pro Max", "01_home"))
        );
        assert_eq!(
            split_fastlane("iPad Pro (12.9-inch) (6th generation)-02-plan", &[], &none),
            Some(("iPad Pro (12.9-inch) (6th generation)", "02-plan"))
        );
        // A hyphen outside parentheses, shared by all files of the device.
        let stems = [
            "iPad Pro 13-inch (M4)-01_home",
            "iPad Pro 13-inch (M4)-02_plan",
            "iPhone 16-01-home",
            "iPhone 16-02-plan-b",
        ];
        let prefixes = DevicePrefixes::new(stems);
        assert_eq!(
            split_fastlane(stems[0], &[], &prefixes),
            Some(("iPad Pro 13-inch (M4)", "01_home"))
        );
        assert_eq!(
            split_fastlane(stems[3], &[], &prefixes),
            Some(("iPhone 16", "02-plan-b"))
        );
        // A known device name wins.
        assert_eq!(
            split_fastlane("Pixel-7 Pro-home", &["Pixel-7 Pro"], &none),
            Some(("Pixel-7 Pro", "home"))
        );
        assert_eq!(split_fastlane("home", &[], &none), None);
        assert!(looks_like_fastlane("iPhone 16-home"));
        assert!(!looks_like_fastlane("garden-plan"));
    }

    #[test]
    fn detects_variants() {
        let v = variants_for_size(1320, 2868);
        assert_eq!(v.first().map(String::as_str), Some("ios-phone"), "{v:?}");
        assert_eq!(variants_for_size(2868, 1320), v);
        assert_eq!(variants_for_size(17, 23), Vec::<String>::new());
        // 3:4 tablet screenshot that is no catalog size: aspect match.
        assert_eq!(variants_for_size(1548, 2064), Vec::<String>::new());
        assert!(variants_for_aspect(1548, 2064).contains(&"ios-tablet".to_owned()));
        assert!(catalog_variants().contains(&"android-phone".to_owned()));
    }

    #[test]
    fn tokenizes() {
        assert_eq!(
            tokens("de-DE_ios-phone 01_home"),
            ["de", "DE", "ios", "phone", "01", "home"]
        );
    }
}
