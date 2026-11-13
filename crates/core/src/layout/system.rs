//! Installed system fonts (ADR 0028): an index of the families in the
//! platform's font folders, built once per process and shared.
//!
//! The index only holds metadata (family, weight range, italic, file);
//! [`super::FontLibrary::add_system_family`] maps the files (fontique
//! memory-maps them) when a project names the family. Scanning uses
//! fontique's path loader (no platform font API, no C dependency), so the
//! same code runs on macOS, Windows and Linux.
//!
//! `VITRINE_SYSTEM_FONTS=off` (or `0`/`false`/`no`) disables system fonts
//! process-wide (CI, reproducibility checks); tests never use them.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};

use parley::fontique::{Blob, Collection, CollectionOptions, FontInfo, FontStyle, SourceKind};
use serde::Serialize;

/// One face of an installed family.
#[derive(Clone, Debug)]
pub struct SystemFace {
    pub path: PathBuf,
    /// Face index in the file (collections: `.ttc`).
    pub index: u32,
    pub weight: u16,
    pub weights: (u16, u16),
    pub italic: bool,
    info: FontInfo,
}

impl SystemFace {
    /// Maps the font file (shared by all faces of the file).
    pub fn load(&self) -> Option<Blob<u8>> {
        self.info.load(None)
    }
}

/// An installed family.
#[derive(Clone, Debug)]
pub struct SystemFamily {
    pub name: String,
    pub faces: Vec<SystemFace>,
}

impl SystemFamily {
    /// Weight range over all faces.
    pub fn weights(&self) -> (u16, u16) {
        (
            self.faces.iter().map(|f| f.weights.0).min().unwrap_or(400),
            self.faces.iter().map(|f| f.weights.1).max().unwrap_or(400),
        )
    }

    pub fn has_italic(&self) -> bool {
        self.faces.iter().any(|f| f.italic)
    }

    /// Distinct font files of the family.
    pub fn files(&self) -> Vec<&Path> {
        let mut out: Vec<&Path> = self.faces.iter().map(|f| f.path.as_path()).collect();
        out.sort_unstable();
        out.dedup();
        out
    }

    /// The face used for coverage checks (regular upright if any).
    pub fn default_face(&self) -> Option<&SystemFace> {
        self.faces
            .iter()
            .min_by_key(|f| (f.italic, (i32::from(f.weight) - 400).abs()))
    }
}

/// Summary of an installed family (pickers, `vitrine fonts --system`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct SystemFamilyInfo {
    pub family: String,
    pub weights: [u16; 2],
    pub italic: bool,
}

/// The installed families, by lower-cased name.
#[derive(Debug, Default)]
pub struct SystemFontIndex {
    families: BTreeMap<String, SystemFamily>,
    dirs: Vec<PathBuf>,
}

impl SystemFontIndex {
    /// An index of the fonts in `dirs` (recursively). Hidden families
    /// (`.` prefix, macOS UI fonts) and `LastResort` are left out.
    pub fn scan(dirs: &[PathBuf]) -> Self {
        let mut collection = Collection::new(CollectionOptions {
            shared: false,
            system_fonts: false,
        });
        collection.load_fonts_from_paths(dirs.iter().filter(|d| d.is_dir()));
        let names: Vec<String> = collection
            .family_names()
            .filter(|n| !n.starts_with('.') && *n != "LastResort" && !n.trim().is_empty())
            .map(str::to_owned)
            .collect();
        let mut families = BTreeMap::new();
        for name in names {
            let Some(fam) = collection.family_by_name(&name) else {
                continue;
            };
            let faces: Vec<SystemFace> = fam
                .fonts()
                .iter()
                .filter_map(|f| {
                    let SourceKind::Path(p) = f.source().kind() else {
                        return None;
                    };
                    let weight = weight_value(f.weight().value());
                    let weights = f
                        .axes()
                        .iter()
                        .find(|a| a.tag.to_be_bytes() == *b"wght")
                        .map_or((weight, weight), |a| {
                            (weight_value(a.min), weight_value(a.max))
                        });
                    Some(SystemFace {
                        path: p.to_path_buf(),
                        index: f.index(),
                        weight,
                        weights,
                        italic: !matches!(f.style(), FontStyle::Normal),
                        info: f.clone(),
                    })
                })
                .collect();
            if !faces.is_empty() {
                families.insert(name.to_lowercase(), SystemFamily { name, faces });
            }
        }
        Self {
            families,
            dirs: dirs.to_vec(),
        }
    }

