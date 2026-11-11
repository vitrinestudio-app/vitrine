//! Built-in project templates (`vitrine new`, GUI "New project"): a
//! project folder (KONZEPT §3.1) with `project.yaml`, `i18n/<lang>.yaml`,
//! `.gitignore` and empty `screenshots/<default>/`, `assets/`, `fonts/`.
//!
//! The template files live in `crates/core/templates/<name>/` and are
//! embedded at compile time. `project.yaml` templates contain the
//! placeholders `__NAME__`, `__DEFAULT_LANG__` and `__LANGS__`.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::io::{I18N_DIR, IoError, PROJECT_FILE, project_from_yaml};
use crate::model::{Lang, Project};

/// A built-in template.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Template {
    /// Three phone pages with gradient, headlines and devices; targets
    /// iPhone 6.9″ (App Store), Google Play phone and a blog banner.
    Phone,
    /// One page with a headline and a device; target iPhone 6.9″.
    Minimal,
}

/// Embedded files of a template.
struct Files {
    project: &'static str,
    /// Texts per language; the first entry is used for languages without
    /// their own entry.
    strings: &'static [(&'static str, &'static str)],
}

impl Template {
    /// All templates, in display order.
    pub const ALL: [Template; 2] = [Template::Phone, Template::Minimal];

    fn files(self) -> Files {
        match self {
            Self::Phone => Files {
                project: include_str!("../templates/phone/project.yaml"),
                strings: &[
                    ("en", include_str!("../templates/phone/en.yaml")),
                    ("de", include_str!("../templates/phone/de.yaml")),
                ],
            },
            Self::Minimal => Files {
                project: include_str!("../templates/minimal/project.yaml"),
                strings: &[
                    ("en", include_str!("../templates/minimal/en.yaml")),
                    ("de", include_str!("../templates/minimal/de.yaml")),
                ],
            },
        }
    }

    /// Identifier (`phone`, `minimal`).
    pub fn name(self) -> &'static str {
        match self {
            Self::Phone => "phone",
            Self::Minimal => "minimal",
        }
    }

    /// One-line English description.
    pub fn description(self) -> &'static str {
        match self {
            Self::Phone => {
                "Three phone pages with gradient, headlines and devices; iPhone 6.9″, Google Play phone, blog banner"
            }
            Self::Minimal => "One page with a headline and a device; iPhone 6.9″",
        }
    }

    /// Template by identifier.
    pub fn parse(name: &str) -> Option<Template> {
        Self::ALL.into_iter().find(|t| t.name() == name)
    }
}

/// Why a project could not be created.
#[derive(Debug, thiserror::Error)]
pub enum TemplateError {
    #[error("`{}` exists and is not a folder", .0.display())]
    NotADir(PathBuf),
    #[error("`{}` is not empty; refusing to overwrite (choose a new or empty folder)", .0.display())]
    NotEmpty(PathBuf),
    #[error("invalid language `{lang}`: {message}")]
    InvalidLang { lang: String, message: String },
    #[error("language `{0}` is given twice")]
    DuplicateLang(String),
    #[error("template does not parse: {0}")]
    Template(String),
    #[error("{}: {source}", path.display())]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("created project does not load: {0}")]
    Load(#[from] IoError),
}

fn io_err(path: &Path) -> impl FnOnce(std::io::Error) -> TemplateError + '_ {
    move |source| TemplateError::Io {
        path: path.to_owned(),
        source,
    }
}

