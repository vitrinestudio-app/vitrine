//! Built-in templates, embedded from `crates/core/templates/<id>/` (a
//! package folder without screenshots and previews; placeholders are
//! generated on first use).

use std::collections::BTreeMap;
use std::sync::OnceLock;

use super::{Package, TemplateError, manifest_from_yaml, placeholder};

type Files = &'static [(&'static str, &'static str)];

const PHONE: Files = &[
    (
        "template.yaml",
        include_str!("../../templates/phone/template.yaml"),
    ),
    (
        "project.yaml",
        include_str!("../../templates/phone/project.yaml"),
    ),
    (
        "i18n/en.yaml",
        include_str!("../../templates/phone/i18n/en.yaml"),
    ),
    (
        "i18n/de.yaml",
        include_str!("../../templates/phone/i18n/de.yaml"),
    ),
];

const MINIMAL: Files = &[
    (
        "template.yaml",
        include_str!("../../templates/minimal/template.yaml"),
    ),
    (
        "project.yaml",
        include_str!("../../templates/minimal/project.yaml"),
    ),
    (
        "i18n/en.yaml",
        include_str!("../../templates/minimal/i18n/en.yaml"),
    ),
    (
        "i18n/de.yaml",
        include_str!("../../templates/minimal/i18n/de.yaml"),
    ),
];

const ALL: [(&str, Files); 2] = [("phone", PHONE), ("minimal", MINIMAL)];

/// Ids of the built-in templates, in display order (`phone` is the
/// default for new projects).
pub fn ids() -> [&'static str; 2] {
    ALL.map(|(id, _)| id)
}

/// The default template of new projects.
pub const DEFAULT: &str = "phone";

fn build(files: Files) -> Result<Package, TemplateError> {
    let mut manifest = None;
    let mut map = BTreeMap::new();
    for (path, text) in files {
        if *path == super::MANIFEST_FILE {
            manifest = Some(manifest_from_yaml(text)?);
        } else {
            map.insert((*path).to_owned(), text.as_bytes().to_vec());
        }
    }
    let manifest = manifest.ok_or_else(|| TemplateError::invalid("template.yaml missing"))?;
    let mut pkg = Package::unchecked(manifest, map);
    let project = pkg.project()?;
    for (path, png) in placeholder::for_project(&project) {
        pkg.put(&path, png)?;
    }
    pkg.check()?;
    Ok(pkg)
}

/// A built-in template (built and checked once per process).
pub fn package(id: &str) -> Result<Package, TemplateError> {
    static CACHE: OnceLock<Vec<(&'static str, Result<Package, String>)>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| {
        ALL.iter()
            .map(|(id, files)| (*id, build(files).map_err(|e| e.to_string())))
            .collect()
    });
    match cache.iter().find(|(i, _)| *i == id) {
        Some((_, Ok(p))) => Ok(p.clone()),
        Some((_, Err(e))) => Err(TemplateError::invalid(format!(
            "built-in template `{id}`: {e}"
        ))),
        None => Err(TemplateError::NotFound(format!(
            "no built-in template `{id}`"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtins_are_valid_packages() {
        for id in ids() {
            let p = package(id).unwrap();
            assert_eq!(p.manifest.id, id);
            assert!(p.screenshots().count() >= 1, "{id} has placeholders");
            let project = p.project().unwrap();
            assert_eq!(project.canvases.len(), p.manifest.canvases.len());
        }
        assert!(package("nope").is_err());
    }
}
