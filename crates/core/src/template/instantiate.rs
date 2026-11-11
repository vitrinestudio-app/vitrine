//! New project from a template (`vitrine new --template`, GUI start
//! screen): all canvases, targets, styles, texts, assets and fonts. The
//! placeholder screenshots stay in the template – the new project shows the
//! editor's placeholder until real screenshots are added, so no
//! placeholder can end up in a store upload.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::io::IoError;
use crate::model::{Lang, Languages, Project, StringKey};

use super::{Package, TemplateError, archive, walk};

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
        let lang = Lang::parse(l).map_err(|message| {
            TemplateError::invalid(format!("invalid language `{l}`: {message}"))
        })?;
        if out.contains(&lang) {
            return Err(TemplateError::invalid(format!(
                "language `{l}` is given twice"
            )));
        }
        out.push(lang);
    }
    Ok(out)
}

/// The template language whose texts serve `lang`: the same language, then
/// the primary subtag (`de-AT` → `de`), then a template language with the
/// same primary subtag, else the template's default language.
pub fn source_lang(template: &Languages, lang: &Lang) -> Lang {
    let primary = lang.as_str().split(['-', '_']).next().unwrap_or_default();
    template
        .available
        .iter()
        .find(|l| *l == lang)
        .or_else(|| template.available.iter().find(|l| l.as_str() == primary))
        .or_else(|| {
            template
                .available
                .iter()
                .find(|l| l.as_str().split(['-', '_']).next() == Some(primary))
        })
        .unwrap_or(&template.default)
        .clone()
}

/// Does a language override / `visible_in` entry `key` apply to one of
/// `langs` (equal, or a prefix like `de` for `de-AT`)?
fn applies(key: &Lang, langs: &[Lang]) -> bool {
    langs.iter().any(|l| {
        l == key
            || l.as_str()
                .strip_prefix(key.as_str())
                .is_some_and(|rest| rest.starts_with(['-', '_']))
    })
}

/// Moves the template's texts and language-specific settings to `langs`
/// (the first is the default): texts per [`source_lang`]; language
/// overrides, `visible_in.langs` and target `langs` keep only entries that
/// apply to one of `langs`. Returns the languages whose texts were copied
/// from another language (to be translated).
pub(crate) fn adapt_languages(tpl: &mut Project, langs: &[Lang]) -> Vec<(Lang, Lang)> {
    let old = tpl.languages.clone();
    let mut strings = BTreeMap::new();
    let mut copied = Vec::new();
    for lang in langs {
        let src = source_lang(&old, lang);
        if &src != lang {
            copied.push((lang.clone(), src.clone()));
        }
        let map: BTreeMap<StringKey, String> = tpl.strings.get(&src).cloned().unwrap_or_default();
        if !map.is_empty() {
            strings.insert(lang.clone(), map);
        }
    }
    tpl.strings = strings;
    tpl.languages = Languages {
        default: langs[0].clone(),
        available: langs.to_vec(),
        fallback: BTreeMap::new(),
    };
    for canvas in &mut tpl.canvases {
        walk::elements_mut(&mut canvas.elements, &mut |e| {
            if let Some(v) = &mut e.visible_in {
                v.langs.retain(|l| applies(l, langs));
            }
            if let crate::model::ElementKind::Text(t) = &mut e.kind {
                t.langs.retain(|l, _| applies(l, langs));
            }
        });
    }
    for t in &mut tpl.targets {
        if let Some(list) = &mut t.langs {
            list.retain(|l| langs.contains(l));
            if list.is_empty() {
                t.langs = None;
            }
        }
    }
    copied
}

/// Result of [`create_project`].
#[derive(Debug)]
pub struct Created {
    pub project: Project,
    /// `(language, copied from)` for texts that need translating.
    pub copied_texts: Vec<(Lang, Lang)>,
}

/// Creates a project from `pkg` in `dir` (which must not exist or be
/// empty): project.yaml + i18n, assets, fonts with licence files,
/// `.gitignore`, empty `screenshots/<default>/`.
pub fn create_project(
    pkg: &Package,
    dir: &Path,
    name: &str,
    langs: &[Lang],
) -> Result<Created, TemplateError> {
    if dir.exists() {
        if !dir.is_dir() {
            return Err(TemplateError::Conflict(format!(
                "`{}` exists and is not a folder",
                dir.display()
            )));
        }
        if fs::read_dir(dir)
            .map_err(TemplateError::io(dir))?
            .next()
            .is_some()
        {
            return Err(TemplateError::Conflict(format!(
                "`{}` is not empty; refusing to overwrite (choose a new or empty folder)",
                dir.display()
            )));
        }
    }
    let Some(default) = langs.first() else {
        return Err(TemplateError::invalid("at least one language is required"));
    };
    let mut project = pkg.project()?;
    let copied_texts = adapt_languages(&mut project, langs);
    name.clone_into(&mut project.name);
    project.revision = 0;
    project.check_structure().map_err(TemplateError::Project)?;

    fs::create_dir_all(dir).map_err(TemplateError::io(dir))?;
    for path in pkg
        .paths()
        .filter(|p| p.starts_with("assets/") || p.starts_with("fonts/"))
    {
        let dest = archive::join(dir, path);
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent).map_err(TemplateError::io(parent))?;
        }
        fs::write(&dest, pkg.file(path).unwrap_or_default()).map_err(TemplateError::io(&dest))?;
    }
    let gitignore = dir.join(".gitignore");
    fs::write(&gitignore, "export/\n.cache/\n").map_err(TemplateError::io(&gitignore))?;
    for sub in [
        Path::new("screenshots").join(default.as_str()),
        PathBuf::from("assets"),
        PathBuf::from("fonts"),
    ] {
        let path = dir.join(sub);
        fs::create_dir_all(&path).map_err(TemplateError::io(&path))?;
    }
    project.save(dir).map_err(|e| io_error(&e))?;
    let project = Project::load(dir).map_err(|e| io_error(&e))?;
    Ok(Created {
        project,
        copied_texts,
    })
}

fn io_error(e: &IoError) -> TemplateError {
    TemplateError::Project(e.to_string())
}