/// YAML double-quoted scalar for `s`.
fn yaml_quoted(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => {
                use std::fmt::Write;
                let _ = write!(out, "\\u{:04X}", u32::from(c));
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// `project.yaml` text of `template` with name and languages filled in.
pub fn project_yaml(
    template: Template,
    name: &str,
    langs: &[Lang],
) -> Result<String, TemplateError> {
    let list: Vec<&str> = langs.iter().map(Lang::as_str).collect();
    let yaml = template
        .files()
        .project
        .replace("__NAME__", &yaml_quoted(name))
        .replace("__DEFAULT_LANG__", list.first().copied().unwrap_or("en"))
        .replace("__LANGS__", &format!("[{}]", list.join(", ")));
    project_from_yaml(&yaml).map_err(TemplateError::Template)?;
    Ok(yaml)
}

/// Texts of `template` for `lang`: exact match, then the primary subtag
/// (`de-AT` → `de`), else the template's first language marked for
/// translation.
pub fn strings_for(template: Template, lang: &str) -> String {
    let files = template.files();
    let primary = lang.split(['-', '_']).next().unwrap_or(lang);
    if let Some((_, s)) = files
        .strings
        .iter()
        .find(|(l, _)| *l == lang)
        .or_else(|| files.strings.iter().find(|(l, _)| *l == primary))
    {
        return (*s).to_owned();
    }
    let (from, s) = files.strings[0];
    format!("# TODO: translate to `{lang}` (copied from `{from}`)\n{s}")
}

/// Project name derived from a folder (`grove.vitrine` → `grove`).
pub fn default_name(dir: &Path) -> String {
    std::path::absolute(dir)
        .ok()
        .and_then(|p| {
            p.file_name()
                .map(|n| n.to_string_lossy().trim_end_matches(".vitrine").to_owned())
        })
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| "My App".to_owned())
}

/// Parses language codes (default `["en"]`); the first is the default
/// language. Rejects invalid and duplicate codes.
pub fn parse_langs(langs: &[String]) -> Result<Vec<Lang>, TemplateError> {
    let default_langs = ["en".to_owned()];
    let given = if langs.is_empty() {
        &default_langs[..]
    } else {
        langs
    };
    let mut out: Vec<Lang> = Vec::new();
    for l in given {
        let lang = Lang::parse(l).map_err(|message| TemplateError::InvalidLang {
            lang: l.clone(),
            message,
        })?;
        if out.contains(&lang) {
            return Err(TemplateError::DuplicateLang(l.clone()));
        }
        out.push(lang);
    }
    Ok(out)
}

/// Creates a project folder from `template` in `dir` (which must not exist
/// or be empty) and returns the loaded project. `langs` must not be empty;
/// the first is the default language.
pub fn create_project(
    dir: &Path,
    name: &str,
    template: Template,
    langs: &[Lang],
) -> Result<Project, TemplateError> {
    if dir.exists() {
        if !dir.is_dir() {
            return Err(TemplateError::NotADir(dir.to_owned()));
        }
        if fs::read_dir(dir).map_err(io_err(dir))?.next().is_some() {
            return Err(TemplateError::NotEmpty(dir.to_owned()));
        }
    }
    let Some(default) = langs.first() else {
        return Err(TemplateError::InvalidLang {
            lang: String::new(),
            message: "at least one language is required".to_owned(),
        });
    };
    let yaml = project_yaml(template, name, langs)?;

    let write = |rel: &str, text: &str| -> Result<(), TemplateError> {
        let path = dir.join(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(io_err(parent))?;
        }
        fs::write(&path, text).map_err(io_err(&path))
    };
    write(PROJECT_FILE, &yaml)?;
    for lang in langs {
        write(
            &format!("{I18N_DIR}/{lang}.yaml"),
            &strings_for(template, lang.as_str()),
        )?;
    }
    write(".gitignore", "export/\n.cache/\n")?;
    for sub in [
        Path::new("screenshots").join(default.as_str()),
        PathBuf::from("assets"),
        PathBuf::from("fonts"),
    ] {
        let path = dir.join(sub);
        fs::create_dir_all(&path).map_err(io_err(&path))?;
    }
    // The written project must load (guards the embedded templates).
    Ok(Project::load(dir)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn langs(list: &[&str]) -> Vec<Lang> {
        list.iter().map(|l| Lang::parse(l).unwrap()).collect()
    }

    #[test]
    fn templates_parse_with_names_needing_quotes() {
        for t in Template::ALL {
            let yaml = project_yaml(t, "Grove: \"eco\" #1 \\ ü", &langs(&["en", "de"])).unwrap();
            let p = project_from_yaml(&yaml).unwrap();
            assert_eq!(p.name, "Grove: \"eco\" #1 \\ ü");
            assert_eq!(p.languages.available.len(), 2);
        }
    }

    #[test]
    fn strings_fall_back_to_primary_subtag_then_first() {
        assert!(strings_for(Template::Phone, "de-AT").contains("Deine App"));
        let fr = strings_for(Template::Phone, "fr");
        assert!(fr.starts_with("# TODO: translate to `fr`"));
        assert!(fr.contains("Your app"));
    }

    #[test]
    fn creates_a_loadable_project_and_refuses_non_empty_folders() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("grove.vitrine");
        let p = create_project(&dir, "Grove", Template::Minimal, &langs(&["de", "en"])).unwrap();
        assert_eq!(p.name, "Grove");
        assert_eq!(p.languages.default.as_str(), "de");
        assert!(dir.join("screenshots/de").is_dir());
        assert!(matches!(
            create_project(&dir, "Grove", Template::Minimal, &langs(&["en"])),
            Err(TemplateError::NotEmpty(_))
        ));
        assert_eq!(default_name(&dir), "grove");
    }

    #[test]
    fn parses_language_lists() {
        assert_eq!(parse_langs(&[]).unwrap(), langs(&["en"]));
        assert!(matches!(
            parse_langs(&["en".into(), "en".into()]),
            Err(TemplateError::DuplicateLang(_))
        ));
        assert!(parse_langs(&["not a lang".into()]).is_err());
        assert_eq!(Template::parse("minimal"), Some(Template::Minimal));
    }
}
