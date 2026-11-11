//! Saving canvases (or a whole project) as a template: the project subset,
//! texts, image assets, OFL fonts with licence files and generated
//! placeholder screenshots. Preview images are added by the caller
//! (`vitrine_render::template::add_previews`), which renders them.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::io::{I18N_DIR, PROJECT_FILE};
use crate::layout::FontLibrary;
use crate::model::Project;

use super::{
    CanvasInfo, FONT_LICENSE, FontInfo, LICENSE, Manifest, Package, TEMPLATE_FORMAT, TemplateError,
    archive, fonts, is_template_id, placeholder, walk,
};

/// What to save and the template's metadata.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SaveOptions {
    /// Template id (default: derived from the name).
    #[serde(default)]
    pub id: Option<String>,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub tags: Vec<String>,
    /// Default `1.0.0`.
    #[serde(default)]
    pub version: Option<String>,
    /// Canvas ids to save (default: all canvases of the project).
    #[serde(default)]
    pub canvases: Option<Vec<String>>,
}

/// Remarks about a saved template (nothing here blocks saving).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SaveReport {
    /// Font families named by the styles that are neither bundled nor in
    /// the project's `fonts/` (the template falls back to Inter for them).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fonts_missing: Vec<String>,
    /// Fonts whose licence file was generated from the name table.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub licenses_generated: Vec<String>,
    /// Assets outside `assets/` that were put there (old → new path).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub assets_moved: Vec<(String, String)>,
    /// Number of generated placeholder screenshots.
    pub placeholders: usize,
}

/// Builds a template package from `project` (in folder `root`).
#[allow(clippy::too_many_lines)]
pub fn extract(
    project: &Project,
    root: &Path,
    opts: &SaveOptions,
) -> Result<(Package, SaveReport), TemplateError> {
    let canvases: Vec<String> = match &opts.canvases {
        Some(list) if !list.is_empty() => {
            for id in list {
                if project.canvas(id).is_none() {
                    return Err(TemplateError::NotFound(format!("unknown canvas `{id}`")));
                }
            }
            project
                .canvases
                .iter()
                .map(|c| c.id.to_string())
                .filter(|id| list.contains(id))
                .collect()
        }
        _ => project.canvases.iter().map(|c| c.id.to_string()).collect(),
    };
    if canvases.is_empty() {
        return Err(TemplateError::invalid("the project has no canvas to save"));
    }
    let id = match &opts.id {
        Some(id) if is_template_id(id) => id.clone(),
        Some(id) => {
            return Err(TemplateError::invalid(format!(
                "invalid template id `{id}` (lowercase letters, digits and `-`)"
            )));
        }
        None => super::slug(&opts.name),
    };

    let mut tpl = walk::subset(project, &canvases);
    tpl.name.clone_from(&opts.name);
    tpl.revision = 0;
    let mut report = SaveReport::default();
    let mut files: BTreeMap<String, Vec<u8>> = BTreeMap::new();

    // Image assets: copied under `assets/` (files elsewhere in the project
    // are moved there and the references follow).
    let mut moved = BTreeMap::new();
    let mut seen = BTreeSet::new();
    for canvas in &tpl.canvases {
        for asset in walk::assets(canvas) {
            if !seen.insert(asset.clone()) {
                continue;
            }
            archive::check_path(&asset).map_err(|e| {
                TemplateError::invalid(format!("canvas `{}`: asset {e}", canvas.id))
            })?;
            let src = archive::join(root, &asset);
            let data = std::fs::read(&src).map_err(|_| {
                TemplateError::NotFound(format!(
                    "canvas `{}` uses `{asset}`, which does not exist in the project",
                    canvas.id
                ))
            })?;
            let dest = if asset.starts_with("assets/") {
                asset.clone()
            } else {
                let name = asset.rsplit('/').next().unwrap_or(&asset);
                unique_path(&files, &format!("assets/{name}"))
            };
            if dest != asset {
                moved.insert(asset.clone(), dest.clone());
                report.assets_moved.push((asset.clone(), dest.clone()));
            }
            files.insert(dest, data);
        }
    }
    if !moved.is_empty() {
        for c in &mut tpl.canvases {
            *c = walk::rewrite_assets(c, &moved);
        }
    }

    // Fonts: project font files of the families the styles name.
    let wanted = walk::font_families(&tpl.styles);
    let bundled = FontLibrary::bundled();
    let mut font_infos = Vec::new();
    let mut found = BTreeSet::new();
    for (file, data) in project_fonts(root) {
        let names = fonts::font_names(&data).unwrap_or_else(|_| fonts::FontNames {
            family: String::new(),
            copyright: String::new(),
            license: String::new(),
            license_url: String::new(),
        });
        let family = library_family(&data).unwrap_or(names.family.clone());
        if !wanted.iter().any(|w| w.eq_ignore_ascii_case(&family)) {
            continue;
        }
        let path = format!("fonts/{file}");
        let names = fonts::check_ofl(&path, &data)?;
        let license_file = fonts::license_path(&path);
        let licence = existing_license(root, &file).unwrap_or_else(|| {
            report.licenses_generated.push(path.clone());
            fonts::generate_license(&names)
        });
        files.insert(license_file.clone(), licence.into_bytes());
        files.insert(path.clone(), data);
        found.insert(family.to_lowercase());
        font_infos.push(FontInfo {
            file: path,
            family,
            license: FONT_LICENSE.to_owned(),
            license_file,
            copyright: names.copyright,
        });
    }
    report.fonts_missing = wanted
        .into_iter()
        .filter(|f| !found.contains(&f.to_lowercase()) && !bundled.has_family(f))
        .collect();

    // Placeholders instead of the project's screenshots.
    let shots = placeholder::for_project(&tpl);
    report.placeholders = shots.len();
    files.extend(shots);

    // project.yaml and texts.
    let yaml = tpl
        .to_yaml()
        .map_err(|e| TemplateError::Project(e.to_string()))?;
    files.insert(PROJECT_FILE.to_owned(), yaml.into_bytes());
    for (lang, map) in &tpl.strings {
        let text = crate::io::to_yaml(map).map_err(|e| TemplateError::Project(e.to_string()))?;
        files.insert(format!("{I18N_DIR}/{lang}.yaml"), text.into_bytes());
    }

    let manifest = Manifest {
        format: TEMPLATE_FORMAT.to_owned(),
        id,
        name: opts.name.trim().to_owned(),
        description: opts.description.trim().to_owned(),
        author: opts.author.trim().to_owned(),
        tags: opts.tags.iter().map(|t| super::slug(t)).collect(),
        version: opts.version.clone().unwrap_or_else(|| "1.0.0".to_owned()),
        license: LICENSE.to_owned(),
        project_format: crate::PROJECT_FORMAT.to_owned(),
        canvases: canvas_infos(&tpl),
        fonts: font_infos,
        generator: None,
    };
    let pkg = Package::new(manifest, files)?;
    Ok((pkg, report))
}

