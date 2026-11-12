//! The bundled font set (`assets/fonts/`, all SIL OFL 1.1): an embedded
//! manifest plus the font files compiled into the binary.
//!
//! The manifest ([`FILES`], [`FAMILIES`]) carries everything the library
//! needs to know about a face (family, weight range, italic), so building
//! a [`super::FontLibrary`] reads no font file. A file's bytes are only
//! touched when a text uses its family: the layouter then registers the
//! file's [`Blob`] (one per file and process, [`blob`]) with its font
//! collection. Sources and checksums: `assets/fonts/SOURCES.md`.

use std::sync::{Arc, OnceLock};

use parley::fontique::Blob;
use serde::Serialize;

/// One bundled font file.
#[derive(Debug)]
pub struct BundledFile {
    /// Path below `assets/fonts/`.
    pub file: &'static str,
    /// Family name as the font declares it (what styles use).
    pub family: &'static str,
    /// Weight of the default instance (`OS/2` weight class).
    pub weight: u16,
    /// Weight range: the `wght` axis of a variable font, else
    /// `(weight, weight)`.
    pub weights: (u16, u16),
    pub italic: bool,
    data: &'static [u8],
}

impl BundledFile {
    /// The file's bytes (embedded; reading them pages them in).
    pub fn data(&self) -> &'static [u8] {
        self.data
    }
}

/// Style category of a bundled family (for pickers and docs).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FontCategory {
    SansSerif,
    Serif,
    Display,
    Handwriting,
    Monospace,
    /// Fonts for a non-Latin script (Arabic, Hebrew, Indic, Thai), used
    /// automatically for characters of that script
    /// ([`super::script::bundled_fallback`]); selectable like any family.
    Script,
}

/// A bundled family.
#[derive(Debug)]
pub struct BundledFamily {
    pub family: &'static str,
    pub category: FontCategory,
    /// Licence file below `assets/fonts/`.
    pub license: &'static str,
}

macro_rules! file {
    ($path:literal, $family:literal, $weight:literal, $lo:literal ..= $hi:literal, $italic:literal) => {
        BundledFile {
            file: $path,
            family: $family,
            weight: $weight,
            weights: ($lo, $hi),
            italic: $italic,
            data: include_bytes!(concat!("../../../../assets/fonts/", $path)),
        }
    };
}

/// Number of bundled font files.
const N: usize = 24;

/// Every bundled font file. Order = face order of
/// [`super::FontLibrary::bundled`] (Inter first: its face ids are stable).
///
/// A `static`, not a `const`: a `const` would be instantiated at every use
/// and could copy the embedded font bytes into the binary several times.
pub static FILES: [BundledFile; N] = [
    file!("Inter-Regular.ttf", "Inter", 400, 400..=400, false),
    file!("Inter-SemiBold.ttf", "Inter", 600, 600..=600, false),
    file!(
        "InterDisplay-Bold.ttf",
        "Inter Display",
        700,
        700..=700,
        false
    ),
    file!(
        "InterDisplay-Black.ttf",
        "Inter Display",
        900,
        900..=900,
        false
    ),
    file!(
        "montserrat/Montserrat[wght].ttf",
        "Montserrat",
        100,
        100..=900,
        false
    ),
    file!(
        "montserrat/Montserrat-Italic[wght].ttf",
        "Montserrat",
        100,
        100..=900,
        true
    ),
    file!(
        "playfairdisplay/PlayfairDisplay[wght].ttf",
        "Playfair Display",
        400,
        400..=900,
        false
    ),
    file!(
        "playfairdisplay/PlayfairDisplay-Italic[wght].ttf",
        "Playfair Display",
        400,
        400..=900,
        true
    ),
    file!("lora/Lora[wght].ttf", "Lora", 400, 400..=700, false),
    file!("lora/Lora-Italic[wght].ttf", "Lora", 400, 400..=700, true),
    file!("nunito/Nunito[wght].ttf", "Nunito", 200, 200..=1000, false),
    file!(
        "nunito/Nunito-Italic[wght].ttf",
        "Nunito",
        200,
        200..=1000,
        true
    ),
    file!("oswald/Oswald[wght].ttf", "Oswald", 400, 200..=700, false),
    file!(
        "bebasneue/BebasNeue-Regular.ttf",
        "Bebas Neue",
        400,
        400..=400,
        false
    ),
    file!("caveat/Caveat[wght].ttf", "Caveat", 400, 400..=700, false),
    file!(
        "dancingscript/DancingScript[wght].ttf",
        "Dancing Script",
        400,
        400..=700,
        false
    ),
    file!(
        "jetbrainsmono/JetBrainsMono[wght].ttf",
        "JetBrains Mono",
        400,
        100..=800,
        false
    ),
    file!(
        "jetbrainsmono/JetBrainsMono-Italic[wght].ttf",
        "JetBrains Mono",
        400,
        100..=800,
        true
    ),
    file!(
        "notosansarabic/NotoSansArabic[wdth,wght].ttf",
        "Noto Sans Arabic",
        400,
        100..=900,
        false
    ),
    file!(
        "notosanshebrew/NotoSansHebrew[wdth,wght].ttf",
        "Noto Sans Hebrew",
        100,
        100..=900,
        false
    ),
    file!(
        "notosansdevanagari/NotoSansDevanagari[wdth,wght].ttf",
        "Noto Sans Devanagari",
        400,
        100..=900,
        false
    ),
    file!(
        "notosansbengali/NotoSansBengali[wdth,wght].ttf",
        "Noto Sans Bengali",
        400,
        100..=900,
        false
    ),
    file!(
        "notosanstamil/NotoSansTamil[wdth,wght].ttf",
        "Noto Sans Tamil",
        400,
        100..=900,
        false
    ),
    file!(
        "notosansthai/NotoSansThai[wdth,wght].ttf",
        "Noto Sans Thai",
        400,
        100..=900,
        false
    ),
];

