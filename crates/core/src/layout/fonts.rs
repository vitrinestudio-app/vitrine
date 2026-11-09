//! The font library: bundled OFL fonts plus fonts from a project's `fonts/`
//! folder. System fonts are never used, so layouts are identical on every
//! machine (golden tests, `docs/adr/0003-text-markup-and-layout.md`).

use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use parley::fontique::{Blob, Collection, CollectionOptions, FontStyle};
use serde::{Deserialize, Serialize};

/// Family used when a requested family is missing (always bundled).
pub const DEFAULT_FAMILY: &str = "Inter";

/// Bundled fonts (`assets/fonts/`, Inter 4.1, OFL), embedded in the binary.
const BUNDLED: &[(&str, &[u8])] = &[
    (
        "Inter-Regular.ttf",
        include_bytes!("../../../../assets/fonts/Inter-Regular.ttf"),
    ),
    (
        "Inter-SemiBold.ttf",
        include_bytes!("../../../../assets/fonts/Inter-SemiBold.ttf"),
    ),
    (
        "InterDisplay-Bold.ttf",
        include_bytes!("../../../../assets/fonts/InterDisplay-Bold.ttf"),
    ),
    (
        "InterDisplay-Black.ttf",
        include_bytes!("../../../../assets/fonts/InterDisplay-Black.ttf"),
    ),
];

/// Index of a face in a [`FontLibrary`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct FontId(pub u32);

/// Where a face comes from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FontSource {
    /// Embedded bundled font (file name in `assets/fonts/`).
    Bundled(&'static str),
    /// Loaded from a file (project `fonts/` folder).
    File(PathBuf),
    /// Added from memory.
    Memory,
}

/// One face (a font file may contain several, e.g. `.ttc`).
#[derive(Clone)]
pub struct FontFace {
    pub family: String,
    pub weight: u16,
    pub italic: bool,
    /// Face index inside the font file.
    pub index: u32,
    pub source: FontSource,
    pub(crate) blob: Blob<u8>,
}

impl FontFace {
    /// Raw font file data (all faces of the file).
    pub fn data(&self) -> &[u8] {
        self.blob.data()
    }
}

impl fmt::Debug for FontFace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FontFace")
            .field("family", &self.family)
            .field("weight", &self.weight)
            .field("italic", &self.italic)
            .field("index", &self.index)
            .field("source", &self.source)
            .field("bytes", &self.blob.data().len())
            .finish()
    }
}

/// Error while adding fonts.
#[derive(Debug, thiserror::Error)]
pub enum FontError {
    #[error("cannot read font `{path}`: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("`{0}` is not a usable font file")]
    Invalid(String),
}

/// Result of [`FontLibrary::load_dir`].
#[derive(Debug, Default)]
pub struct FontDirReport {
    pub added: Vec<FontId>,
    /// Files that could not be read or parsed; the rest is still loaded.
    pub errors: Vec<FontError>,
}

/// The fonts available to layout and rendering.
///
/// Cheap to clone (font data is shared). Faces are matched by family name
/// (case-insensitive), weight and italic like CSS; a per-family fallback
/// list ([`FontLibrary::set_fallbacks`]) is tried for missing glyphs, then
/// [`DEFAULT_FAMILY`].
#[derive(Clone, Debug)]
pub struct FontLibrary {
    faces: Vec<FontFace>,
    fallbacks: BTreeMap<String, Vec<String>>,
}

impl Default for FontLibrary {
    fn default() -> Self {
        Self::bundled()
    }
}

impl FontLibrary {
    /// A library without any fonts.
    pub fn empty() -> Self {
        Self {
            faces: Vec::new(),
            fallbacks: BTreeMap::new(),
        }
    }

    /// The bundled fonts: Inter (400, 600) and Inter Display (700, 900).
    pub fn bundled() -> Self {
        let mut lib = Self::empty();
        for (name, data) in BUNDLED {
            lib.add_blob(&Blob::new(Arc::new(*data)), &FontSource::Bundled(name))
                .expect("bundled fonts are valid");
        }
        lib
    }

