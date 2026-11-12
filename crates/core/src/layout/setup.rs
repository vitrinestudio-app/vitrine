//! The font library of a project (ADR 0028): one function used by the
//! GUI session, the CLI and MCP, so preview, export and validation on one
//! machine see the same fonts.
//!
//! Resolution order for a family a style names: **bundled → project
//! (`fonts/`, then stored system fonts `fonts/system/`) → installed system
//! fonts** (only with `system: true`). Characters no font of a text's
//! chain covers get, in this order, the bundled script fonts (Arabic,
//! Hebrew, Indic, Thai; always), the language's CJK system family and the
//! generic system fallbacks (both only with `system: true`).

use std::collections::BTreeSet;
use std::path::Path;

use super::fonts::{FontDirReport, FontLibrary};
use super::system::SystemFontIndex;
use crate::model::{Project, Styles};

/// Families named by a project's styles: text and span styles, the keys of
/// `styles.fonts` and their fallback lists.
pub fn named_families(styles: &Styles) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    out.extend(styles.text.values().filter_map(|t| t.font.clone()));
    out.extend(styles.spans.values().filter_map(|s| s.font.clone()));
    for (family, f) in &styles.fonts {
        out.insert(family.clone());
        out.extend(f.fallback.iter().cloned());
    }
    out
}

/// Loads the installed system families the project names (unless bundled
/// or project fonts have them) and, per language, system fallbacks for
/// characters no other font covers.
pub fn add_system_fonts(lib: &mut FontLibrary, project: &Project, index: &SystemFontIndex) {
    if index.is_empty() {
        return;
    }
    for family in named_families(&project.styles) {
        lib.add_system_family(index, &family);
    }
    for (lang, map) in &project.strings {
        let text: String = map
            .values()
            .map(String::as_str)
            .collect::<Vec<_>>()
            .join(" ");
        lib.add_system_fallbacks_from(index, Some(lang.as_str()), &text);
    }
}

/// Does the project name a family `lib` lacks, or have characters no
/// font of `lib` covers? Only then are the system fonts scanned (the
/// first scan takes about a second).
pub fn needs_system_fonts(lib: &FontLibrary, project: &Project) -> bool {
    named_families(&project.styles)
        .iter()
        .any(|f| !lib.has_family(f))
        || project.strings.iter().any(|(lang, map)| {
            map.values()
                .any(|t| !lib.uncovered_for(t, Some(lang.as_str())).is_empty())
        })
}

/// The library for the project in `root`: bundled fonts, `fonts/`, the
/// stored system fonts and – with `system` and a project – the installed
/// system fonts it needs (process-wide index, see
/// [`super::system::index`]). Unreadable project font files are reported.
pub fn project_library(
    root: &Path,
    project: Option<&Project>,
    system: bool,
) -> (FontLibrary, FontDirReport) {
    let mut lib = FontLibrary::bundled();
    let mut report = lib.load_dir(&root.join("fonts"));
    let stored = super::stored::load_stored(&mut lib, root);
    report.added.extend(stored.added);
    report.errors.extend(stored.errors);
    if system
        && let Some(p) = project
        && super::system::allowed()
        && needs_system_fonts(&lib, p)
    {
        add_system_fonts(&mut lib, p, &super::system::index());
    }
    (lib, report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Lang, TextStyle};

    #[test]
    fn system_families_are_loaded_by_name_after_bundled_and_project() {
        let sys = tempfile::tempdir().unwrap();
        // Two "installed" families: Lora (also bundled) and Inter (bundled).
        std::fs::write(
            sys.path().join("Lora.ttf"),
            super::super::bundled::FILES[8].data(),
        )
        .unwrap();
        let index = SystemFontIndex::scan(&[sys.path().to_owned()]);
        let mut p = Project::new("p", Lang::new("en"));
        p.styles.text.insert(
            "t".into(),
            TextStyle {
                font: Some("Lora".into()),
                ..Default::default()
            },
        );
        // Bundled wins.
        let mut lib = FontLibrary::bundled();
        add_system_fonts(&mut lib, &p, &index);
        assert!(lib.system_families().is_empty(), "bundled wins");
        assert!(!lib.is_system_family("Lora"));
        // Not bundled: from the system.
        let mut lib = FontLibrary::empty();
        add_system_fonts(&mut lib, &p, &index);
        assert_eq!(lib.system_families(), ["Lora"]);
        assert!(lib.is_system_family("lora"));
        assert!(named_families(&p.styles).contains("Lora"));
    }

    #[test]
    fn project_library_without_system_fonts() {
        let dir = tempfile::tempdir().unwrap();
        let p = Project::new("p", Lang::new("en"));
        let (lib, report) = project_library(dir.path(), Some(&p), false);
        assert!(report.errors.is_empty(), "{:?}", report.errors);
        assert!(lib.system_families().is_empty() && lib.system_fallbacks().is_empty());
        assert!(lib.has_family("Noto Sans Arabic"));
    }
}
