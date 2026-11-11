//! Project folder IO: `project.yaml` + `i18n/<lang>.yaml` (KONZEPT §3.1,
//! §8.3, ADR 0002).
//!
//! - Output is canonical: stable field order, defaults omitted, maps sorted,
//!   small value types as flow maps. Loading and saving a canonical file
//!   reproduces it byte for byte.
//! - Saves are atomic per file (temp file in the same folder + rename) and
//!   guarded by the revision number: if `project.yaml` on disk has another
//!   revision than the one that was loaded, [`IoError::Conflict`] is
//!   returned and nothing is written.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::model::{Lang, Project, StringKey};

/// File name of the project file inside the project folder.
pub const PROJECT_FILE: &str = "project.yaml";
/// Folder with one translation file per language.
pub const I18N_DIR: &str = "i18n";

/// Errors of project IO.
#[derive(Debug, thiserror::Error)]
pub enum IoError {
    #[error("{}: {source}", path.display())]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    /// Syntax, type or unknown-field error, with location.
    #[error("{}: {message}", path.display())]
    Parse { path: PathBuf, message: String },
    /// Structurally invalid project (duplicate ids, wrong format …).
    #[error("{}: {message}", path.display())]
    Invalid { path: PathBuf, message: String },
    /// `project.yaml` was changed on disk since it was loaded.
    #[error(
        "project was changed on disk (revision {found}, expected {expected}); reload and retry"
    )]
    Conflict { expected: u64, found: u64 },
    #[error("cannot serialize: {0}")]
    Serialize(String),
}

fn io_err(path: &Path) -> impl FnOnce(std::io::Error) -> IoError + '_ {
    move |source| IoError::Io {
        path: path.to_owned(),
        source,
    }
}

/// Serialize a value as canonical Vitrine Studio YAML.
pub fn to_yaml<T: Serialize>(value: &T) -> Result<String, IoError> {
    let opts = serde_saphyr::ser_options! {
        yaml_12: true,
        no_lang_directive: true,
        prefer_block_scalars: false,
    };
    serde_saphyr::to_string_with_options(value, opts).map_err(|e| IoError::Serialize(e.to_string()))
}

/// Parse Vitrine Studio YAML into a value (YAML 1.2 core schema: only
/// `true`/`false` are booleans, so `y`, `no`, `on` stay strings).
pub fn from_yaml<T: serde::de::DeserializeOwned>(yaml: &str) -> Result<T, String> {
    let opts = serde_saphyr::options! { strict_booleans: true };
    serde_saphyr::from_str_with_options(yaml, opts).map_err(|e| e.to_string())
}

/// Parse `project.yaml` content (without strings) and check its structure.
pub fn project_from_yaml(yaml: &str) -> Result<Project, String> {
    let mut project: Project = from_yaml(yaml)?;
    project.check_structure()?;
    project.dropped_override_nulls = project.drop_override_nulls();
    Ok(project)
}

/// Parse an i18n file: a flat map key → text. Keys may contain dots.
pub fn strings_from_yaml(yaml: &str) -> Result<BTreeMap<StringKey, String>, String> {
    if yaml.trim().is_empty() {
        return Ok(BTreeMap::new());
    }
    from_yaml(yaml)
}

#[derive(Deserialize)]
struct RevisionOnly {
    #[serde(default)]
    revision: u64,
}

/// Revision of the `project.yaml` currently on disk (`None` if missing).
pub fn disk_revision(dir: &Path) -> Result<Option<u64>, IoError> {
    let path = dir.join(PROJECT_FILE);
    let text = match fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(io_err(&path)(e)),
    };
    let r: RevisionOnly = from_yaml(&text).map_err(|message| IoError::Parse {
        path: path.clone(),
        message,
    })?;
    Ok(Some(r.revision))
}