/// Manifest entries of the canvases (no previews yet).
pub fn canvas_infos(project: &Project) -> Vec<CanvasInfo> {
    project
        .canvases
        .iter()
        .map(|c| CanvasInfo {
            id: c.id.to_string(),
            aspect: c.page.aspect.to_string(),
            pages: c.page.count,
            targets: project
                .targets
                .iter()
                .filter(|t| t.canvas == c.id)
                .map(|t| t.id.to_string())
                .collect(),
            preview: None,
        })
        .collect()
}

/// `base`, or `stem-2.ext` … when taken.
fn unique_path(files: &BTreeMap<String, Vec<u8>>, base: &str) -> String {
    if !files.contains_key(base) {
        return base.to_owned();
    }
    let (stem, ext) = base.rsplit_once('.').unwrap_or((base, ""));
    (2..10_000)
        .map(|n| format!("{stem}-{n}.{ext}"))
        .find(|p| !files.contains_key(p))
        .unwrap_or_else(|| base.to_owned())
}

/// Font files directly in the project's `fonts/` folder, by name.
fn project_fonts(root: &Path) -> Vec<(String, Vec<u8>)> {
    let mut out: Vec<(String, Vec<u8>)> = std::fs::read_dir(root.join("fonts"))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            let ext = name.rsplit_once('.')?.1.to_ascii_lowercase();
            if !matches!(ext.as_str(), "ttf" | "otf")
                || archive::check_path(&format!("fonts/{name}")).is_err()
                || !e.file_type().is_ok_and(|t| t.is_file())
            {
                return None;
            }
            Some((name, std::fs::read(e.path()).ok()?))
        })
        .collect();
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

/// The family name as layout sees it (fontique), for matching styles.
fn library_family(data: &[u8]) -> Option<String> {
    let mut lib = FontLibrary::empty();
    let ids = lib
        .add_font(data.to_vec(), &crate::layout::FontSource::Memory)
        .ok()?;
    lib.face(*ids.first()?).map(|f| f.family.clone())
}

/// A licence file next to the font in the project (`<stem>-OFL.txt`,
/// `<stem>.txt`, `OFL.txt`, `LICENSE.txt`) that is the OFL text.
fn existing_license(root: &Path, file: &str) -> Option<String> {
    let stem = file.rsplit_once('.').map_or(file, |(s, _)| s);
    let dir = root.join("fonts");
    [
        format!("{stem}-OFL.txt"),
        format!("{stem}.txt"),
        "OFL.txt".to_owned(),
        "LICENSE.txt".to_owned(),
    ]
    .iter()
    .filter_map(|n| std::fs::read_to_string(dir.join(n)).ok())
    .find(|t| fonts::is_ofl_text(t))
}