    /// Adds all faces of a font file held in memory.
    pub fn add_font(
        &mut self,
        data: Vec<u8>,
        source: &FontSource,
    ) -> Result<Vec<FontId>, FontError> {
        self.add_blob(&Blob::new(Arc::new(data)), source)
    }

    fn add_blob(&mut self, blob: &Blob<u8>, source: &FontSource) -> Result<Vec<FontId>, FontError> {
        // Let fontique read the metadata so matching here and in layout agree.
        let mut scratch = Collection::new(CollectionOptions {
            shared: false,
            system_fonts: false,
        });
        let mut added = Vec::new();
        for (family_id, infos) in scratch.register_fonts(blob.clone(), None) {
            let family = scratch
                .family_name(family_id)
                .unwrap_or_default()
                .to_owned();
            for info in infos {
                let id = FontId(u32::try_from(self.faces.len()).unwrap_or(u32::MAX));
                self.faces.push(FontFace {
                    family: family.clone(),
                    weight: info.weight().value().round().clamp(1.0, 1000.0) as u16,
                    italic: !matches!(info.style(), FontStyle::Normal),
                    index: info.index(),
                    source: source.clone(),
                    blob: blob.clone(),
                });
                added.push(id);
            }
        }
        if added.is_empty() {
            let name = match source {
                FontSource::Bundled(n) => (*n).to_owned(),
                FontSource::File(p) => p.display().to_string(),
                FontSource::Memory => "<memory>".to_owned(),
            };
            return Err(FontError::Invalid(name));
        }
        Ok(added)
    }

    /// Loads a font file (`.ttf`, `.otf`, `.ttc`, `.otc`).
    pub fn load_file(&mut self, path: &Path) -> Result<Vec<FontId>, FontError> {
        let data = std::fs::read(path).map_err(|source| FontError::Io {
            path: path.to_owned(),
            source,
        })?;
        self.add_font(data, &FontSource::File(path.to_owned()))
    }

