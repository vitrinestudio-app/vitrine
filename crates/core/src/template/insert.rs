//! Inserting canvases of a template into an open project (ADR 0023,
//! "Merge rules"). The result is a plan: ops for one undo step plus files
//! to copy (assets, fonts); the caller writes the files and applies the ops
//! as one transaction.
//!
//! Rules, applied to a scratch copy of the template project with its own
//! rename ops, so every reference follows:
//!
//! - **Styles** (colours, then shadows, span styles, text styles): a name
//!   only in the template is added; the same name with an equal definition
//!   is shared; the same name with a different definition is renamed in the
//!   template (`accent` → `accent-2`, [`StyleConflict::Rename`], default)
//!   or the project's definition is used ([`StyleConflict::UseProject`]).
//!   A template shadow named like a built-in preset (`soft`, `hard`,
//!   `none`) the project does not redefine counts as a conflict too. The
//!   project's frame style and font settings win.
//! - **Ids**: canvases, targets and elements colliding with the project get
//!   the next free id (`phone` → `phone-2`).
//! - **Texts**: languages are mapped like for a new project; a key the
//!   project already uses is shared when the texts are equal in every
//!   language, else it becomes `<template id>.<key>`.
//! - **Files**: assets and fonts are copied; an existing file with the same
//!   content is reused, a different one gets a new name (`x-2.png`).

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::model::{CanvasId, ElementId, Lang, Project, StringKey, StyleKind, Styles, TargetId};
use crate::ops::Op;

use super::{
    Package, TemplateError, archive,
    instantiate::{adapt_languages, sample_copies},
    walk,
};

/// How a style name used by both the project and the template with
/// different definitions is resolved.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StyleConflict {
    /// Keep both: the template's style is renamed (`accent-2`).
    #[default]
    Rename,
    /// The inserted canvases use the project's style of that name.
    UseProject,
}

/// What to insert.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InsertOptions {
    /// Canvas ids of the template (default: all).
    #[serde(default)]
    pub canvases: Option<Vec<String>>,
    #[serde(default)]
    pub styles: StyleConflict,
}

/// A file to copy from the package into the project.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileCopy {
    /// Package path.
    pub from: String,
    /// Path relative to the project folder.
    pub to: String,
}

/// An id or name that was changed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Renamed {
    pub from: String,
    pub to: String,
}

/// A style and what happened to it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StyleChange {
    pub kind: StyleKind,
    pub name: String,
    /// New name (renamed conflicts only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<String>,
}

/// What an insert does, for the UI and the CLI.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct InsertReport {
    /// Inserted canvases (`from` = id in the template).
    pub canvases: Vec<Renamed>,
    pub targets: Vec<Renamed>,
    /// Elements whose id changed.
    pub elements_renamed: Vec<Renamed>,
    pub styles_added: Vec<StyleChange>,
    /// Same name, equal definition: shared.
    pub styles_shared: Vec<StyleChange>,
    /// Same name, different definition, renamed in the inserted canvases.
    pub styles_renamed: Vec<StyleChange>,
    /// Same name, different definition, the project's is used.
    pub styles_from_project: Vec<StyleChange>,
    pub keys_renamed: Vec<Renamed>,
    /// `(language, template language)` whose texts were copied over.
    pub copied_texts: Vec<(Lang, Lang)>,
    /// Project paths of copied files (assets, fonts, licences, samples).
    pub files: Vec<String>,
    /// Project paths of the copied sample screenshots (also in `files`;
    /// marked in `screenshots/samples.yaml`). A screenshot the project
    /// already has is never replaced.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub samples: Vec<String>,
}

/// Ops and files of an insert.
#[derive(Clone, Debug, PartialEq)]
pub struct InsertPlan {
    pub ops: Vec<Op>,
    pub files: Vec<FileCopy>,
    pub report: InsertReport,
}

/// The next free name: `base`, else `stem-2`, `stem-3` … (a numeric suffix
/// of `base` is replaced).
fn next_free(base: &str, taken: &dyn Fn(&str) -> bool) -> String {
    if !taken(base) {
        return base.to_owned();
    }
    let stem = match base.rsplit_once('-') {
        Some((s, n)) if !s.is_empty() && !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()) => {
            s
        }
        _ => base,
    };
    appended(stem, taken)
}

/// `base`, else `base-2`, `base-3` … (canvas and target ids often end in a
/// number that means something, like `ios-iphone-69`).
fn next_free_appended(base: &str, taken: &dyn Fn(&str) -> bool) -> String {
    if taken(base) {
        appended(base, taken)
    } else {
        base.to_owned()
    }
}

