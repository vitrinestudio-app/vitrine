//! The font library: bundled OFL fonts, fonts from a project's `fonts/`
//! folder and – only where the caller asks for it – installed system
//! fonts (ADR 0028). Layout and rendering only see the library, so tests
//! and golden images (bundled fonts only) are identical on every machine
//! (`docs/adr/0003-text-markup-and-layout.md`).
//!
//! Bundled faces come from an embedded manifest ([`super::bundled`]) and
//! are loaded lazily: the library knows their family, weights and style
//! without reading the file; the layouter registers a file only when a
//! text uses its family.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use parley::fontique::{Blob, Collection, CollectionOptions, FontStyle};
use serde::{Deserialize, Serialize};

use super::bundled;
use super::system::{SystemFamily, SystemFontIndex};

/// Family used when a requested family is missing (always bundled).
pub const DEFAULT_FAMILY: &str = "Inter";

/// Index of a face in a [`FontLibrary`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct FontId(pub u32);

/// Where a face comes from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FontSource {
    /// Embedded bundled font (path below `assets/fonts/`).
    Bundled(&'static str),
    /// Loaded from a file (project `fonts/` folder).
    File(PathBuf),
    /// An installed system font (named by the project or loaded as a
    /// fallback for missing characters; never in tests).
    System(PathBuf),
    /// Added from memory.
    Memory,
}

/// One face (a font file may contain several, e.g. `.ttc`).
#[derive(Clone)]
pub struct FontFace {
    pub family: String,
    /// Weight of the default instance.
    pub weight: u16,
    /// Weight range: the `wght` axis of a variable font, else
    /// `(weight, weight)`.
    pub weights: (u16, u16),
    pub italic: bool,
    /// Face index inside the font file.
    pub index: u32,
    pub source: FontSource,
    data: FaceData,
}

/// The bytes behind a face: a loaded file, or a bundled file (embedded,
/// its blob created on first use, see [`super::bundled::blob`]).
#[derive(Clone)]
enum FaceData {
    Blob(Blob<u8>),
    Bundled(usize),
}

impl FontFace {
    /// Raw font file data (all faces of the file).
    pub fn data(&self) -> &[u8] {
        match &self.data {
            FaceData::Blob(b) => b.data(),
            FaceData::Bundled(i) => bundled::FILES[*i].data(),
        }
    }

    /// The font file as a fontique blob (bundled files: the process-wide
    /// blob, created on first call).
    pub(crate) fn blob(&self) -> Blob<u8> {
        match &self.data {
            FaceData::Blob(b) => b.clone(),
            FaceData::Bundled(i) => bundled::blob(*i),
        }
    }

    /// Is this a bundled face (registered lazily by the layouter)?
    pub fn is_bundled(&self) -> bool {
        matches!(self.data, FaceData::Bundled(_))
    }
}