    /// Loads every font file directly inside `dir` (a project's `fonts/`
    /// folder), in file name order. A missing folder is not an error.
    pub fn load_dir(&mut self, dir: &Path) -> FontDirReport {
        let mut report = FontDirReport::default();
        let entries = match std::fs::read_dir(dir) {
            Ok(e) => e,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return report,
            Err(source) => {
                report.errors.push(FontError::Io {
                    path: dir.to_owned(),
                    source,
                });
                return report;
            }
        };
        let mut files: Vec<PathBuf> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                p.is_file()
                    && p.extension().and_then(|x| x.to_str()).is_some_and(|x| {
                        ["ttf", "otf", "ttc", "otc"].contains(&x.to_ascii_lowercase().as_str())
                    })
            })
            .collect();
        files.sort();
        for f in files {
            match self.load_file(&f) {
                Ok(ids) => report.added.extend(ids),
                Err(e) => report.errors.push(e),
            }
        }
        report
    }

    pub fn faces(&self) -> &[FontFace] {
        &self.faces
    }

    pub fn face(&self, id: FontId) -> Option<&FontFace> {
        self.faces.get(id.0 as usize)
    }

    /// Distinct family names, sorted.
    pub fn families(&self) -> Vec<&str> {
        let mut f: Vec<&str> = self.faces.iter().map(|f| f.family.as_str()).collect();
        f.sort_unstable();
        f.dedup();
        f
    }

    pub fn has_family(&self, family: &str) -> bool {
        self.faces
            .iter()
            .any(|f| f.family.eq_ignore_ascii_case(family))
    }

    /// Sets the fallback families tried (in order) for characters missing
    /// in `family` — e.g. a CJK font for a Latin headline font (§5.1).
    pub fn set_fallbacks(&mut self, family: &str, fallbacks: Vec<String>) {
        self.fallbacks.insert(family.to_owned(), fallbacks);
    }

    pub fn fallbacks(&self, family: &str) -> &[String] {
        self.fallbacks.get(family).map_or(&[], Vec::as_slice)
    }

    /// The family list used for layout: `family`, its fallbacks, then
    /// [`DEFAULT_FAMILY`].
    pub fn family_chain(&self, family: &str) -> Vec<String> {
        let mut out = vec![family.to_owned()];
        for f in self.fallbacks(family) {
            if !out.contains(f) {
                out.push(f.clone());
            }
        }
        if !out.iter().any(|f| f.eq_ignore_ascii_case(DEFAULT_FAMILY)) {
            out.push(DEFAULT_FAMILY.to_owned());
        }
        out
    }

    /// Best face of `family` for `weight`/`italic`, following CSS font
    /// matching: italic first, then weight (desired 400–500: up to 500,
    /// then lighter, then heavier; below 400: lighter first; above 500:
    /// heavier first).
    pub fn match_face(&self, family: &str, weight: u16, italic: bool) -> Option<FontId> {
        let d = i32::from(weight);
        let rank = |fw: i32| -> (u8, i32) {
            if fw == d {
                (0, 0)
            } else if (400..=500).contains(&d) {
                if fw > d && fw <= 500 {
                    (1, fw - d)
                } else if fw < d {
                    (2, d - fw)
                } else {
                    (3, fw - d)
                }
            } else if (d < 400) == (fw < d) {
                (1, (fw - d).abs())
            } else {
                (2, (fw - d).abs())
            }
        };
        self.faces
            .iter()
            .enumerate()
            .filter(|(_, f)| f.family.eq_ignore_ascii_case(family))
            .min_by_key(|(_, f)| (f.italic != italic, rank(i32::from(f.weight))))
            .map(|(i, _)| FontId(i as u32))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_faces_have_metadata() {
        let lib = FontLibrary::bundled();
        assert_eq!(lib.families(), ["Inter", "Inter Display"]);
        let weights: Vec<_> = lib.faces().iter().map(|f| (f.weight, f.italic)).collect();
        assert_eq!(
            weights,
            [(400, false), (600, false), (700, false), (900, false)]
        );
        assert!(lib.has_family("inter display"));
    }

    #[test]
    fn css_like_matching() {
        let lib = FontLibrary::bundled();
        let w = |fam: &str, wt: u16| {
            lib.face(lib.match_face(fam, wt, false).unwrap())
                .unwrap()
                .weight
        };
        assert_eq!(w("Inter", 400), 400);
        assert_eq!(w("Inter", 500), 400);
        assert_eq!(w("Inter", 800), 600);
        assert_eq!(w("Inter Display", 800), 900);
        assert_eq!(w("Inter Display", 600), 700);
        assert_eq!(w("Inter Display", 300), 700);
        assert_eq!(w("Inter Display", 850), 900);
        assert_eq!(lib.match_face("Comic Sans", 400, false), None);
    }

    #[test]
    fn fallback_chain() {
        let mut lib = FontLibrary::bundled();
        assert_eq!(lib.family_chain("Inter"), ["Inter"]);
        lib.set_fallbacks("Brand", vec!["Noto Sans JP".into()]);
        assert_eq!(
            lib.family_chain("Brand"),
            ["Brand", "Noto Sans JP", "Inter"]
        );
    }

    #[test]
    fn project_font_folder() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("b.ttf"),
            include_bytes!("../../../../assets/fonts/Inter-Regular.ttf"),
        )
        .unwrap();
        std::fs::write(dir.path().join("a.otf"), b"not a font").unwrap();
        std::fs::write(dir.path().join("readme.txt"), b"ignored").unwrap();
        let mut lib = FontLibrary::empty();
        let report = lib.load_dir(dir.path());
        assert_eq!(report.added, [FontId(0)]);
        assert_eq!(report.errors.len(), 1);
        assert!(matches!(lib.faces()[0].source, FontSource::File(_)));
        assert!(lib.load_dir(&dir.path().join("missing")).errors.is_empty());
    }
}