fn appended(stem: &str, taken: &dyn Fn(&str) -> bool) -> String {
    (2..10_000)
        .map(|n| format!("{stem}-{n}"))
        .find(|c| !taken(c))
        .unwrap_or_else(|| stem.to_owned())
}

fn op_err(what: &str) -> impl FnOnce(crate::ops::OpError) -> TemplateError + '_ {
    move |e| TemplateError::Project(format!("{what}: {e}"))
}

/// Plans inserting canvases of `pkg` into `project` (folder `root`).
#[allow(clippy::too_many_lines)]
pub fn plan_insert(
    project: &Project,
    root: &Path,
    pkg: &Package,
    opts: &InsertOptions,
) -> Result<InsertPlan, TemplateError> {
    let full = pkg.project()?;
    let selected: Vec<String> = match &opts.canvases {
        Some(list) if !list.is_empty() => {
            for id in list {
                if full.canvas(id).is_none() {
                    return Err(TemplateError::NotFound(format!(
                        "the template has no canvas `{id}`"
                    )));
                }
            }
            list.clone()
        }
        _ => full.canvases.iter().map(|c| c.id.to_string()).collect(),
    };
    let mut tpl = walk::subset(&full, &selected);
    let template_langs = tpl.languages.clone();
    let mut report = InsertReport::default();

    // Languages: the project's, default first.
    let mut langs = vec![project.languages.default.clone()];
    langs.extend(
        project
            .languages
            .available
            .iter()
            .filter(|l| **l != project.languages.default)
            .cloned(),
    );
    report.copied_texts = adapt_languages(&mut tpl, &langs);

    merge_styles(project, &mut tpl, opts.styles, &mut report)?;
    let original_ids: Vec<String> = tpl.canvases.iter().map(|c| c.id.to_string()).collect();
    rename_ids(project, &mut tpl, &mut report)?;
    report.canvases = original_ids
        .into_iter()
        .zip(tpl.canvases.iter())
        .map(|(from, c)| Renamed {
            from,
            to: c.id.to_string(),
        })
        .collect();
    let shared_keys = rename_keys(project, &mut tpl, &pkg.manifest.id, &mut report)?;

    // Files.
    let mut files = Vec::new();
    let mut planned: BTreeSet<String> = BTreeSet::new();
    let mut asset_map = BTreeMap::new();
    let assets: BTreeSet<String> = tpl.canvases.iter().flat_map(walk::assets).collect();
    for asset in assets {
        let data = pkg.file(&asset).unwrap_or_default();
        if let Some(to) = place_file(root, &asset, data, &mut planned) {
            if to != asset {
                asset_map.insert(asset.clone(), to.clone());
            }
            files.push(FileCopy { from: asset, to });
        }
    }
    for f in &pkg.manifest.fonts {
        let data = pkg.file(&f.file).unwrap_or_default();
        if let Some(to) = place_file(root, &f.file, data, &mut planned) {
            let licence_to = super::fonts::license_path(&to);
            files.push(FileCopy {
                from: f.file.clone(),
                to,
            });
            files.push(FileCopy {
                from: f.license_file.clone(),
                to: licence_to.clone(),
            });
            planned.insert(licence_to);
        }
    }
    // Sample screenshots of the inserted canvases, where the project has no
    // screenshot of that name yet (any extension).
    let keys: BTreeSet<String> = tpl
        .canvases
        .iter()
        .flat_map(walk::screenshots)
        .map(|u| u.key)
        .collect();
    for (from, to) in sample_copies(pkg, &template_langs, &langs, Some(&keys)) {
        let stem = to.rsplit_once('.').map_or(to.as_str(), |(s, _)| s);
        let taken = crate::resolve::SCREENSHOT_EXTENSIONS.iter().any(|ext| {
            let p = format!("{stem}.{ext}");
            planned.contains(&p) || archive::join(root, &p).exists()
        });
        if taken {
            continue;
        }
        planned.insert(to.clone());
        report.samples.push(to.clone());
        files.push(FileCopy { from, to });
    }
    report.files = files.iter().map(|f| f.to.clone()).collect();

    // Ops: styles, texts, canvases, targets – one transaction.
    let mut ops = Vec::new();
    let merged = merged_styles(&project.styles, &tpl.styles);
    if merged != project.styles {
        ops.push(Op::SetStyles { styles: merged });
    }
    for (lang, map) in &tpl.strings {
        for (key, value) in map {
            if shared_keys.contains(key) {
                continue;
            }
            if project.string(lang.as_str(), key.as_str()) != Some(value.as_str()) {
                ops.push(Op::SetString {
                    lang: lang.clone(),
                    key: key.clone(),
                    value: Some(value.clone()),
                });
            }
        }
    }
    for c in &tpl.canvases {
        ops.push(Op::AddCanvas {
            canvas: walk::rewrite_assets(c, &asset_map),
            index: None,
        });
    }
    for t in &tpl.targets {
        ops.push(Op::AddTarget {
            target: t.clone(),
            index: None,
        });
    }
    // Dry run so the caller gets a clear error before touching files.
    project
        .clone()
        .apply_ops(ops.clone())
        .map_err(op_err("the inserted canvases do not fit the project"))?;
    Ok(InsertPlan { ops, files, report })
}