/// Every bundled family, in picker order (Inter first).
pub static FAMILIES: &[BundledFamily] = &[
    family("Inter", FontCategory::SansSerif, "OFL.txt"),
    family("Inter Display", FontCategory::SansSerif, "OFL.txt"),
    family("Montserrat", FontCategory::SansSerif, "montserrat/OFL.txt"),
    family("Nunito", FontCategory::SansSerif, "nunito/OFL.txt"),
    family("Oswald", FontCategory::SansSerif, "oswald/OFL.txt"),
    family(
        "Playfair Display",
        FontCategory::Serif,
        "playfairdisplay/OFL.txt",
    ),
    family("Lora", FontCategory::Serif, "lora/OFL.txt"),
    family("Bebas Neue", FontCategory::Display, "bebasneue/OFL.txt"),
    family("Caveat", FontCategory::Handwriting, "caveat/OFL.txt"),
    family(
        "Dancing Script",
        FontCategory::Handwriting,
        "dancingscript/OFL.txt",
    ),
    family(
        "JetBrains Mono",
        FontCategory::Monospace,
        "jetbrainsmono/OFL.txt",
    ),
    family(
        "Noto Sans Arabic",
        FontCategory::Script,
        "notosansarabic/OFL.txt",
    ),
    family(
        "Noto Sans Hebrew",
        FontCategory::Script,
        "notosanshebrew/OFL.txt",
    ),
    family(
        "Noto Sans Devanagari",
        FontCategory::Script,
        "notosansdevanagari/OFL.txt",
    ),
    family(
        "Noto Sans Bengali",
        FontCategory::Script,
        "notosansbengali/OFL.txt",
    ),
    family(
        "Noto Sans Tamil",
        FontCategory::Script,
        "notosanstamil/OFL.txt",
    ),
    family(
        "Noto Sans Thai",
        FontCategory::Script,
        "notosansthai/OFL.txt",
    ),
];

const fn family(
    family: &'static str,
    category: FontCategory,
    license: &'static str,
) -> BundledFamily {
    BundledFamily {
        family,
        category,
        license,
    }
}

/// The bundled family named `name` (case-insensitive).
pub fn family_info(name: &str) -> Option<&'static BundledFamily> {
    FAMILIES
        .iter()
        .find(|f| f.family.eq_ignore_ascii_case(name))
}

/// Files of the bundled family `name` (case-insensitive).
pub fn files_of(name: &str) -> impl Iterator<Item = &'static BundledFile> {
    FILES
        .iter()
        .filter(move |f| f.family.eq_ignore_ascii_case(name))
}

static BLOBS: [OnceLock<Blob<u8>>; N] = [const { OnceLock::new() }; N];

/// The process-wide blob of bundled file `index` (created on first use,
/// then shared by every library and layouter, so fontique sees one font
/// source per file).
///
/// # Panics
/// If `index` is out of range.
pub fn blob(index: usize) -> Blob<u8> {
    BLOBS[index]
        .get_or_init(|| Blob::new(Arc::new(FILES[index].data)))
        .clone()
}

/// Bundled files whose blob exists in this process (for diagnostics and
/// the start-up measurement in `examples/font_bench.rs`).
pub fn loaded_files() -> usize {
    BLOBS.iter().filter(|b| b.get().is_some()).count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::FontLibrary;

    /// The manifest says what the files say (family as fontique reads it,
    /// default weight, `wght` range, italic): the library is built from
    /// the manifest without parsing, so both must agree.
    #[test]
    fn manifest_matches_the_font_files() {
        let mut errors = Vec::new();
        for f in &FILES {
            let mut lib = FontLibrary::empty();
            let ids = lib
                .add_font(f.data().to_vec(), &crate::layout::FontSource::Memory)
                .unwrap_or_else(|e| panic!("{}: {e}", f.file));
            assert_eq!(ids.len(), 1, "{}: one face per file", f.file);
            let face = &lib.faces()[0];
            let got = (face.family.as_str(), face.weight, face.weights, face.italic);
            let want = (f.family, f.weight, f.weights, f.italic);
            if got != want {
                errors.push(format!("{}: file says {got:?}, manifest {want:?}", f.file));
            }
        }
        assert!(errors.is_empty(), "{}", errors.join("\n"));
    }

    /// Every file declares the OFL in its name table (IDs 13/14) and its
    /// family has a licence file with the OFL 1.1 text.
    #[test]
    fn bundled_fonts_are_ofl_with_licence_files() {
        let assets = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/fonts");
        for f in &FILES {
            let names = crate::template::fonts::font_names(f.data()).unwrap();
            assert!(names.declares_ofl(), "{}: {names:?}", f.file);
            let fam =
                family_info(f.family).unwrap_or_else(|| panic!("{} not in FAMILIES", f.family));
            let text = std::fs::read_to_string(assets.join(fam.license)).unwrap();
            assert!(
                text.contains("SIL Open Font License, Version 1.1"),
                "{}",
                fam.license
            );
        }
        for fam in FAMILIES {
            assert!(
                files_of(fam.family).next().is_some(),
                "{}: no files",
                fam.family
            );
        }
    }

    #[test]
    fn blobs_are_shared_per_file() {
        let a = blob(4);
        let b = blob(4);
        assert_eq!(a.id(), b.id());
        assert_eq!(a.data().len(), FILES[4].data().len());
        assert!(loaded_files() >= 1);
    }
}
