//! Sample screenshots (owner decision 2026-11-12, ADR 0023 amendment): a
//! template may ship the author's own dummy app screenshots (CC0) so users
//! can experiment. When a project is created from such a template (or
//! canvases are inserted), the files are copied into `screenshots/` and
//! listed in `screenshots/samples.yaml` with their size and SHA-256:
//!
//! ```yaml
//! template: travel-bold
//! files:
//!   screenshots/en/home.png: {size: 183424, sha256: 3f0c…}
//! ```
//!
//! A listed file is a sample only while its content is unchanged, so
//! replacing it with an own screenshot un-marks it automatically. Samples
//! give the validation info `sample_screenshot`, and the store upload plan
//! refuses sets that contain them (export works). Deleting the marker file
//! turns the remaining samples into ordinary screenshots.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};
use std::time::SystemTime;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// The marker file, relative to the project folder.
pub const SAMPLES_FILE: &str = "screenshots/samples.yaml";

const HEADER: &str = "# Sample screenshots copied from a template (Vitrine Studio).\n\
# Replace them with screenshots of your app: a file whose content changed is no\n\
# longer a sample. Store uploads refuse sets that still contain samples.\n";

/// One sample file.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SampleFile {
    pub size: u64,
    /// Lowercase hex SHA-256 of the content.
    pub sha256: String,
}

/// `screenshots/samples.yaml`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SampleMarks {
    /// Template the samples came from (informational).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub template: String,
    /// Project-relative path (`screenshots/en/home.png`) → content.
    #[serde(default)]
    pub files: BTreeMap<String, SampleFile>,
}

/// Lowercase hex SHA-256.
pub fn sha256_hex(data: &[u8]) -> String {
    use std::fmt::Write as _;
    Sha256::digest(data)
        .iter()
        .fold(String::with_capacity(64), |mut s, b| {
            let _ = write!(s, "{b:02x}");
            s
        })
}

impl SampleMarks {
    /// Reads the marker of the project in `root` (empty when missing or
    /// unreadable).
    pub fn load(root: &Path) -> Self {
        std::fs::read_to_string(root.join(SAMPLES_FILE))
            .ok()
            .and_then(|t| crate::io::from_yaml(&t).ok())
            .unwrap_or_default()
    }

    /// Writes the marker (removes it when no file is listed).
    pub fn save(&self, root: &Path) -> Result<(), crate::io::IoError> {
        let path = root.join(SAMPLES_FILE);
        if self.files.is_empty() {
            if path.exists() {
                std::fs::remove_file(&path).map_err(|e| crate::io::IoError::Io {
                    path: path.clone(),
                    source: e,
                })?;
            }
            return Ok(());
        }
        let text = format!("{HEADER}{}", crate::io::to_yaml(self)?);
        crate::io::write_atomic(&path, text.as_bytes())
    }

    /// Lists `path` (project-relative) with this content.
    pub fn add(&mut self, path: &str, data: &[u8]) {
        self.files.insert(
            path.to_owned(),
            SampleFile {
                size: data.len() as u64,
                sha256: sha256_hex(data),
            },
        );
    }

    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    /// Is the file at `path` (project-relative) still the listed sample?
    pub fn is_sample(&self, root: &Path, path: &str) -> bool {
        let Some(entry) = self.files.get(path) else {
            return false;
        };
        file_matches(&root.join(path), entry)
    }

    /// The listed files that are still unchanged samples.
    pub fn current(&self, root: &Path) -> BTreeSet<String> {
        self.files
            .keys()
            .filter(|p| self.is_sample(root, p))
            .cloned()
            .collect()
    }
}

/// Hash results per file state, so validation does not re-read unchanged
/// files: (path, size, mtime) → matches.
type HashCache = HashMap<(PathBuf, u64, Option<SystemTime>, String), bool>;
static CACHE: LazyLock<Mutex<HashCache>> = LazyLock::new(|| Mutex::new(HashMap::new()));

fn file_matches(path: &Path, entry: &SampleFile) -> bool {
    let Ok(meta) = std::fs::metadata(path) else {
        return false;
    };
    if !meta.is_file() || meta.len() != entry.size {
        return false;
    }
    let key = (
        path.to_path_buf(),
        meta.len(),
        meta.modified().ok(),
        entry.sha256.clone(),
    );
    if let Some(hit) = CACHE.lock().ok().and_then(|c| c.get(&key).copied()) {
        return hit;
    }
    let matches = std::fs::read(path).is_ok_and(|d| sha256_hex(&d) == entry.sha256);
    if let Ok(mut c) = CACHE.lock() {
        if c.len() > 4096 {
            c.clear();
        }
        c.insert(key, matches);
    }
    matches
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_replaced_file_is_no_longer_a_sample() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join("screenshots/en")).unwrap();
        std::fs::write(root.join("screenshots/en/home.png"), b"sample").unwrap();
        // Same size, other content.
        std::fs::write(root.join("screenshots/en/list.png"), b"sampl3").unwrap();
        let mut marks = SampleMarks {
            template: "t".into(),
            ..SampleMarks::default()
        };
        marks.add("screenshots/en/home.png", b"sample");
        marks.add("screenshots/en/list.png", b"sample");
        marks.save(root).unwrap();
        let text = std::fs::read_to_string(root.join(SAMPLES_FILE)).unwrap();
        assert!(text.starts_with("# Sample screenshots"), "{text}");
        let marks = SampleMarks::load(root);
        assert!(marks.is_sample(root, "screenshots/en/home.png"));
        assert!(!marks.is_sample(root, "screenshots/en/list.png"));
        assert!(!marks.is_sample(root, "screenshots/en/none.png"));
        // Replaced: no longer a sample.
        std::fs::write(root.join("screenshots/en/home.png"), b"my screenshot").unwrap();
        assert!(!marks.is_sample(root, "screenshots/en/home.png"));
        assert!(marks.current(root).is_empty());
        SampleMarks::default().save(root).unwrap();
        assert!(!root.join(SAMPLES_FILE).exists());
    }
}