/// Marks the sample screenshots an insert copied (call after writing
/// `plan.files`).
pub fn mark_samples(root: &Path, pkg: &Package, plan: &InsertPlan) -> Result<(), TemplateError> {
    if plan.report.samples.is_empty() {
        return Ok(());
    }
    let mut marks = crate::samples::SampleMarks::load(root);
    if marks.template.is_empty() {
        marks.template.clone_from(&pkg.manifest.id);
    }
    for f in plan
        .files
        .iter()
        .filter(|f| plan.report.samples.contains(&f.to))
    {
        marks.add(&f.to, pkg.file(&f.from).unwrap_or_default());
    }
    marks
        .save(root)
        .map_err(|e| TemplateError::Project(e.to_string()))
}

/// Where a package file goes: its own path, a reused identical file
/// (`None` = nothing to copy) or a free variant of the name.
fn place_file(
    root: &Path,
    path: &str,
    data: &[u8],
    planned: &mut BTreeSet<String>,
) -> Option<String> {
    let exists = |p: &str| planned.contains(p) || archive::join(root, p).exists();
    if !planned.contains(path) && std::fs::read(archive::join(root, path)).is_ok_and(|d| d == data)
    {
        return None;
    }
    let (stem, ext) = path.rsplit_once('.').unwrap_or((path, ""));
    let to = if exists(path) {
        (2..10_000)
            .map(|n| format!("{stem}-{n}.{ext}"))
            .find(|p| !exists(p))
            .unwrap_or_else(|| path.to_owned())
    } else {
        path.to_owned()
    };
    planned.insert(to.clone());
    Some(to)
}

/// The kinds in merge order: colours first (other styles refer to them).
const KINDS: [StyleKind; 4] = [
    StyleKind::Color,
    StyleKind::Shadow,
    StyleKind::Span,
    StyleKind::Text,
];

/// Names of a kind and whether project and template define them equally.
fn compare(kind: StyleKind, p: &Styles, t: &Styles) -> Vec<(String, Option<bool>)> {
    fn cmp<V: PartialEq>(
        p: &BTreeMap<String, V>,
        t: &BTreeMap<String, V>,
    ) -> Vec<(String, Option<bool>)> {
        t.iter()
            .map(|(k, v)| (k.clone(), p.get(k).map(|pv| pv == v)))
            .collect()
    }
    match kind {
        StyleKind::Color => cmp(&p.colors, &t.colors),
        StyleKind::Text => cmp(&p.text, &t.text),
        StyleKind::Span => cmp(&p.spans, &t.spans),
        StyleKind::Shadow => {
            let mut out = cmp(&p.shadows, &t.shadows);
            for (name, same) in &mut out {
                // Redefining a built-in the project uses as is: conflict.
                if same.is_none() && crate::model::BUILTIN_SHADOWS.contains(&name.as_str()) {
                    *same = Some(false);
                }
            }
            out
        }
    }
}

fn has(kind: StyleKind, s: &Styles, name: &str) -> bool {
    match kind {
        StyleKind::Color => s.colors.contains_key(name),
        StyleKind::Text => s.text.contains_key(name),
        StyleKind::Span => s.spans.contains_key(name),
        StyleKind::Shadow => {
            s.shadows.contains_key(name) || crate::model::BUILTIN_SHADOWS.contains(&name)
        }
    }
}

fn merge_styles(
    project: &Project,
    tpl: &mut Project,
    rule: StyleConflict,
    report: &mut InsertReport,
) -> Result<(), TemplateError> {
    for kind in KINDS {
        for (name, same) in compare(kind, &project.styles, &tpl.styles) {
            let change = |to: Option<String>| StyleChange {
                kind,
                name: name.clone(),
                to,
            };
            match (same, rule) {
                (None, _) => report.styles_added.push(change(None)),
                (Some(true), _) => report.styles_shared.push(change(None)),
                (Some(false), StyleConflict::UseProject) => {
                    report.styles_from_project.push(change(None));
                    drop_style(kind, &mut tpl.styles, &name);
                }
                (Some(false), StyleConflict::Rename) => {
                    let to = next_free(&format!("{name}-2"), &|c| {
                        has(kind, &project.styles, c) || has(kind, &tpl.styles, c)
                    });
                    tpl.apply(Op::RenameStyle {
                        kind,
                        from: name.clone(),
                        to: to.clone(),
                    })
                    .map_err(op_err("renaming a style"))?;
                    report.styles_renamed.push(change(Some(to)));
                }
            }
        }
    }
    Ok(())
}