    /// The folders this index was built from.
    pub fn dirs(&self) -> &[PathBuf] {
        &self.dirs
    }

    /// The installed family `name` (case-insensitive).
    pub fn family(&self, name: &str) -> Option<&SystemFamily> {
        self.families.get(&name.to_lowercase())
    }

    pub fn families(&self) -> impl Iterator<Item = &SystemFamily> {
        self.families.values()
    }

    pub fn len(&self) -> usize {
        self.families.len()
    }

    pub fn is_empty(&self) -> bool {
        self.families.is_empty()
    }

    /// Sorted family summaries.
    pub fn infos(&self) -> Vec<SystemFamilyInfo> {
        self.families
            .values()
            .map(|f| {
                let (lo, hi) = f.weights();
                SystemFamilyInfo {
                    family: f.name.clone(),
                    weights: [lo, hi],
                    italic: f.has_italic(),
                }
            })
            .collect()
    }
}

fn weight_value(w: f32) -> u16 {
    w.round().clamp(1.0, 1000.0) as u16
}

/// Are system fonts allowed in this process (`VITRINE_SYSTEM_FONTS`)?
pub fn allowed() -> bool {
    std::env::var("VITRINE_SYSTEM_FONTS").map_or(true, |v| {
        !matches!(
            v.trim().to_ascii_lowercase().as_str(),
            "0" | "off" | "false" | "no"
        )
    })
}

static INDEX: Mutex<Option<Arc<SystemFontIndex>>> = Mutex::new(None);

/// The process-wide index of the platform's font folders (scanned on first
/// call; [`refresh`] rescans). Empty when system fonts are not
/// [`allowed`].
pub fn index() -> Arc<SystemFontIndex> {
    if !allowed() {
        return Arc::new(SystemFontIndex::default());
    }
    let mut slot = INDEX.lock().unwrap_or_else(PoisonError::into_inner);
    slot.get_or_insert_with(|| Arc::new(SystemFontIndex::scan(&super::fonts::system_font_dirs())))
        .clone()
}

/// Rescans the font folders (fonts installed while the app runs).
pub fn refresh() -> Arc<SystemFontIndex> {
    *INDEX.lock().unwrap_or_else(PoisonError::into_inner) = None;
    index()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scans_a_font_folder() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("Inter-Regular.ttf"),
            include_bytes!("../../../../assets/fonts/inter/Inter-Regular.ttf"),
        )
        .unwrap();
        let sub = dir.path().join("lora");
        std::fs::create_dir(&sub).unwrap();
        std::fs::write(
            sub.join("Lora.ttf"),
            include_bytes!("../../../../assets/fonts/lora/Lora[wght].ttf"),
        )
        .unwrap();
        let ix = SystemFontIndex::scan(&[dir.path().to_owned()]);
        let names: Vec<&str> = ix.families().map(|f| f.name.as_str()).collect();
        assert_eq!(names, ["Inter", "Lora"]);
        let lora = ix.family("lora").unwrap();
        assert_eq!(lora.weights(), (400, 700));
        assert!(!lora.has_italic());
        assert_eq!(lora.files().len(), 1);
        assert!(lora.default_face().unwrap().load().is_some());
        assert_eq!(
            ix.infos()[0],
            SystemFamilyInfo {
                family: "Inter".into(),
                weights: [400, 400],
                italic: false
            }
        );
        assert!(SystemFontIndex::scan(&[dir.path().join("missing")]).is_empty());
    }
}
