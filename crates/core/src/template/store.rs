//! The app-wide template store: `<app data>/templates/<id>/` holds one
//! extracted, checked package per template (saved from a project,
//! imported from a file or downloaded from the gallery). Built-in
//! templates are not stored; [`TemplateStore::find`] resolves both.

use std::fs;
use std::path::{Path, PathBuf};

use super::{
    Limits, MANIFEST_FILE, Package, TemplateError, TemplateSource, TemplateSummary, builtin,
    manifest_from_yaml,
};

/// The store folder.
#[derive(Clone, Debug)]
pub struct TemplateStore {
    root: PathBuf,
}

impl TemplateStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    fn dir(&self, id: &str) -> Result<PathBuf, TemplateError> {
        if !super::is_template_id(id) {
            return Err(TemplateError::invalid(format!(
                "invalid template id `{id}`"
            )));
        }
        Ok(self.root.join(id))
    }

    /// Built-in templates first, then the installed ones by name.
    /// Unreadable store entries are skipped.
    pub fn list(&self) -> Vec<TemplateSummary> {
        let mut out: Vec<TemplateSummary> = builtin::ids()
            .iter()
            .filter_map(|id| builtin::package(id).ok())
            .map(|p| TemplateSummary::of(&p.manifest, TemplateSource::Builtin))
            .collect();
        let mut installed: Vec<TemplateSummary> = fs::read_dir(&self.root)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
            .filter_map(|e| {
                let text = fs::read_to_string(e.path().join(MANIFEST_FILE)).ok()?;
                let m = manifest_from_yaml(&text).ok()?;
                (m.id == e.file_name().to_string_lossy()).then_some(m)
            })
            .map(|m| TemplateSummary::of(&m, TemplateSource::Installed))
            .collect();
        installed.sort_by_key(|t| t.name.to_lowercase());
        out.extend(installed);
        out
    }

    /// An installed template (checked again on every read).
    pub fn get(&self, id: &str) -> Result<Package, TemplateError> {
        let dir = self.dir(id)?;
        if !dir.join(MANIFEST_FILE).is_file() {
            return Err(TemplateError::NotFound(format!(
                "no template `{id}` installed"
            )));
        }
        let pkg = Package::from_dir(&dir, Limits::default())?;
        if pkg.manifest.id != id {
            return Err(TemplateError::invalid(format!(
                "template folder `{id}` holds template `{}`",
                pkg.manifest.id
            )));
        }
        Ok(pkg)
    }

    /// A template by built-in name, installed id, or path of a
    /// `.vitrine-template` file / template folder.
    pub fn find(&self, name: &str) -> Result<Package, TemplateError> {
        if builtin::ids().contains(&name) {
            return builtin::package(name);
        }
        if super::is_template_id(name) && self.root.join(name).is_dir() {
            return self.get(name);
        }
        let path = Path::new(name);
        if path.exists() {
            return Package::open(path, Limits::default());
        }
        Err(TemplateError::NotFound(format!(
            "no template `{name}` (built in: {}; installed: see `vitrine template list`; or give a .vitrine-template file)",
            builtin::ids().join(", ")
        )))
    }

    /// Installs a checked package (`replace`: overwrite one with the same
    /// id). Built-in names are reserved.
    pub fn install(&self, pkg: &Package, replace: bool) -> Result<TemplateSummary, TemplateError> {
        let id = &pkg.manifest.id;
        if builtin::ids().contains(&id.as_str()) {
            return Err(TemplateError::Conflict(format!(
                "`{id}` is the name of a built-in template; choose another id"
            )));
        }
        let dir = self.dir(id)?;
        if dir.exists() && !replace {
            return Err(TemplateError::Conflict(format!(
                "a template `{id}` is already installed (replace it or choose another id)"
            )));
        }
        fs::create_dir_all(&self.root).map_err(TemplateError::io(&self.root))?;
        // Write next to the target, then swap, so a failure leaves the old
        // version in place.
        let staging = self
            .root
            .join(format!(".staging-{id}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&staging);
        pkg.write_dir(&staging)?;
        let old = self.root.join(format!(".old-{id}-{}", std::process::id()));
        if dir.exists() {
            fs::rename(&dir, &old).map_err(TemplateError::io(&dir))?;
        }
        if let Err(e) = fs::rename(&staging, &dir) {
            let _ = fs::rename(&old, &dir);
            let _ = fs::remove_dir_all(&staging);
            return Err(TemplateError::io(&dir)(e));
        }
        let _ = fs::remove_dir_all(&old);
        Ok(TemplateSummary::of(
            &pkg.manifest,
            TemplateSource::Installed,
        ))
    }

    /// Removes an installed template.
    pub fn remove(&self, id: &str) -> Result<(), TemplateError> {
        if builtin::ids().contains(&id) {
            return Err(TemplateError::Conflict(format!(
                "`{id}` is built in and cannot be removed"
            )));
        }
        let dir = self.dir(id)?;
        if !dir.is_dir() {
            return Err(TemplateError::NotFound(format!(
                "no template `{id}` installed"
            )));
        }
        fs::remove_dir_all(&dir).map_err(TemplateError::io(&dir))
    }
}
