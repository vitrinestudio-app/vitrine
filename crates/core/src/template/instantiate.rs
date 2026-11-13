//! New project from a template (`vitrine new --template`, GUI start
//! screen): all canvases, targets, styles, texts, assets and fonts. The
//! placeholder screenshots stay in the template – the new project shows the
//! editor's placeholder until real screenshots are added, so no
//! placeholder can end up in a store upload. Sample screenshots (the
//! author's own, listed in the manifest) are copied and marked in
//! `screenshots/samples.yaml` (`core::samples`): validation reports them
//! and store uploads refuse them until they are replaced.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use crate::io::IoError;
use crate::model::{ElementId, ElementKind, Lang, Languages, Project, StringKey};
use crate::ops::Op;
use crate::samples::SampleMarks;

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

/// The project languages a template language list `keys` (`visible_in.langs`)
/// means once the template's languages `old` become `langs`: those a key
/// applies to (`de` → `de-AT`), and those whose texts come from such a
/// template language ([`source_lang`]: `fr` served by `en` shows what `en`
/// shows). Empty when no project language is meant.
fn remap_langs(keys: &[Lang], old: &Languages, langs: &[Lang]) -> Vec<Lang> {
    langs
        .iter()
        .filter(|lang| {
            let src = source_lang(old, lang);
            keys.iter().any(|k| {
                applies(k, std::slice::from_ref(lang)) || applies(k, std::slice::from_ref(&src))
            })
        })
        .cloned()
        .collect()
}

/// Moves the template's texts and language-specific settings to `langs`
/// (the first is the default): texts per [`source_lang`]; language
/// overrides and `visible_in.langs` follow the texts ([`remap_langs`]: a
/// project language gets what its source language had), target `langs`
/// keep only entries that apply to one of `langs`. Elements shown only in
/// template languages the project does not get (`card-de` in an English
/// project) are removed with their variant overrides: an empty
/// `visible_in` would show them everywhere. Returns the languages whose
/// texts were copied from another language (to be translated).
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
    // Elements left without a project language (removed below).
    let mut orphans: Vec<ElementId> = Vec::new();
    for canvas in &mut tpl.canvases {
        walk::elements_mut(&mut canvas.elements, &mut |e| {
            if let Some(v) = &mut e.visible_in
                && !v.langs.is_empty()
            {
                v.langs = remap_langs(&v.langs, &old, langs);
                if v.langs.is_empty() {
                    orphans.push(e.id.clone());
                }
            }
            if let ElementKind::Text(t) = &mut e.kind {
                let mut kept: BTreeMap<Lang, _> = t
                    .langs
                    .iter()
                    .filter(|(l, _)| applies(l, langs))
                    .map(|(l, p)| (l.clone(), p.clone()))
                    .collect();
                for lang in langs {
                    let one = std::slice::from_ref(lang);
                    if kept.keys().any(|k| applies(k, one)) {
                        continue;
                    }
                    let src = source_lang(&old, lang);
                    let src = std::slice::from_ref(&src);
                    if let Some(p) = t
                        .langs
                        .iter()
                        .find(|(k, _)| applies(k, src))
                        .map(|(_, p)| p)
                    {
                        kept.insert(lang.clone(), p.clone());
                    }
                }
                t.langs = kept;
            }
        });
        for variant in canvas.variants.values_mut() {
            for patch in variant.overrides.values_mut() {
                if let Some(Some(v)) = &mut patch.visible_in
                    && !v.langs.is_empty()
                {
                    v.langs = remap_langs(&v.langs, &old, langs);
                    if v.langs.is_empty() {
                        // Shown in this variant only in languages the
                        // project does not get: hidden in it.
                        patch.visible_in = None;
                        patch.visible = Some(Some(false));
                    }
                }
            }
        }
    }
    tpl.languages = Languages {
        default: langs[0].clone(),
        available: langs.to_vec(),
        fallback: BTreeMap::new(),
    };
    for t in &mut tpl.targets {
        if let Some(list) = &mut t.langs {
            list.retain(|l| langs.contains(l));
            if list.is_empty() {
                t.langs = None;
            }
        }
    }
    remove_orphans(tpl, orphans);
    copied
}

/// Removes the elements `ids` (with their variant overrides). One still
/// used as a `clip_to` mask by a kept image stays, hidden (a mask works
/// hidden too).
fn remove_orphans(tpl: &mut Project, mut ids: Vec<ElementId>) {
    // A mask can be removed once the images clipped to it are gone.
    loop {
        let before = ids.len();
        ids.retain(|id| {
            tpl.element(id.as_str()).is_some()
                && tpl.apply(Op::RemoveElement { id: id.clone() }).is_err()
        });
        if ids.is_empty() || ids.len() == before {
            break;
        }
    }
    for canvas in &mut tpl.canvases {
        walk::elements_mut(&mut canvas.elements, &mut |e| {
            if ids.contains(&e.id) {
                e.visible = false;
                e.visible_in = None;
            }
        });
    }
}

/// Where the template's sample screenshots go in a project with `langs`
/// (the first is the default): `(package path, project path)`. A sample of
/// template language `src` is copied to the project's default language when
/// `src` serves it ([`source_lang`]) and to every project language equal to
/// `src`; other languages fall back to the default language's files. With
/// `keys`, only samples of those screenshot keys.
pub(crate) fn sample_copies(
    pkg: &Package,
    template: &Languages,
    langs: &[Lang],
    keys: Option<&BTreeSet<String>>,
) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for path in &pkg.manifest.samples {
        let Some(rest) = path.strip_prefix("screenshots/") else {
            continue;
        };
        let Some((src, tail)) = rest.split_once('/') else {
            continue;
        };
        if let Some(keys) = keys {
            let file = tail.rsplit('/').next().unwrap_or(tail);
            let stem = file.rsplit_once('.').map_or(file, |(s, _)| s);
            if !keys.contains(stem) {
                continue;
            }
        }
        for (i, lang) in langs.iter().enumerate() {
            let serves = source_lang(template, lang).as_str() == src;
            if serves && (i == 0 || lang.as_str() == src) {
                out.push((path.clone(), format!("screenshots/{lang}/{tail}")));
            }
        }
    }
    out
}

/// Result of [`create_project`].
#[derive(Debug)]
pub struct Created {
    pub project: Project,
    /// `(language, copied from)` for texts that need translating.
    pub copied_texts: Vec<(Lang, Lang)>,
    /// Project paths of the copied sample screenshots.
    pub samples: Vec<String>,
    /// Font families the template uses but does not include (see
    /// [`super::unavailable_fonts`]).
    pub fonts_required: Vec<super::RequiredFont>,
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
    let samples = sample_copies(pkg, &project.languages, langs, None);
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
    let mut marks = SampleMarks {
        template: pkg.manifest.id.clone(),
        ..SampleMarks::default()
    };
    for (from, to) in &samples {
        let data = pkg.file(from).unwrap_or_default();
        let dest = archive::join(dir, to);
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent).map_err(TemplateError::io(parent))?;
        }
        fs::write(&dest, data).map_err(TemplateError::io(&dest))?;
        marks.add(to, data);
    }
    marks.save(dir).map_err(|e| io_error(&e))?;
    project.save(dir).map_err(|e| io_error(&e))?;
    let project = Project::load(dir).map_err(|e| io_error(&e))?;
    Ok(Created {
        project,
        copied_texts,
        samples: samples.into_iter().map(|(_, to)| to).collect(),
        fonts_required: pkg.manifest.fonts_required.clone(),
    })
}

fn io_error(e: &IoError) -> TemplateError {
    TemplateError::Project(e.to_string())
}