impl fmt::Debug for FontFace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FontFace")
            .field("family", &self.family)
            .field("weight", &self.weight)
            .field("weights", &self.weights)
            .field("italic", &self.italic)
            .field("index", &self.index)
            .field("source", &self.source)
            .field("bytes", &self.data().len())
            .finish_non_exhaustive()
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
    /// Families loaded from system fonts as last resort for characters no
    /// other font has (never in tests); appended to every family chain.
    system_fallbacks: Vec<String>,
    /// Like `system_fallbacks`, but only for texts of one language
    /// (lower-case tag): the CJK family of the language (Han glyph shapes
    /// differ between Japanese and Chinese).
    lang_fallbacks: BTreeMap<String, Vec<String>>,
    /// Installed system families loaded because the project names them
    /// ([`Self::add_system_family`]).
    system_named: Vec<String>,
    /// Families from system fonts stored in the project (`fonts/system/`,
    /// [`super::stored`]).
    stored: Vec<String>,
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
            system_fallbacks: Vec::new(),
            lang_fallbacks: BTreeMap::new(),
            system_named: Vec::new(),
            stored: Vec::new(),
        }
    }

    /// The bundled fonts ([`bundled::FILES`]): Inter (400, 600), Inter
    /// Display (700, 900) and the families added 2026-11-11 (variable
    /// files, see [`bundled::FAMILIES`]). Built from the embedded manifest:
    /// no font file is read here.
    pub fn bundled() -> Self {
        let mut lib = Self::empty();
        lib.faces = bundled::FILES
            .iter()
            .enumerate()
            .map(|(i, f)| FontFace {
                family: f.family.to_owned(),
                weight: f.weight,
                weights: f.weights,
                italic: f.italic,
                index: 0,
                source: FontSource::Bundled(f.file),
                data: FaceData::Bundled(i),
            })
            .collect();
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
                let weight = weight_value(info.weight().value());
                let weights = info
                    .axes()
                    .iter()
                    .find(|a| a.tag.to_be_bytes() == *b"wght")
                    .map_or((weight, weight), |a| {
                        (weight_value(a.min), weight_value(a.max))
                    });
                self.faces.push(FontFace {
                    family: family.clone(),
                    weight,
                    weights,
                    italic: !matches!(info.style(), FontStyle::Normal),
                    index: info.index(),
                    source: source.clone(),
                    data: FaceData::Blob(blob.clone()),
                });
                added.push(id);
            }
        }
        if added.is_empty() {
            let name = match source {
                FontSource::Bundled(n) => (*n).to_owned(),
                FontSource::File(p) | FontSource::System(p) => p.display().to_string(),
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

    /// Families added as last-resort fallback for every language
    /// ([`Self::add_system_fallbacks`]).
    pub fn system_fallbacks(&self) -> &[String] {
        &self.system_fallbacks
    }

    /// System families added as fallback for the texts of one language
    /// (lower-case tag → families).
    pub fn lang_fallbacks(&self) -> &BTreeMap<String, Vec<String>> {
        &self.lang_fallbacks
    }

    /// Records that `family` comes from the project's stored system fonts.
    pub fn mark_stored(&mut self, family: &str) {
        if !self.stored.iter().any(|f| f.eq_ignore_ascii_case(family)) {
            self.stored.push(family.to_owned());
        }
    }

    /// Families from the project's stored system fonts (`fonts/system/`).
    pub fn stored_families(&self) -> &[String] {
        &self.stored
    }

    /// Appends `family` to the fallback chains of language `lang` (`None`:
    /// of every language), like a system fallback (stored copies).
    pub fn add_chain_fallback(&mut self, lang: Option<&str>, family: &str) {
        let list = match lang {
            Some(l) => self.lang_fallbacks.entry(l.to_lowercase()).or_default(),
            None => &mut self.system_fallbacks,
        };
        if !list.iter().any(|f| f.eq_ignore_ascii_case(family)) {
            list.push(family.to_owned());
        }
    }

    /// Installed system families loaded because the project names them
    /// ([`Self::add_system_family`]).
    pub fn system_families(&self) -> &[String] {
        &self.system_named
    }

    /// Is every face of `family` an installed system font (none bundled or
    /// from the project)?
    pub fn is_system_family(&self, family: &str) -> bool {
        let mut faces = self
            .faces
            .iter()
            .filter(|f| f.family.eq_ignore_ascii_case(family))
            .peekable();
        faces.peek().is_some() && faces.all(|f| matches!(f.source, FontSource::System(_)))
    }

    /// Does any face have a glyph for `c`?
    pub fn covers(&self, c: char) -> bool {
        self.faces.iter().any(|f| face_covers(f.data(), f.index, c))
    }

    /// Characters of `text` (ignoring whitespace and controls) that no face
    /// of the library covers.
    pub fn uncovered(&self, text: &str) -> BTreeSet<char> {
        self.uncovered_where(text, |_| true)
    }

    /// Characters of `text` that no face usable for language `lang`
    /// covers: bundled and project faces, the system fallbacks for every
    /// language and those of `lang` (system families loaded by name or for
    /// another language are not in this language's chains).
    pub fn uncovered_for(&self, text: &str, lang: Option<&str>) -> BTreeSet<char> {
        let key = lang.map(str::to_lowercase);
        let usable: Vec<&String> = self
            .system_fallbacks
            .iter()
            .chain(
                key.as_ref()
                    .and_then(|k| self.lang_fallbacks.get(k))
                    .into_iter()
                    .flatten(),
            )
            .collect();
        self.uncovered_where(text, |f| {
            !matches!(f.source, FontSource::System(_))
                || usable.iter().any(|u| u.eq_ignore_ascii_case(&f.family))
        })
    }

    fn uncovered_where(&self, text: &str, usable: impl Fn(&FontFace) -> bool) -> BTreeSet<char> {
        use skrifa::MetadataProvider;
        let maps: Vec<skrifa::charmap::Charmap<'_>> = self
            .faces
            .iter()
            .filter(|f| usable(f))
            .filter_map(|f| skrifa::FontRef::from_index(f.data(), f.index).ok())
            .map(|f| f.charmap())
            .collect();
        let mut seen = BTreeSet::new();
        text.chars()
            .filter(|c| !c.is_whitespace() && !c.is_control() && *c != '\u{AD}')
            .filter(|c| seen.insert(*c))
            .filter(|c| !maps.iter().any(|m| m.map(*c).is_some()))
            .collect()
    }

    /// Loads the installed system family `name` (all its faces) unless the
    /// library already has a family of that name: bundled and project
    /// fonts win (ADR 0028). Returns whether it was added.
    pub fn add_system_family(&mut self, index: &SystemFontIndex, name: &str) -> bool {
        if self.has_family(name) {
            return false;
        }
        let Some(family) = index.family(name) else {
            return false;
        };
        let added = self.add_system_faces(family);
        if added {
            self.system_named.push(family.name.clone());
        }
        added
    }

    /// Adds every file of an installed family (each file once).
    fn add_system_faces(&mut self, family: &SystemFamily) -> bool {
        let mut files = BTreeSet::new();
        let mut added = false;
        for face in &family.faces {
            if !files.insert(face.path.clone()) {
                continue;
            }
            if let Some(blob) = face.load() {
                added |= self
                    .add_blob(&blob, &FontSource::System(face.path.clone()))
                    .is_ok();
            }
        }
        added
    }

    /// Loads installed system fonts covering the characters of `text` that
    /// the library lacks, as last-resort fallback families for every
    /// language (see [`Self::add_system_fallbacks_from`]). Layouts then
    /// depend on the machine, so tests never call this. Returns the
    /// families added.
    pub fn add_system_fallbacks(&mut self, text: &str) -> Vec<String> {
        self.add_system_fallbacks_from(&super::system::index(), None, text)
    }

    /// [`Self::add_system_fallbacks`] with explicit font folders.
    pub fn add_fallbacks_from_dirs(&mut self, text: &str, dirs: &[PathBuf]) -> Vec<String> {
        if dirs.is_empty() {
            return Vec::new();
        }
        self.add_system_fallbacks_from(&SystemFontIndex::scan(dirs), None, text)
    }

    /// Fallbacks for the characters of `text` (in language `lang`) that no
    /// font usable for that language covers ([`Self::uncovered_for`]):
    ///
    /// 1. CJK characters: the first installed family of the language's
    ///    preference list ([`super::script::cjk_preferences`]: `Hiragino Sans`,
    ///    `Yu Gothic` for `ja`, `PingFang SC`, `Microsoft YaHei` for `zh-Hans`
    ///    …) that covers any of them – a fallback for `lang` only when it
    ///    is a CJK language, else for every language;
    /// 2. then greedily the family (default face) covering most missing
    ///    characters, repeated while characters remain (at most 8
    ///    families per call), as fallback for every language. Hidden (`.`)
    ///    families and `LastResort` are never candidates.
    pub fn add_system_fallbacks_from(
        &mut self,
        index: &SystemFontIndex,
        lang: Option<&str>,
        text: &str,
    ) -> Vec<String> {
        use super::script::{Script, cjk_flavour, cjk_preferences, script_of};
        let mut missing = self.uncovered_for(text, lang);
        if missing.is_empty() || index.is_empty() {
            return Vec::new();
        }
        let mut added: Vec<String> = Vec::new();
        let cjk: Vec<char> = missing
            .iter()
            .copied()
            .filter(|c| script_of(*c) == Some(Script::Cjk))
            .collect();
        if !cjk.is_empty() {
            let flavour = lang.and_then(cjk_flavour);
            let pick = cjk_preferences(flavour)
                .iter()
                .filter_map(|n| index.family(n))
                .find(|fam| family_count(fam, cjk.iter()) > 0);
            if let Some(fam) = pick {
                let name = fam.name.clone();
                if self.has_family(&name) || self.add_system_faces(fam) {
                    let list = match (flavour, lang) {
                        (Some(_), Some(l)) => {
                            self.lang_fallbacks.entry(l.to_lowercase()).or_default()
                        }
                        _ => &mut self.system_fallbacks,
                    };
                    if !list.contains(&name) {
                        list.push(name.clone());
                    }
                    added.push(name);
                    missing = self.uncovered_for(text, lang);
                }
            }
        }
        let candidates: Vec<&SystemFamily> = index
            .families()
            .filter(|f| !self.has_family(&f.name))
            .collect();
        while !missing.is_empty() && added.len() < 8 {
            let best = candidates
                .iter()
                .filter(|f| !added.contains(&f.name))
                .map(|f| (family_count(f, missing.iter()), *f))
                .filter(|(n, _)| *n > 0)
                // Most covered; ties: the first name.
                .max_by(|a, b| a.0.cmp(&b.0).then_with(|| b.1.name.cmp(&a.1.name)));
            let Some((_, fam)) = best else {
                break;
            };
            if !self.add_system_faces(fam) {
                break;
            }
            self.system_fallbacks.push(fam.name.clone());
            added.push(fam.name.clone());
            missing = self.uncovered_for(text, lang);
        }
        added
    }

    pub fn fallbacks(&self, family: &str) -> &[String] {
        self.fallbacks.get(family).map_or(&[], Vec::as_slice)
    }

    /// The family list used for layout: `family`, its fallbacks, then
    /// [`DEFAULT_FAMILY`], then the system fallback families (if any).
    pub fn family_chain(&self, family: &str) -> Vec<String> {
        self.family_chain_with(family, &[])
    }

    /// Like [`Self::family_chain`] with `extra` fallbacks (the project's
    /// `styles.fonts`) tried before the library's own.
    pub fn family_chain_with(&self, family: &str, extra: &[String]) -> Vec<String> {
        self.family_chain_for(family, extra, None, &[])
    }

    /// The full chain for a text in language `lang`: `family`, `extra`
    /// (project fallbacks), the library's fallbacks for `family`,
    /// [`DEFAULT_FAMILY`], the bundled script fonts `scripts`
    /// ([`super::script::bundled_fallbacks`] of the text), the system
    /// fallbacks of `lang`, then those for every language.
    pub fn family_chain_for(
        &self,
        family: &str,
        extra: &[String],
        lang: Option<&str>,
        scripts: &[&str],
    ) -> Vec<String> {
        let mut out = vec![family.to_owned()];
        let push = |f: &str, out: &mut Vec<String>| {
            if !out.iter().any(|x| x.eq_ignore_ascii_case(f)) {
                out.push(f.to_owned());
            }
        };
        for f in extra.iter().chain(self.fallbacks(family)) {
            push(f, &mut out);
        }
        push(DEFAULT_FAMILY, &mut out);
        for f in scripts {
            push(f, &mut out);
        }
        let by_lang = lang
            .and_then(|l| self.lang_fallbacks.get(&l.to_lowercase()))
            .into_iter()
            .flatten();
        for f in by_lang.chain(&self.system_fallbacks) {
            push(f, &mut out);
        }
        out
    }

    /// Best face of `family` for `weight`/`italic`, following CSS font
    /// matching: italic first, then weight (desired 400–500: up to 500,
    /// then lighter, then heavier; below 400: lighter first; above 500:
    /// heavier first). A variable face matches every weight of its range
    /// exactly; outside it counts with its nearest end (CSS Fonts 4
    /// §5.2). Layout itself matches through fontique the same way and
    /// sets the `wght` axis to the requested weight.
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
            .min_by_key(|(_, f)| {
                let (lo, hi) = (i32::from(f.weights.0), i32::from(f.weights.1));
                (f.italic != italic, rank(d.clamp(lo, hi.max(lo))))
            })
            .map(|(i, _)| FontId(i as u32))
    }
}

