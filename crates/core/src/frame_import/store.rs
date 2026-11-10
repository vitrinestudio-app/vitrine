//! The app-wide frame store:
//!
//! ```text
//! <frames root>/                      (app data folder, never a project)
//!   <source>/                         apple
//!     index.yaml                      installed frames (FrameSourceIndex)
//!     user-mapping.yaml               manual file assignments (UserMapping)
//!     <model>/<colour>/<orientation>/<vendor-model>.png       the vendor bezel, unchanged
//!     <model>/<colour>/<orientation>/<vendor-model>.mask.png  detected screen mask
//! ```
//!
//! One catalog model × colour × orientation may hold several vendor models
//! (`iPhone 18 Pro` and `iPhone 18 Pro Max` are both `iphone-island`):
//! resolve picks one per target ([`super::select_bezel`]). Stores written
//! before that (`<model>/<colour>/<orientation>.png`, no `apple_model`)
//! are read as they are and moved to this layout by [`FrameStore::migrate`].

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::analyze::PxRect;
use super::rules::{Rules, normalize_name, slug, vendor_model};
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
    /// The vendor's model name (`iPhone 18 Pro`), from the file name.
    #[serde(default)]
    pub apple_model: String,
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
    /// Rule priority (tie-break of the automatic choice).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub priority: i32,
    /// Original file name in the package.
    pub source_file: String,
    /// Package it came from (file name of the DMG / ZIP / folder).
    pub package: String,
    /// The package's licence file (`License.rtf`): the frames are subject
    /// to it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,
    /// Import date (`YYYY-MM-DD`, UTC).
    pub imported: String,
    /// Housing colour `#RRGGBB` measured from the bezel at import (the
    /// inspector's swatch); filled in later for older stores
    /// ([`FrameStore::fill_swatches`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub swatch: Option<String>,
}

#[allow(
    clippy::trivially_copy_pass_by_ref,
    reason = "serde skip_serializing_if"
)]
fn is_zero(v: &i32) -> bool {
    *v == 0
}

impl InstalledFrame {
    /// Same catalog model × colour × orientation × vendor model.
    pub fn same_slot(&self, other: &InstalledFrame) -> bool {
        self.model == other.model
            && self.color == other.color
            && self.orientation == other.orientation
            && slug(&self.apple_model) == slug(&other.apple_model)
    }

    /// Does `name` (as written in `bezel:`) name this vendor model?
    /// Case-insensitive; `iphone-18-pro` works too.
    pub fn is_bezel(&self, name: &str) -> bool {
        self.apple_model.eq_ignore_ascii_case(name.trim()) || slug(&self.apple_model) == slug(name)
    }

    /// Fills `apple_model` of entries written before it existed (from the
    /// source file name and the rules of `source`).
    fn fill_legacy(&mut self, source: &str) {
        if !self.apple_model.is_empty() {
            return;
        }
        let name = normalize_name(&self.source_file);
        self.apple_model = Rules::builtin(source)
            .and_then(|r| r.match_file(&self.source_file))
            .map_or_else(|| vendor_model(&name, &self.color_label), |m| m.apple_model);
    }

    /// Screen area in px².
    pub fn screen_area(&self) -> u64 {
        u64::from(self.screen.w) * u64::from(self.screen.h)
    }
}

/// An installed colour of one model ([`FrameIndex::installed_colors`]).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct InstalledColor {
    /// Colour id (`burgundy`).
    pub id: String,
    /// As the vendor writes it (`Burgundy`).
    pub label: String,
    /// Measured housing colour `#RRGGBB` (see [`super::analyze`]).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub swatch: Option<String>,
}

/// `<source>/index.yaml`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FrameSourceIndex {
    #[serde(default)]
    pub frames: Vec<InstalledFrame>,
}

impl FrameSourceIndex {
    fn fill_legacy(&mut self, source: &str) {
        for f in &mut self.frames {
            f.fill_legacy(source);
        }
    }