/// Removes a style from the template's own map (its references then
/// resolve to the project's style of that name).
fn drop_style(kind: StyleKind, s: &mut Styles, name: &str) {
    match kind {
        StyleKind::Color => {
            s.colors.remove(name);
        }
        StyleKind::Text => {
            s.text.remove(name);
        }
        StyleKind::Span => {
            s.spans.remove(name);
        }
        StyleKind::Shadow => {
            s.shadows.remove(name);
        }
    }
}

/// The project's styles plus the template's names it lacks.
fn merged_styles(p: &Styles, t: &Styles) -> Styles {
    let mut out = p.clone();
    for (k, v) in &t.colors {
        out.colors.entry(k.clone()).or_insert(*v);
    }
    for (k, v) in &t.text {
        out.text.entry(k.clone()).or_insert_with(|| v.clone());
    }
    for (k, v) in &t.spans {
        out.spans.entry(k.clone()).or_insert_with(|| v.clone());
    }
    for (k, v) in &t.shadows {
        out.shadows.entry(k.clone()).or_insert_with(|| v.clone());
    }
    for (k, v) in &t.fonts {
        out.fonts.entry(k.clone()).or_insert_with(|| v.clone());
    }
    out
}

fn rename_ids(
    project: &Project,
    tpl: &mut Project,
    report: &mut InsertReport,
) -> Result<(), TemplateError> {
    // Canvases.
    let ids: Vec<CanvasId> = tpl.canvases.iter().map(|c| c.id.clone()).collect();
    for id in ids {
        if project.canvas(id.as_str()).is_some() {
            let to = next_free_appended(id.as_str(), &|c| {
                project.canvas(c).is_some() || tpl.canvas(c).is_some()
            });
            tpl.apply(Op::RenameCanvas {
                from: id,
                to: CanvasId::new(&to),
            })
            .map_err(op_err("renaming a canvas"))?;
        }
    }
    // Targets.
    let ids: Vec<TargetId> = tpl.targets.iter().map(|t| t.id.clone()).collect();
    for id in ids {
        let to = if project.target(id.as_str()).is_some() {
            let to = next_free_appended(id.as_str(), &|c| {
                project.target(c).is_some() || tpl.target(c).is_some()
            });
            tpl.apply(Op::RenameTarget {
                from: id.clone(),
                to: TargetId::new(&to),
            })
            .map_err(op_err("renaming a target"))?;
            to
        } else {
            id.to_string()
        };
        report.targets.push(Renamed {
            from: id.to_string(),
            to,
        });
    }
    // Elements (ids are unique across the project).
    let mut ids = Vec::new();
    for c in &tpl.canvases {
        c.walk_elements(&mut |e| ids.push(e.id.clone()));
    }
    for id in ids {
        if project.element(id.as_str()).is_some() {
            let to = next_free(id.as_str(), &|c| {
                project.element(c).is_some() || tpl.element(c).is_some()
            });
            tpl.apply(Op::RenameElement {
                from: id.clone(),
                to: ElementId::new(&to),
            })
            .map_err(op_err("renaming an element"))?;
            report.elements_renamed.push(Renamed {
                from: id.to_string(),
                to,
            });
        }
    }
    Ok(())
}

/// Renames template keys that the project uses with other texts; returns
/// the keys shared with the project (equal texts).
fn rename_keys(
    project: &Project,
    tpl: &mut Project,
    template_id: &str,
    report: &mut InsertReport,
) -> Result<BTreeSet<StringKey>, TemplateError> {
    let used: BTreeSet<StringKey> = tpl.canvases.iter().flat_map(walk::string_keys).collect();
    let project_has = |k: &str| project.strings.values().any(|m| m.contains_key(k));
    let mut shared = BTreeSet::new();
    for key in used {
        if !project_has(key.as_str()) {
            continue;
        }
        let equal = tpl.strings.iter().all(|(lang, map)| {
            map.get(&key)
                .is_none_or(|v| project.string(lang.as_str(), key.as_str()) == Some(v.as_str()))
        });
        if equal {
            shared.insert(key);
            continue;
        }
        let to = next_free(&format!("{template_id}.{key}"), &|c| {
            project_has(c) || tpl.strings.values().any(|m| m.contains_key(c))
        });
        tpl.apply(Op::RenameStringKey {
            from: key.clone(),
            to: StringKey::new(&to),
        })
        .map_err(op_err("renaming a text key"))?;
        report.keys_renamed.push(Renamed {
            from: key.to_string(),
            to,
        });
    }
    Ok(shared)
}