/// Write a file atomically: temp file in the same folder, then rename.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), IoError> {
    let dir = path.parent().unwrap_or(Path::new("."));
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let tmp = dir.join(format!(".{name}.{}.tmp", std::process::id()));
    let result = (|| {
        use std::io::Write;
        let mut f = fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
        drop(f);
        fs::rename(&tmp, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result.map_err(io_err(path))
}

impl Project {
    /// Load a project folder: `project.yaml` and all `i18n/*.yaml`.
    pub fn load(dir: impl AsRef<Path>) -> Result<Project, IoError> {
        let dir = dir.as_ref();
        let path = dir.join(PROJECT_FILE);
        let text = fs::read_to_string(&path).map_err(io_err(&path))?;
        let mut project: Project = from_yaml(&text).map_err(|message| IoError::Parse {
            path: path.clone(),
            message,
        })?;
        project
            .check_structure()
            .map_err(|message| IoError::Invalid {
                path: path.clone(),
                message,
            })?;
        project.dropped_override_nulls = project.drop_override_nulls();

        let i18n = dir.join(I18N_DIR);
        if i18n.is_dir() {
            let mut files: Vec<PathBuf> = fs::read_dir(&i18n)
                .map_err(io_err(&i18n))?
                .filter_map(Result::ok)
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|x| x == "yaml"))
                .collect();
            files.sort();
            for file in files {
                let stem = file
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_default();
                if stem.starts_with('.') {
                    continue; // temp files of an interrupted save
                }
                let lang = Lang::parse(&stem).map_err(|message| IoError::Invalid {
                    path: file.clone(),
                    message,
                })?;
                let text = fs::read_to_string(&file).map_err(io_err(&file))?;
                let map = strings_from_yaml(&text).map_err(|message| IoError::Parse {
                    path: file.clone(),
                    message,
                })?;
                project.strings.insert(lang, map);
            }
        }
        Ok(project)
    }

    /// `project.yaml` content for this project (canonical YAML).
    pub fn to_yaml(&self) -> Result<String, IoError> {
        to_yaml(self)
    }

    /// Save to a project folder.
    ///
    /// Checks that the revision on disk equals `self.revision` (unless there
    /// is no `project.yaml` yet), writes changed `i18n/*.yaml` files (and
    /// deletes those of languages without strings) and then
    /// `project.yaml` with `revision + 1`, each atomically. On success
    /// `self.revision` is the new revision.
    pub fn save(&mut self, dir: impl AsRef<Path>) -> Result<(), IoError> {
        let dir = dir.as_ref();
        if let Some(found) = disk_revision(dir)?
            && found != self.revision
        {
            return Err(IoError::Conflict {
                expected: self.revision,
                found,
            });
        }
        fs::create_dir_all(dir).map_err(io_err(dir))?;

        // Translation files of languages without strings in the model
        // (language removed, last key deleted) are removed, otherwise the
        // next load would bring them back.
        let i18n = dir.join(I18N_DIR);
        if i18n.is_dir() {
            for entry in fs::read_dir(&i18n).map_err(io_err(&i18n))?.flatten() {
                let path = entry.path();
                let stem = path
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_default();
                if path.extension().is_some_and(|x| x == "yaml")
                    && !stem.starts_with('.')
                    && Lang::parse(&stem).is_ok()
                    && !self.strings.contains_key(stem.as_str())
                {
                    fs::remove_file(&path).map_err(io_err(&path))?;
                }
            }
        }
        if !self.strings.is_empty() {
            fs::create_dir_all(&i18n).map_err(io_err(&i18n))?;
            for (lang, map) in &self.strings {
                let path = i18n.join(format!("{lang}.yaml"));
                let text = to_yaml(map)?;
                let unchanged = fs::read_to_string(&path).is_ok_and(|old| old == text);
                if !unchanged {
                    write_atomic(&path, text.as_bytes())?;
                }
            }
        }

        self.revision += 1;
        let result =
            to_yaml(self).and_then(|text| write_atomic(&dir.join(PROJECT_FILE), text.as_bytes()));
        if result.is_err() {
            self.revision -= 1;
        } else {
            // The file no longer has them.
            self.dropped_override_nulls.clear();
        }
        result
    }
}
