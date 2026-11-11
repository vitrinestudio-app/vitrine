//! Project commands: new, open, close, summary, full model, recent list.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use vitrine_core::Project;
use vitrine_core::model::{Element, ElementKind, FrameStyle, Strings};
use vitrine_core::ops::Op;
use vitrine_core::template::builtin;
use vitrine_core::template::instantiate::{create_project, default_name, parse_langs};

use super::{AppState, NoArgs, absolute};
use crate::error::{CmdResult, CommandError};
use crate::settings::RecentProject;

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectNewArgs {
    /// Folder to create (must not exist or be empty).
    pub dir: PathBuf,
    /// Default: the folder name.
    #[serde(default)]
    pub name: Option<String>,
    /// Template: built-in name (`phone`, `minimal`), id of an installed
    /// template or path of a `.vitrine-template` file. Default: `phone`.
    #[serde(default)]
    pub template: Option<String>,
    /// Languages, the first is the default. Default: the app setting
    /// `default_project_language`.
    #[serde(default)]
    pub langs: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectOpenArgs {
    pub dir: PathBuf,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecentRemoveArgs {
    pub path: PathBuf,
}

/// Page spec of a canvas.
#[derive(Debug, Clone, Serialize)]
pub struct PageSummary {
    /// `w:h`.
    pub aspect: String,
    pub count: u32,
    /// Ids of all pages (explicit or generated `page-NN`).
    pub ids: Vec<String>,
    /// Page height in lu (width is always 1000).
    pub height_lu: f64,
}

/// An element in painting order (groups before their children).
#[derive(Debug, Clone, Serialize)]
pub struct ElementSummary {
    pub id: String,
    pub kind: &'static str,
    /// Home page (0-based).
    pub page: u32,
    /// Human-readable label: text in the default language, device model,
    /// asset path, shape kind.
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    pub visible: bool,
    pub z: i32,
}

#[derive(Debug, Clone, Serialize)]
pub struct CanvasSummary {
    pub id: String,
    pub page: PageSummary,
    pub variants: Vec<String>,
    pub elements: Vec<ElementSummary>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TargetSummary {
    pub id: String,
    pub kind: vitrine_core::model::TargetKind,
    pub canvas: String,
    /// `[w, h]` px.
    pub size: [u32; 2],
    /// Pages per output image.
    pub group: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variant: Option<String>,
    /// Gallery profile preset id.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gallery: Option<String>,
    /// Screenshot variant key.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub screenshots: Option<String>,
    /// Languages exported for this target.
    pub langs: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LanguagesSummary {
    pub default: String,
    pub available: Vec<String>,
    pub fallback: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct StylesSummary {
    pub colors: BTreeMap<String, String>,
    pub text: Vec<String>,
    pub spans: Vec<String>,
    pub shadows: Vec<String>,
    /// Effective frame style (`detailed` when unset).
    pub frame: String,
}

/// Overview of the open project for the UI chrome.
#[derive(Debug, Clone, Serialize)]
pub struct ProjectSummary {
    pub dir: PathBuf,
    pub name: String,
    pub revision: u64,
    pub languages: LanguagesSummary,
    pub canvases: Vec<CanvasSummary>,
    pub targets: Vec<TargetSummary>,
    pub styles: StylesSummary,
}

/// The full model: `project.yaml` content as JSON plus all strings.
#[derive(Debug, Clone, Serialize)]
pub struct ProjectModel {
    pub dir: PathBuf,
    pub revision: u64,
    pub project: Project,
    pub strings: Strings,
}

fn kind_name(e: &Element) -> &'static str {
    e.kind_tag().as_str()
}

fn element_label(p: &Project, e: &Element) -> String {
    let default = p.languages.default.as_str();
    match &e.kind {
        ElementKind::Text(t) => p.string(default, t.text.as_str()).map_or_else(
            || t.text.to_string(),
            |s| {
                let one_line: String = s.split_whitespace().collect::<Vec<_>>().join(" ");
                let mut chars = one_line.chars();
                let short: String = chars.by_ref().take(60).collect();
                if chars.next().is_some() {
                    format!("{short}…")
                } else {
                    short
                }
            },
        ),
        ElementKind::Device(d) => d.model.clone(),
        ElementKind::Image(i) => i.asset.clone(),
        ElementKind::Shape(s) => format!("{:?}", s.shape).to_lowercase(),
        ElementKind::Group(g) => format!("{} elements", g.children.len()),
        ElementKind::Background(_) => "background".to_owned(),
    }
}

fn walk(p: &Project, list: &[Element], parent: Option<&str>, out: &mut Vec<ElementSummary>) {
    for e in list {
        out.push(ElementSummary {
            id: e.id.to_string(),
            kind: kind_name(e),
            page: e.page,
            label: element_label(p, e),
            parent: parent.map(str::to_owned),
            visible: e.visible,
            z: e.z,
        });
        walk(p, e.children(), Some(e.id.as_str()), out);
    }
}

/// Summary of a project.
pub fn summarize(p: &Project, dir: PathBuf) -> ProjectSummary {
    let all_langs: Vec<String> = p
        .languages
        .available
        .iter()
        .map(ToString::to_string)
        .collect();
    ProjectSummary {
        dir,
        name: p.name.clone(),
        revision: p.revision,
        languages: LanguagesSummary {
            default: p.languages.default.to_string(),
            available: all_langs.clone(),
            fallback: p
                .languages
                .fallback
                .iter()
                .map(|(a, b)| (a.to_string(), b.to_string()))
                .collect(),
        },
        canvases: p
            .canvases
            .iter()
            .map(|s| {
                let mut elements = Vec::new();
                walk(p, &s.elements, None, &mut elements);
                CanvasSummary {
                    id: s.id.to_string(),
                    page: PageSummary {
                        aspect: s.page.aspect.to_string(),
                        count: s.page.count,
                        ids: (0..s.page.count).map(|i| s.page.page_id(i)).collect(),
                        height_lu: vitrine_core::page_height_lu(
                            f64::from(s.page.aspect.w),
                            f64::from(s.page.aspect.h),
                        ),
                    },
                    variants: s.variants.keys().map(ToString::to_string).collect(),
                    elements,
                }
            })
            .collect(),
        targets: p
            .targets
            .iter()
            .map(|t| TargetSummary {
                id: t.id.to_string(),
                kind: t.kind,
                canvas: t.canvas.to_string(),
                size: [t.size.w, t.size.h],
                group: t.group_size(),
                variant: t.variant.as_ref().map(ToString::to_string),
                gallery: t.gallery.as_ref().and_then(|g| g.preset.clone()),
                screenshots: t.screenshots.clone(),
                langs: t.langs.as_ref().map_or_else(
                    || all_langs.clone(),
                    |l| l.iter().map(ToString::to_string).collect(),
                ),
            })
            .collect(),
        styles: StylesSummary {
            colors: p
                .styles
                .colors
                .iter()
                .map(|(k, v)| (k.clone(), v.to_string()))
                .collect(),
            text: p.styles.text.keys().cloned().collect(),
            spans: p.styles.spans.keys().cloned().collect(),
            shadows: p.styles.shadows.keys().cloned().collect(),
            frame: p
                .styles
                .frame
                .as_ref()
                .map_or_else(|| "detailed".to_owned(), ToString::to_string),
        },
    }
}

/// Creates a project from a template and opens it.
pub fn project_new(app: &AppState, args: ProjectNewArgs) -> CmdResult<ProjectSummary> {
    let settings = app.settings_store().load().unwrap_or_default();
    let dir = absolute(&args.dir)?;
    let langs = if args.langs.is_empty() {
        vec![settings.default_project_language.clone()]
    } else {
        args.langs
    };
    let langs = parse_langs(&langs)?;
    let name = args.name.unwrap_or_else(|| default_name(&dir));
    let template = args.template.as_deref().unwrap_or(builtin::DEFAULT);
    let pkg = app.templates().find(template)?;
    let mut project = create_project(&pkg, &dir, &name, &langs)?.project;
    // App default frame style for new projects (through the ops API).
    if let Ok(style) = FrameStyle::parse(&settings.default_frame_style)
        && project.styles.frame.is_none()
        && style != FrameStyle::Detailed
    {
        let mut styles = project.styles.clone();
        styles.frame = Some(style);
        project.apply(Op::SetStyles { styles })?;
        project.save(&dir)?;
    }
    project_open(app, ProjectOpenArgs { dir })
}

/// Opens a project folder (closing the current one).
pub fn project_open(app: &AppState, args: ProjectOpenArgs) -> CmdResult<ProjectSummary> {
    let session = app.open_project(&absolute(&args.dir)?)?;
    let summary = session.with_project(|p| summarize(p, session.dir().to_owned()));
    app.events().emit(
        crate::events::PROJECT_OPENED,
        serde_json::json!({
            "dir": summary.dir, "name": summary.name, "actor": crate::Actor::current(),
        }),
    );
    Ok(summary)
}

pub fn project_close(app: &AppState, _: NoArgs) -> CmdResult<()> {
    app.close_project();
    app.events().emit(
        crate::events::PROJECT_CLOSED,
        serde_json::json!({ "actor": crate::Actor::current() }),
    );
    Ok(())
}

pub fn project_summary(app: &AppState, _: NoArgs) -> CmdResult<ProjectSummary> {
    let session = app.session()?;
    Ok(session.with_project(|p| summarize(p, session.dir().to_owned())))
}

pub fn project_get(app: &AppState, _: NoArgs) -> CmdResult<ProjectModel> {
    let session = app.session()?;
    let project = session.snapshot();
    Ok(ProjectModel {
        dir: session.dir().to_owned(),
        revision: project.revision,
        strings: project.strings.clone(),
        project,
    })
}

pub fn recent_projects(app: &AppState, _: NoArgs) -> CmdResult<Vec<RecentProject>> {
    app.recent_store().list()
}

pub fn recent_remove(app: &AppState, args: RecentRemoveArgs) -> CmdResult<()> {
    app.recent_store().remove(&args.path)
}

/// Shared check for commands taking a target id.
pub(super) fn check_target(project: &Project, target: &str) -> CmdResult<()> {
    if project.target(target).is_none() {
        return Err(CommandError::not_found(format!(
            "unknown target `{target}`"
        )));
    }
    Ok(())
}
