//! The incremental-export manifest `<export>/.vitrine-export.json`:
//! path → entry with the input hash and what the file is (target,
//! language, kind, image, pixel size, layout, store locale), so the
//! overview page lists every current file and stale files of a re-export
//! can be removed.
//!
//! Older exports wrote `path → "hash"`; such entries are read with target
//! and language taken from the path (`<target>/<lang>/<file>`) and are
//! replaced by full entries the next time the file is exported.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::{ExportLayout, FileKind};

/// One file of the manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManifestEntry {
    /// Input hash (16 hex digits).
    pub hash: String,
    #[serde(default)]
    pub target: String,
    #[serde(default)]
    pub lang: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<FileKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<u32>,
    /// Pixel size (0 = unknown: legacy entry).
    #[serde(default)]
    pub width: u32,
    #[serde(default)]
    pub height: u32,
    #[serde(default)]
    pub layout: ExportLayout,
    /// Store locale folder (fastlane layout).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub locale: Option<String>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Raw {
    Hash(String),
    Entry(ManifestEntry),
}

/// Entry of a legacy `path → hash` manifest, described from its path.
fn legacy(path: &str, hash: String) -> ManifestEntry {
    let parts: Vec<&str> = path.split('/').collect();
    let (target, lang) = match parts.as_slice() {
        [t, l, _] => ((*t).to_owned(), (*l).to_owned()),
        _ => (String::new(), String::new()),
    };
    let name = parts.last().copied().unwrap_or_default();
    let kind = if name == "_gallery.png" {
        FileKind::Gallery
    } else if name.starts_with("banner.") {
        FileKind::Banner
    } else {
        FileKind::Page
    };
    let digits: String = name.chars().take_while(char::is_ascii_digit).collect();
    let image = digits
        .parse::<u32>()
        .ok()
        .and_then(|n| n.checked_sub(1))
        .or((kind == FileKind::Banner).then_some(0));
    ManifestEntry {
        hash,
        target,
        lang,
        kind: Some(kind),
        image,
        width: 0,
        height: 0,
        layout: ExportLayout::Standard,
        locale: None,
    }
}

/// Reads the manifest (missing or unreadable = empty).
pub fn read(path: &Path) -> BTreeMap<String, ManifestEntry> {
    let Some(raw) = std::fs::read(path)
        .ok()
        .and_then(|b| serde_json::from_slice::<BTreeMap<String, Raw>>(&b).ok())
    else {
        return BTreeMap::new();
    };
    raw.into_iter()
        .map(|(p, r)| {
            let e = match r {
                Raw::Hash(h) => legacy(&p, h),
                Raw::Entry(e) => e,
            };
            (p, e)
        })
        .collect()
}

/// Writes the manifest.
pub fn write(path: &Path, manifest: &BTreeMap<String, ManifestEntry>) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(
        path,
        serde_json::to_vec_pretty(manifest).unwrap_or_default(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_legacy_and_current_entries() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("m.json");
        std::fs::write(
            &path,
            r#"{
  "ios/en/02_plan.png": "00000000000000ab",
  "ios/en/_gallery.png": "00000000000000ac",
  "blog/de/banner.jpg": "00000000000000ad",
  "screenshots/en-US/01_ios.png": {"hash": "00000000000000ae", "target": "ios", "lang": "en",
    "kind": "page", "image": 0, "width": 10, "height": 20, "layout": "fastlane", "locale": "en-US"}
}"#,
        )
        .unwrap();
        let m = read(&path);
        let e = &m["ios/en/02_plan.png"];
        assert_eq!(
            (e.target.as_str(), e.lang.as_str(), e.kind, e.image),
            ("ios", "en", Some(FileKind::Page), Some(1))
        );
        assert_eq!(m["ios/en/_gallery.png"].kind, Some(FileKind::Gallery));
        assert_eq!(m["blog/de/banner.jpg"].image, Some(0));
        let f = &m["screenshots/en-US/01_ios.png"];
        assert_eq!(f.layout, ExportLayout::Fastlane);
        assert_eq!(f.locale.as_deref(), Some("en-US"));
        write(&path, &m).unwrap();
        assert_eq!(read(&path), m);
        assert!(read(&tmp.path().join("missing.json")).is_empty());
    }
}
