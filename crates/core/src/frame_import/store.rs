//! The app-wide frame store:
//!
//! ```text
//! <frames root>/                      (app data folder, never a project)
//!   <source>/                         apple
//!     index.yaml                      installed frames (FrameSourceIndex)
//!     user-mapping.yaml               manual file assignments (UserMapping)
//!     <model>/<colour>/<orientation>.png        the vendor bezel, unchanged
//!     <model>/<colour>/<orientation>.mask.png   detected screen mask
//! ```

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::analyze::PxRect;
use crate::io::{from_yaml, to_yaml, write_atomic};
use crate::model::{Orientation, PixelSize};

pub const INDEX_FILE: &str = "index.yaml";
pub const USER_MAPPING_FILE: &str = "user-mapping.yaml";

/// One installed frame.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InstalledFrame {
    pub model: String,
    /// Colour id (`black-titanium`).
    pub color: String,
    /// Colour as the vendor writes it (`Black Titanium`).
    pub color_label: String,
    pub orientation: Orientation,
    /// Bezel PNG, relative to the source folder.
    pub file: String,
    /// Mask PNG, relative to the source folder.
    pub mask: String,
    /// Pixel size of the bezel.
    pub size: PixelSize,
    /// Detected screen rectangle (px).
    pub screen: PxRect,
    /// Estimated screen corner radius (px).
    pub radius: f64,
    /// Original file name in the package.
    pub source_file: String,
    /// Package it came from (file name of the DMG / ZIP / folder).
    pub package: String,
    /// Import date (`YYYY-MM-DD`, UTC).
    pub imported: String,
}

/// `<source>/index.yaml`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FrameSourceIndex {
    #[serde(default)]
    pub frames: Vec<InstalledFrame>,
}

/// A manual assignment of a file name (GUI "unmatched files").
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Assignment {
    pub model: String,
    /// Colour as written by the user (slugified on import).
    pub color: String,
    pub orientation: Orientation,
}

/// `<source>/user-mapping.yaml`: normalised file name → assignment
/// (`None` = ignore the file).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UserMapping {
    #[serde(default)]
    pub files: BTreeMap<String, Option<Assignment>>,
}

/// All installed frames below a frames root, read-only (what resolve and
/// render use).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FrameIndex {
    root: PathBuf,
    sources: BTreeMap<String, FrameSourceIndex>,
}

impl FrameIndex {
    /// Reads every `<root>/<source>/index.yaml`; a missing root is empty,
    /// unreadable indexes are skipped.
    pub fn load(root: &Path) -> FrameIndex {
        let mut sources = BTreeMap::new();
        if let Ok(rd) = fs::read_dir(root) {
            for e in rd.flatten() {
                let name = e.file_name().to_string_lossy().into_owned();
                let path = e.path().join(INDEX_FILE);
                if let Ok(text) = fs::read_to_string(&path)
                    && let Ok(index) = from_yaml::<FrameSourceIndex>(&text)
                {
                    sources.insert(name, index);
                }
            }
        }
        FrameIndex {
            root: root.to_owned(),
            sources,
        }
    }

    /// An index from memory (tests).
    pub fn from_sources(root: &Path, sources: BTreeMap<String, FrameSourceIndex>) -> FrameIndex {
        FrameIndex {
            root: root.to_owned(),
            sources,
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn is_empty(&self) -> bool {
        self.sources.values().all(|s| s.frames.is_empty())
    }

    /// Source ids with at least one frame.
    pub fn sources(&self) -> impl Iterator<Item = &str> {
        self.sources
            .iter()
            .filter(|(_, s)| !s.frames.is_empty())
            .map(|(k, _)| k.as_str())
    }

    pub fn frames(&self, source: &str) -> &[InstalledFrame] {
        self.sources.get(source).map_or(&[], |s| &s.frames)
    }

    /// The frame for `model` × `color` × `orientation`. Without a colour:
    /// `default_color` if installed, else the first installed colour.
    pub fn find(
        &self,
        source: &str,
        model: &str,
        color: Option<&str>,
        default_color: Option<&str>,
        orientation: Orientation,
    ) -> Option<&InstalledFrame> {
        let mut candidates = self
            .frames(source)
            .iter()
            .filter(|f| f.model == model && f.orientation == orientation);
        if let Some(c) = color {
            return candidates.find(|f| f.color == c);
        }
        let all: Vec<_> = candidates.collect();
        default_color
            .and_then(|d| all.iter().find(|f| f.color == d).copied())
            .or_else(|| all.first().copied())
    }

    /// Installed colours of `model` (any orientation), sorted, unique.
    pub fn colors(&self, source: &str, model: &str) -> Vec<(String, String)> {
        let mut out: Vec<(String, String)> = self
            .frames(source)
            .iter()
            .filter(|f| f.model == model)
            .map(|f| (f.color.clone(), f.color_label.clone()))
            .collect();
        out.sort();
        out.dedup_by(|a, b| a.0 == b.0);
        out
    }

    /// Path of a frame file relative to the frames root
    /// (`apple/iphone-island/black/portrait.png`).
    pub fn rel_path(source: &str, file: &str) -> String {
        format!("{source}/{file}")
    }
}

/// The writable store below a frames root.
#[derive(Debug, Clone)]
pub struct FrameStore {
    root: PathBuf,
}

fn err(path: &Path) -> impl FnOnce(std::io::Error) -> String + '_ {
    move |e| format!("{}: {e}", path.display())
}

impl FrameStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn source_dir(&self, source: &str) -> PathBuf {
        self.root.join(source)
    }