/// A fontique weight as a CSS weight (1–1000).
fn weight_value(w: f32) -> u16 {
    w.round().clamp(1.0, 1000.0) as u16
}

/// How many of `chars` the default face of an installed family covers
/// (maps the file once).
fn family_count<'a>(family: &SystemFamily, chars: impl Iterator<Item = &'a char>) -> usize {
    use skrifa::MetadataProvider;
    let Some((blob, index)) = family
        .default_face()
        .and_then(|f| Some((f.load()?, f.index)))
    else {
        return 0;
    };
    let Ok(font) = skrifa::FontRef::from_index(blob.data(), index) else {
        return 0;
    };
    // Bitmap-only colour fonts (Apple Color Emoji: `sbix`, Noto Color
    // Emoji: `CBDT`) have no outlines to draw: never a fallback.
    if [*b"sbix", *b"CBDT"]
        .iter()
        .any(|t| font.table_data(skrifa::raw::types::Tag::new(t)).is_some())
    {
        return 0;
    }
    let map = font.charmap();
    chars.filter(|c| map.map(**c).is_some()).count()
}

/// Does face `index` of font file `data` map `c` to a glyph?
fn face_covers(data: &[u8], index: u32, c: char) -> bool {
    use skrifa::MetadataProvider;
    skrifa::FontRef::from_index(data, index).is_ok_and(|f| f.charmap().map(c).is_some())
}