    fn sort(&mut self) {
        self.frames.sort_by(|a, b| {
            (&a.model, &a.color, a.orientation as u8, &a.apple_model).cmp(&(
                &b.model,
                &b.color,
                b.orientation as u8,
                &b.apple_model,
            ))
        });
    }
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
                    && let Ok(mut index) = from_yaml::<FrameSourceIndex>(&text)
                {
                    index.fill_legacy(&name);
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

    /// All installed bezels (vendor models) for `model` × `color` ×
    /// `orientation`, in index order. A colour that is not installed (or
    /// none) gives `default_color` if installed, else the first installed
    /// colour ([`Self::effective_color`]).
    pub fn candidates(
        &self,
        source: &str,
        model: &str,
        color: Option<&str>,
        default_color: Option<&str>,
        orientation: Orientation,
    ) -> Vec<&InstalledFrame> {
        let all: Vec<_> = self
            .frames(source)
            .iter()
            .filter(|f| f.model == model && f.orientation == orientation)
            .collect();
        let has = |c: &str| all.iter().any(|f| f.color == c);
        let color = color
            .filter(|c| has(c))
            .or_else(|| default_color.filter(|d| has(d)))
            .or_else(|| all.first().map(|f| f.color.as_str()));
        let Some(color) = color else {
            return Vec::new();
        };
        all.into_iter().filter(|f| f.color == color).collect()
    }

    /// The largest installed bezel for `model` × `color` × `orientation`
    /// (see [`Self::candidates`]; resolve uses [`super::select_bezel`]).
    pub fn find(
        &self,
        source: &str,
        model: &str,
        color: Option<&str>,
        default_color: Option<&str>,
        orientation: Orientation,
    ) -> Option<&InstalledFrame> {
        let c = self.candidates(source, model, color, default_color, orientation);
        super::select_bezel(&c, None, None, None).map(|(f, _)| f)
    }

    /// Installed colours of `model` (any orientation), sorted, unique:
    /// `(id, label)`.
    pub fn colors(&self, source: &str, model: &str) -> Vec<(String, String)> {
        self.installed_colors(source, model)
            .into_iter()
            .map(|c| (c.id, c.label))
            .collect()
    }

    /// Installed colours of `model` with their measured swatch (portrait
    /// bezels preferred), sorted by id.
    pub fn installed_colors(&self, source: &str, model: &str) -> Vec<InstalledColor> {
        let mut out: Vec<InstalledColor> = Vec::new();
        for f in self.frames(source).iter().filter(|f| f.model == model) {
            match out.iter_mut().find(|c| c.id == f.color) {
                Some(c) => {
                    if c.swatch.is_none() || f.orientation == Orientation::Portrait {
                        c.swatch = f.swatch.clone().or(c.swatch.take());
                    }
                }
                None => out.push(InstalledColor {
                    id: f.color.clone(),
                    label: f.color_label.clone(),
                    swatch: f.swatch.clone(),
                }),
            }
        }
        out.sort_by(|a, b| a.id.cmp(&b.id));
        out
    }

    /// Path of a frame file relative to the frames root
    /// (`apple/iphone-island/black/portrait/iphone-18-pro.png`).
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
            Ok(t) => from_yaml::<FrameSourceIndex>(&t)
                .map(|mut ix| {
                    ix.fill_legacy(source);
                    ix
                })
                .map_err(|e| format!("{}: {e}", path.display())),
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

    /// Relative folder and file stem of a frame in the current layout
    /// (`iphone-island/black/portrait`, `iphone-18-pro`).
    fn layout_of(frame: &InstalledFrame) -> (String, String) {
        let o = super::orientation_name(frame.orientation);
        let stem = match slug(&frame.apple_model) {
            s if s.is_empty() => "bezel".to_owned(),
            s => s,
        };
        (format!("{}/{}/{o}", frame.model, frame.color), stem)
    }

    fn abs(&self, source: &str, rel: &str) -> PathBuf {
        rel.split('/')
            .fold(self.source_dir(source), |p, c| p.join(c))
    }

    /// Writes one frame (bezel bytes unchanged + mask) and records it,
    /// replacing an entry with the same model × colour × orientation ×
    /// vendor model.
    pub fn install(
        &self,
        source: &str,
        mut frame: InstalledFrame,
        bezel: &[u8],
        mask_png: &[u8],
    ) -> Result<InstalledFrame, String> {
        let mut index = self.migrate(source)?;
        let (rel_dir, stem) = Self::layout_of(&frame);
        let dir = self.abs(source, &rel_dir);
        fs::create_dir_all(&dir).map_err(err(&dir))?;
        write_atomic(&dir.join(format!("{stem}.png")), bezel).map_err(|e| e.to_string())?;
        write_atomic(&dir.join(format!("{stem}.mask.png")), mask_png).map_err(|e| e.to_string())?;
        frame.file = format!("{rel_dir}/{stem}.png");
        frame.mask = format!("{rel_dir}/{stem}.mask.png");
        index.frames.retain(|f| !f.same_slot(&frame));
        index.frames.push(frame.clone());
        index.sort();
        self.save_index(source, &index)?;
        Ok(frame)
    }

    /// Moves frames of an older store layout
    /// (`<model>/<colour>/<orientation>.png`, no `apple_model`) to the
    /// current one and rewrites the index; returns the (migrated) index.
    /// Missing files are dropped from the index. A store in the current
    /// layout is returned unchanged.
    pub fn migrate(&self, source: &str) -> Result<FrameSourceIndex, String> {
        let mut index = self.source_index(source)?;
        let mut changed = false;
        let mut kept: Vec<InstalledFrame> = Vec::with_capacity(index.frames.len());
        for mut f in std::mem::take(&mut index.frames) {
            let (rel_dir, stem) = Self::layout_of(&f);
            let (file, mask) = (
                format!("{rel_dir}/{stem}.png"),
                format!("{rel_dir}/{stem}.mask.png"),
            );
            if f.file != file || f.mask != mask {
                let (old_file, old_mask) = (self.abs(source, &f.file), self.abs(source, &f.mask));
                if !old_file.is_file() {
                    changed = true;
                    continue;
                }
                let dir = self.abs(source, &rel_dir);
                fs::create_dir_all(&dir).map_err(err(&dir))?;
                let new_file = self.abs(source, &file);
                fs::rename(&old_file, &new_file).map_err(err(&old_file))?;
                if old_mask.is_file() {
                    let new_mask = self.abs(source, &mask);
                    fs::rename(&old_mask, &new_mask).map_err(err(&old_mask))?;
                }
                f.file = file;
                f.mask = mask;
                changed = true;
            }
            // Two legacy entries can never share a slot, but be safe.
            if let Some(i) = kept.iter().position(|k| k.same_slot(&f)) {
                kept.remove(i);
                changed = true;
            }
            kept.push(f);
        }
        index.frames = kept;
        if changed {
            index.sort();
            self.save_index(source, &index)?;
        }
        Ok(index)
    }

    /// Measures the housing colour of frames recorded without one (stores
    /// imported before swatches existed) and saves the index; returns how
    /// many were filled. Unreadable files are left without a swatch.
    pub fn fill_swatches(&self, source: &str) -> Result<usize, String> {
        let mut index = self.source_index(source)?;
        let mut filled = 0;
        for f in index.frames.iter_mut().filter(|f| f.swatch.is_none()) {
            let Ok(bytes) = fs::read(self.abs(source, &f.file)) else {
                continue;
            };
            if let Ok(img) = super::analyze::decode_png(&bytes) {
                let screen = f.screen;
                if screen.x + screen.w <= img.width && screen.y + screen.h <= img.height {
                    f.swatch = Some(super::analyze::hex_rgb(super::analyze::body_swatch(
                        &img, screen,
                    )));
                    filled += 1;
                }
            }
        }
        if filled > 0 {
            self.save_index(source, &index)?;
        }
        Ok(filled)
    }

    /// [`Self::fill_swatches`] for every source folder of the store.
    pub fn fill_all_swatches(&self) -> usize {
        let Ok(rd) = fs::read_dir(&self.root) else {
            return 0;
        };
        rd.flatten()
            .filter(|e| e.path().join(INDEX_FILE).is_file())
            .filter_map(|e| self.fill_swatches(&e.file_name().to_string_lossy()).ok())
            .sum()
    }

    /// Removes frames of `source` matching the filters (`None` = any;
    /// `bezel` = vendor model, see [`InstalledFrame::is_bezel`]); returns
    /// how many were removed. Emptied folders are deleted (the user
    /// mapping is kept).
    pub fn remove(&self, source: &str, filter: &RemoveFilter<'_>) -> Result<usize, String> {
        let mut index = self.migrate(source)?;
        let (gone, keep): (Vec<_>, Vec<_>) = index.frames.into_iter().partition(|f| {
            filter.model.is_none_or(|m| f.model == m)
                && filter.color.is_none_or(|c| f.color == c)
                && filter.orientation.is_none_or(|o| f.orientation == o)
                && filter.bezel.is_none_or(|b| f.is_bezel(b))
        });
        for f in &gone {
            for rel in [&f.file, &f.mask] {
                let _ = fs::remove_file(self.abs(source, rel));
            }
            // Drop emptied orientation / colour / model folders.
            let mut dir = self.abs(source, &f.file);
            for _ in 0..3 {
                if !dir.pop() || fs::remove_dir(&dir).is_err() {
                    break;
                }
            }
        }
        index.frames = keep;
        self.save_index(source, &index)?;
        Ok(gone.len())
    }
}

/// Which frames [`FrameStore::remove`] removes (`None` = any).
#[derive(Debug, Clone, Copy, Default)]
pub struct RemoveFilter<'a> {
    pub model: Option<&'a str>,
    pub color: Option<&'a str>,
    pub orientation: Option<Orientation>,
    pub bezel: Option<&'a str>,
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