    pub fn index(&self) -> FrameIndex {
        FrameIndex::load(&self.root)
    }

    pub fn source_index(&self, source: &str) -> Result<FrameSourceIndex, String> {
        let path = self.source_dir(source).join(INDEX_FILE);
        match fs::read_to_string(&path) {
            Ok(t) => from_yaml(&t).map_err(|e| format!("{}: {e}", path.display())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(FrameSourceIndex::default()),
            Err(e) => Err(format!("{}: {e}", path.display())),
        }
    }

    fn save_index(&self, source: &str, index: &FrameSourceIndex) -> Result<(), String> {
        let dir = self.source_dir(source);
        fs::create_dir_all(&dir).map_err(err(&dir))?;
        let path = dir.join(INDEX_FILE);
        let yaml = to_yaml(index).map_err(|e| e.to_string())?;
        write_atomic(&path, yaml.as_bytes()).map_err(|e| e.to_string())
    }

    pub fn user_mapping(&self, source: &str) -> Result<UserMapping, String> {
        let path = self.source_dir(source).join(USER_MAPPING_FILE);
        match fs::read_to_string(&path) {
            Ok(t) => from_yaml(&t).map_err(|e| format!("{}: {e}", path.display())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(UserMapping::default()),
            Err(e) => Err(format!("{}: {e}", path.display())),
        }
    }

    pub fn save_user_mapping(&self, source: &str, mapping: &UserMapping) -> Result<(), String> {
        let dir = self.source_dir(source);
        fs::create_dir_all(&dir).map_err(err(&dir))?;
        let yaml = to_yaml(mapping).map_err(|e| e.to_string())?;
        write_atomic(&dir.join(USER_MAPPING_FILE), yaml.as_bytes()).map_err(|e| e.to_string())
    }

    /// Writes one frame (bezel bytes unchanged + mask) and records it,
    /// replacing an entry with the same model × colour × orientation.
    pub fn install(
        &self,
        source: &str,
        mut frame: InstalledFrame,
        bezel: &[u8],
        mask_png: &[u8],
    ) -> Result<InstalledFrame, String> {
        let o = match frame.orientation {
            Orientation::Portrait => "portrait",
            Orientation::Landscape => "landscape",
        };
        let rel_dir = format!("{}/{}", frame.model, frame.color);
        let dir = self
            .source_dir(source)
            .join(&frame.model)
            .join(&frame.color);
        fs::create_dir_all(&dir).map_err(err(&dir))?;
        write_atomic(&dir.join(format!("{o}.png")), bezel).map_err(|e| e.to_string())?;
        write_atomic(&dir.join(format!("{o}.mask.png")), mask_png).map_err(|e| e.to_string())?;
        frame.file = format!("{rel_dir}/{o}.png");
        frame.mask = format!("{rel_dir}/{o}.mask.png");
        let mut index = self.source_index(source)?;
        index.frames.retain(|f| {
            !(f.model == frame.model
                && f.color == frame.color
                && f.orientation == frame.orientation)
        });
        index.frames.push(frame.clone());
        index.frames.sort_by(|a, b| {
            (&a.model, &a.color, a.orientation as u8).cmp(&(
                &b.model,
                &b.color,
                b.orientation as u8,
            ))
        });
        self.save_index(source, &index)?;
        Ok(frame)
    }

    /// Removes frames of `source` matching the filters (`None` = any);
    /// returns how many were removed. Removing everything deletes the
    /// source folder (the user mapping is kept).
    pub fn remove(
        &self,
        source: &str,
        model: Option<&str>,
        color: Option<&str>,
        orientation: Option<Orientation>,
    ) -> Result<usize, String> {
        let mut index = self.source_index(source)?;
        let dir = self.source_dir(source);
        let (gone, keep): (Vec<_>, Vec<_>) = index.frames.into_iter().partition(|f| {
            model.is_none_or(|m| f.model == m)
                && color.is_none_or(|c| f.color == c)
                && orientation.is_none_or(|o| f.orientation == o)
        });
        for f in &gone {
            for rel in [&f.file, &f.mask] {
                let p = rel.split('/').fold(dir.clone(), |p, c| p.join(c));
                let _ = fs::remove_file(&p);
            }
            // Drop emptied colour / model folders.
            let colour_dir = dir.join(&f.model).join(&f.color);
            let _ = fs::remove_dir(&colour_dir);
            let _ = fs::remove_dir(dir.join(&f.model));
        }
        index.frames = keep;
        self.save_index(source, &index)?;
        Ok(gone.len())
    }
}

/// Today's date (UTC) as `YYYY-MM-DD`.
pub fn today() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let days = i64::try_from(secs / 86_400).unwrap_or(0);
    // Civil-from-days (H. Hinnant).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}