/// The platform's standard font folders (system and user).
pub fn system_font_dirs() -> Vec<PathBuf> {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let mut out: Vec<PathBuf> = Vec::new();
    if cfg!(target_os = "macos") {
        out.push("/System/Library/Fonts".into());
        out.push("/Library/Fonts".into());
        out.extend(home.map(|h| h.join("Library/Fonts")));
        // Fonts downloaded on demand (Font Book, e.g. PingFang, Songti).
        out.extend(
            std::fs::read_dir("/System/Library/AssetsV2")
                .into_iter()
                .flatten()
                .flatten()
                .map(|e| e.path())
                .filter(|p| {
                    p.file_name().is_some_and(|n| {
                        n.to_string_lossy()
                            .starts_with("com_apple_MobileAsset_Font")
                    })
                }),
        );
    } else if cfg!(target_os = "windows") {
        let windir =
            std::env::var_os("WINDIR").map_or_else(|| PathBuf::from(r"C:\Windows"), PathBuf::from);
        out.push(windir.join("Fonts"));
        out.extend(
            std::env::var_os("LOCALAPPDATA")
                .map(|d| PathBuf::from(d).join(r"Microsoft\Windows\Fonts")),
        );
    } else {
        out.push("/usr/share/fonts".into());
        out.push("/usr/local/share/fonts".into());
        if let Some(h) = home {
            out.push(h.join(".local/share/fonts"));
            out.push(h.join(".fonts"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_faces_have_metadata() {
        let lib = FontLibrary::bundled();
        assert_eq!(
            lib.families(),
            [
                "Bebas Neue",
                "Caveat",
                "Dancing Script",
                "Inter",
                "Inter Display",
                "JetBrains Mono",
                "Lora",
                "Montserrat",
                "Noto Sans Arabic",
                "Noto Sans Bengali",
                "Noto Sans Devanagari",
                "Noto Sans Hebrew",
                "Noto Sans Tamil",
                "Noto Sans Thai",
                "Nunito",
                "Oswald",
                "Playfair Display"
            ]
        );
        // Inter keeps the first face ids.
        let weights: Vec<_> = lib.faces()[..4]
            .iter()
            .map(|f| (f.family.as_str(), f.weight, f.italic))
            .collect();
        assert_eq!(
            weights,
            [
                ("Inter", 400, false),
                ("Inter", 600, false),
                ("Inter Display", 700, false),
                ("Inter Display", 900, false)
            ]
        );
        assert!(lib.has_family("inter display"));
        assert!(lib.has_family("playfair display"));
        assert!(lib.faces().iter().all(FontFace::is_bundled));
        let m = lib.match_face("Montserrat", 400, true).unwrap();
        assert_eq!(lib.face(m).unwrap().weights, (100, 900));
        assert!(lib.face(m).unwrap().italic);
    }

    /// A family mixing static and variable faces: static 300, variable
    /// 500–700, static 900, plus a variable italic 400–800.
    fn mixed_family() -> FontLibrary {
        let face = |weight: u16, weights: (u16, u16), italic: bool| FontFace {
            family: "Mix".to_owned(),
            weight,
            weights,
            italic,
            index: 0,
            source: FontSource::Memory,
            data: FaceData::Bundled(0),
        };
        let mut lib = FontLibrary::empty();
        lib.faces = vec![
            face(300, (300, 300), false),
            face(500, (500, 700), false),
            face(900, (900, 900), false),
            face(400, (400, 800), true),
        ];
        lib
    }

    #[test]
    fn weight_range_matching() {
        let lib = mixed_family();
        let m = |wt: u16, italic: bool| lib.match_face("Mix", wt, italic).unwrap().0;
        // Inside the variable range: exact.
        assert_eq!(m(500, false), 1);
        assert_eq!(m(600, false), 1);
        assert_eq!(m(700, false), 1);
        // 400–500: up to 500 first (the range starts at 500), then lighter.
        assert_eq!(m(400, false), 1);
        assert_eq!(m(450, false), 1);
        // Above 500: heavier first (900), even though 700 is closer.
        assert_eq!(m(800, false), 2);
        assert_eq!(m(1000, false), 2);
        // Below 400: lighter first, then the nearest heavier.
        assert_eq!(m(350, false), 0);
        assert_eq!(m(300, false), 0);
        assert_eq!(m(200, false), 0);
        // Italic wins over weight; the italic range covers 400–800.
        assert_eq!(m(300, true), 3);
        assert_eq!(m(900, true), 3);
        // A static face only: unchanged CSS rules.
        let lib = FontLibrary::bundled();
        let w = |fam: &str, wt: u16, it: bool| {
            let f = lib.face(lib.match_face(fam, wt, it).unwrap()).unwrap();
            (f.weights, f.italic)
        };
        assert_eq!(w("Montserrat", 300, false), ((100, 900), false));
        assert_eq!(w("Montserrat", 900, true), ((100, 900), true));
        // No italic file: the upright face (layout slants it).
        assert_eq!(w("Oswald", 500, true), ((200, 700), false));
        assert_eq!(w("Bebas Neue", 900, false), ((400, 400), false));
    }

    #[test]
    fn variable_project_font_reports_its_range() {
        let mut lib = FontLibrary::empty();
        let f = &bundled::FILES[8];
        assert_eq!(f.file, "lora/Lora[wght].ttf");
        lib.add_font(f.data().to_vec(), &FontSource::Memory)
            .unwrap();
        assert_eq!(lib.faces()[0].weights, (400, 700));
        assert!(!lib.faces()[0].is_bundled());
        let mut lib = FontLibrary::empty();
        lib.add_font(bundled::FILES[0].data().to_vec(), &FontSource::Memory)
            .unwrap();
        assert_eq!(lib.faces()[0].weights, (400, 400));
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
            include_bytes!("../../../../assets/fonts/inter/Inter-Regular.ttf"),
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

    #[test]
    fn fallbacks_from_font_folders_cover_missing_characters() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("Inter-Regular.ttf"),
            include_bytes!("../../../../assets/fonts/inter/Inter-Regular.ttf"),
        )
        .unwrap();
        let mut lib = FontLibrary::empty();
        assert_eq!(lib.uncovered("ab a"), BTreeSet::from(['a', 'b']));
        let added = lib.add_fallbacks_from_dirs("ab", &[dir.path().to_owned()]);
        assert_eq!(added, ["Inter"]);
        assert_eq!(lib.uncovered("ab"), BTreeSet::new());
        assert!(matches!(lib.faces()[0].source, FontSource::System(_)));
        assert_eq!(lib.family_chain("Brand"), ["Brand", "Inter"]);
        // Nothing missing: nothing loaded.
        let none = lib.add_fallbacks_from_dirs("ab", &[dir.path().to_owned()]);
        assert_eq!(none, Vec::<String>::new());
        // A character no font has stays missing.
        let mut lib = FontLibrary::empty();
        let none = lib.add_fallbacks_from_dirs("語", &[dir.path().to_owned()]);
        assert_eq!(none, Vec::<String>::new());
    }
}
