//! `vitrine template save|list|tags|import|export|remove|insert|new`: the
//! app-wide template store (`<app data>/templates/`, same as the GUI;
//! `VITRINE_APP_DIR` overrides it) and templates in projects (ADR 0023).

use std::path::PathBuf;

use anyhow::{Context, bail};
use clap::{Args, Subcommand, ValueEnum};
use vitrine_core::template::extract::{SaveOptions, extract};
use vitrine_core::template::insert::{InsertOptions, StyleConflict, plan_insert};
use vitrine_core::template::store::TemplateStore;
use vitrine_core::template::tags::taxonomy;
use vitrine_core::template::{
    EXTENSION, Limits, Package, TEMPLATES_DIR, TemplateSource, TemplateTags,
};
use vitrine_render::template::{PREVIEW_HEIGHT, add_previews};

use crate::term::{Style, plural};
use crate::{EXIT_OK, Format, Global, new, project};

#[derive(Debug, Args)]
pub struct TemplateArgs {
    #[command(subcommand)]
    pub command: TemplateCommand,
}

#[derive(Debug, Subcommand)]
pub enum TemplateCommand {
    /// Save canvases of the project (default: all) as a template in the store.
    Save(SaveArgs),
    /// List built-in and installed templates.
    List(ListArgs),
    /// List the template tags (facets style, color, use; devices are
    /// derived from the targets).
    Tags(ListArgs),
    /// Install a `.vitrine-template` file into the store (checked first).
    Import(ImportArgs),
    /// Write a template as a `.vitrine-template` file (to share or upload).
    Export(ExportArgs),
    /// Remove an installed template.
    Remove(RemoveArgs),
    /// Insert canvases of a template into the project (one step; ids,
    /// styles and texts that collide are renamed).
    Insert(InsertArgs),
    /// Create a new project from a template (same as `vitrine new`).
    New(new::NewArgs),
}

#[derive(Debug, Args)]
pub struct SaveArgs {
    /// Template name.
    #[arg(long)]
    pub name: String,
    /// Template id (default: derived from the name).
    #[arg(long)]
    pub id: Option<String>,
    #[arg(long, default_value = "")]
    pub description: String,
    #[arg(long, default_value = "")]
    pub author: String,
    /// Style tag (1–2 for the gallery; `vitrine template tags`); repeat
    /// or separate with commas.
    #[arg(long, value_name = "STYLE", value_delimiter = ',', value_parser = facet_values("style"))]
    pub style: Vec<String>,
    /// Colour tag (1–2 for the gallery).
    #[arg(long, value_name = "COLOR", value_delimiter = ',', value_parser = facet_values("color"))]
    pub color: Vec<String>,
    /// App category tag (1–3 for the gallery).
    #[arg(long = "use", value_name = "USE", value_delimiter = ',', value_parser = facet_values("use"))]
    pub use_: Vec<String>,
    /// Free tags are no longer accepted (use --style, --color, --use).
    #[arg(long = "tag", value_name = "TAG", hide = true)]
    pub tag: Vec<String>,
    /// Pack the project's screenshots used by the canvases as sample
    /// screenshots (your own app screenshots, published as CC0 when
    /// uploaded); keys without a screenshot get placeholders.
    #[arg(long)]
    pub sample_screenshots: bool,
    /// Leave out project fonts whose licence does not allow sharing (not
    /// OFL) instead of refusing; the template lists them as required, not
    /// included (used where installed, else Inter).
    #[arg(long)]
    pub skip_unlicensed_fonts: bool,
    /// Template version (default 1.0.0).
    #[arg(long)]
    pub version: Option<String>,
    /// Canvas to save; repeat for more (default: all).
    #[arg(long = "canvas", value_name = "ID")]
    pub canvases: Vec<String>,
    /// Replace an installed template with the same id.
    #[arg(long)]
    pub replace: bool,
    /// Also write the package to this file.
    #[arg(short, long, value_name = "FILE")]
    pub output: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct ListArgs {
    #[arg(long, value_enum, default_value_t = Format::Text)]
    pub format: Format,
}

#[derive(Debug, Args)]
pub struct ImportArgs {
    /// `.vitrine-template` file (or an extracted template folder).
    pub path: PathBuf,
    /// Replace an installed template with the same id.
    #[arg(long)]
    pub replace: bool,
}

#[derive(Debug, Args)]
pub struct ExportArgs {
    /// Template id (installed or built in).
    pub id: String,
    /// Output file (`.vitrine-template` is appended when missing).
    #[arg(short, long, value_name = "FILE")]
    pub output: PathBuf,
}

#[derive(Debug, Args)]
pub struct RemoveArgs {
    pub id: String,
}

/// Rule for style names both define differently.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Styles {
    /// Keep both: the template's style is renamed (`accent-2`).
    Rename,
    /// The inserted canvases use the project's style.
    UseProject,
}

#[derive(Debug, Args)]
pub struct InsertArgs {
    /// Template id or `.vitrine-template` file.
    pub template: String,
    /// Canvas of the template; repeat for more (default: all).
    #[arg(long = "canvas", value_name = "ID")]
    pub canvases: Vec<String>,
    #[arg(long, value_enum, default_value_t = Styles::Rename)]
    pub styles: Styles,
    /// Only show what would happen.
    #[arg(long)]
    pub dry_run: bool,
    #[arg(long, value_enum, default_value_t = Format::Text)]
    pub format: Format,
}

/// Allowed values of a tag facet (for clap).
fn facet_values(facet: &str) -> clap::builder::PossibleValuesParser {
    let values: Vec<&'static str> = taxonomy()
        .facet(facet)
        .map(|f| f.values.iter().map(|v| v.id.as_str()).collect())
        .unwrap_or_default();
    clap::builder::PossibleValuesParser::new(values)
}

/// The error for the removed `--tag`.
fn tag_hint(tags: &[String]) -> anyhow::Error {
    let t = TemplateTags::from_flat(tags);
    let mut hint = Vec::new();
    for (flag, values) in [
        ("--style", &t.style),
        ("--color", &t.color),
        ("--use", &t.use_),
    ] {
        if !values.is_empty() {
            hint.push(format!("{flag} {}", values.join(",")));
        }
    }
    let hint = if hint.is_empty() {
        String::new()
    } else {
        format!(" (e.g. {})", hint.join(" "))
    };
    anyhow::anyhow!(
        "--tag is no longer supported: templates use predefined tags in three facets – \
         --style, --color and --use{hint}; see `vitrine template tags`"
    )
}

/// The app-wide store (`$VITRINE_APP_DIR/data/templates` or the platform
/// data folder).
pub fn store() -> anyhow::Result<TemplateStore> {
    let dir = crate::frames::app_data_dir()
        .context("no app data folder (no home folder?); set VITRINE_APP_DIR")?;
    Ok(TemplateStore::new(dir.join(TEMPLATES_DIR)))
}

pub fn run(args: &TemplateArgs, global: &Global) -> anyhow::Result<u8> {
    match &args.command {
        TemplateCommand::Save(a) => run_save(a, global),
        TemplateCommand::List(a) => run_list(a),
        TemplateCommand::Tags(a) => run_tags(a),
        TemplateCommand::Import(a) => {
            let pkg = Package::open(&a.path, Limits::default())
                .with_context(|| format!("cannot import `{}`", a.path.display()))?;
            let t = store()?.install(&pkg, a.replace)?;
            if !global.quiet {
                println!(
                    "{} template `{}` ({})",
                    Style::stdout().ok("Installed"),
                    t.id,
                    t.name
                );
            }
            Ok(EXIT_OK)
        }
        TemplateCommand::Export(a) => {
            let mut pkg = store()?.find(&a.id)?;
            if pkg.manifest.canvases.iter().any(|c| c.preview.is_none()) {
                add_previews(&mut pkg, PREVIEW_HEIGHT)?;
            }
            let out = with_extension(&a.output);
            pkg.write_zip(&out)?;
            println!("{}", out.display());
            Ok(EXIT_OK)
        }
        TemplateCommand::Remove(a) => {
            store()?.remove(&a.id)?;
            if !global.quiet {
                println!("{} template `{}`", Style::stdout().ok("Removed"), a.id);
            }
            Ok(EXIT_OK)
        }
        TemplateCommand::Insert(a) => run_insert(a, global),
        TemplateCommand::New(a) => new::run(a, global),
    }
}

fn with_extension(p: &std::path::Path) -> PathBuf {
    if p.extension().is_some_and(|e| e == EXTENSION) {
        p.to_owned()
    } else {
        let mut s = p.as_os_str().to_owned();
        s.push(".");
        s.push(EXTENSION);
        PathBuf::from(s)
    }
}

fn run_tags(a: &ListArgs) -> anyhow::Result<u8> {
    let tax = taxonomy();
    if a.format == Format::Json {
        println!("{}", serde_json::to_string_pretty(tax)?);
        return Ok(EXIT_OK);
    }
    let style = Style::stdout();
    for f in &tax.facets {
        let rule = if f.derived {
            "derived from the targets, not set by hand".to_owned()
        } else {
            format!(
                "--{} · {}–{} per template in the gallery",
                f.id, f.min, f.max
            )
        };
        println!("{} {}", style.bold(&f.id), style.dim(&format!("({rule})")));
        for v in &f.values {
            println!("  {:<16} {}", v.id, style.dim(&v.label.en));
        }
    }
    Ok(EXIT_OK)
}

fn run_save(a: &SaveArgs, global: &Global) -> anyhow::Result<u8> {
    if !a.tag.is_empty() {
        return Err(tag_hint(&a.tag));
    }
    let root = &global.project;
    let project = project::load(root)?;
    let opts = SaveOptions {
        id: a.id.clone(),
        name: a.name.clone(),
        description: a.description.clone(),
        author: a.author.clone(),
        tags: TemplateTags {
            style: a.style.clone(),
            color: a.color.clone(),
            use_: a.use_.clone(),
            dropped: Vec::new(),
        },
        version: a.version.clone(),
        canvases: (!a.canvases.is_empty()).then(|| a.canvases.clone()),
        sample_screenshots: a.sample_screenshots,
        skip_unlicensed_fonts: a.skip_unlicensed_fonts,
    };
    let (mut pkg, report) = extract(&project, root, &opts)?;
    pkg.manifest.generator = Some(format!("Vitrine Studio {}", env!("CARGO_PKG_VERSION")));
    add_previews(&mut pkg, PREVIEW_HEIGHT)?;
    let t = store()?.install(&pkg, a.replace)?;
    if let Some(out) = &a.output {
        let out = with_extension(out);
        pkg.write_zip(&out)?;
        if !global.quiet {
            println!("Wrote {}", out.display());
        }
    }
    if !global.quiet {
        let style = Style::stdout();
        println!(
            "{} template `{}` ({}, {} canvas(es), {} placeholder screenshots, licence CC0-1.0)",
            style.ok("Saved"),
            t.id,
            t.name,
            t.canvases.len(),
            report.placeholders
        );
        for f in &report.licenses_generated {
            println!("  licence file generated for {f} (OFL 1.1)");
        }
        for f in &report.fonts_missing {
            println!(
                "  {} font `{f}` is not in the project's fonts/ folder (a system font?): listed as \
                 required, not included – users without it get Inter",
                style.warn("note:")
            );
        }
        for (file, why) in &report.fonts_skipped {
            println!(
                "  {} {file} left out: {why} – listed as required, not included",
                style.warn("note:")
            );
        }
        for (from, to) in &report.assets_moved {
            println!("  asset {from} → {to}");
        }
        if !report.samples.is_empty() {
            println!(
                "  {} packed as samples (your own screenshots; CC0 when uploaded)",
                plural(report.samples.len(), "screenshot")
            );
        }
        for (path, why) in &report.samples_skipped {
            println!("  {} {path} not packed: {why}", style.warn("warning:"));
        }
        if let Err(e) = t.tags.check(true) {
            println!(
                "  {} the gallery needs complete tags: {e}",
                style.warn("note:")
            );
        }
    }
    Ok(EXIT_OK)
}

fn run_list(a: &ListArgs) -> anyhow::Result<u8> {
    let list = store()?.list();
    if a.format == Format::Json {
        println!("{}", serde_json::to_string_pretty(&list)?);
        return Ok(EXIT_OK);
    }
    let style = Style::stdout();
    for t in list {
        let source = match t.source {
            TemplateSource::Builtin => "built in",
            TemplateSource::Installed => "installed",
        };
        let canvases: Vec<&str> = t.canvases.iter().map(|c| c.id.as_str()).collect();
        println!(
            "{}  {} {} – canvases: {}",
            style.bold(&t.id),
            t.name,
            style.dim(&format!("({source}, v{})", t.version)),
            canvases.join(", ")
        );
        if !t.description.is_empty() {
            println!("    {}", t.description);
        }
        let mut tags = t.tags.all();
        tags.extend(t.devices.iter().cloned());
        if !tags.is_empty() || t.samples > 0 {
            let samples = if t.samples > 0 {
                format!(" · {}", plural(t.samples, "sample screenshot"))
            } else {
                String::new()
            };
            println!(
                "    {}",
                style.dim(&format!("{}{samples}", tags.join(", ")))
            );
        }
    }
    Ok(EXIT_OK)
}

fn run_insert(a: &InsertArgs, global: &Global) -> anyhow::Result<u8> {
    let root = &global.project;
    let mut project = project::load(root)?;
    let pkg = store()?.find(&a.template)?;
    let opts = InsertOptions {
        canvases: (!a.canvases.is_empty()).then(|| a.canvases.clone()),
        styles: match a.styles {
            Styles::Rename => StyleConflict::Rename,
            Styles::UseProject => StyleConflict::UseProject,
        },
    };
    let plan = plan_insert(&project, root, &pkg, &opts)?;
    if a.format == Format::Json {
        println!("{}", serde_json::to_string_pretty(&plan.report)?);
    } else if !global.quiet {
        let r = &plan.report;
        for c in &r.canvases {
            println!("canvas {} → {}", c.from, c.to);
        }
        for t in &r.targets {
            println!("target {} → {}", t.from, t.to);
        }
        if !r.elements_renamed.is_empty() {
            println!("{} renamed", plural(r.elements_renamed.len(), "element"));
        }
        for s in &r.styles_renamed {
            println!(
                "style {} → {} (the project defines it differently)",
                s.name,
                s.to.as_deref().unwrap_or("?")
            );
        }
        for s in &r.styles_from_project {
            println!("style {} taken from the project", s.name);
        }
        for k in &r.keys_renamed {
            println!("text key {} → {}", k.from, k.to);
        }
        for (lang, from) in &r.copied_texts {
            println!("texts for {lang} copied from {from}");
        }
        for f in &r.files {
            println!("file {f}");
        }
        for f in &r.fonts_required {
            println!(
                "font {} used but not included (used where installed, else Inter)",
                f.family
            );
        }
    }
    if a.dry_run {
        return Ok(EXIT_OK);
    }
    for f in &plan.files {
        let dest = f.to.split('/').fold(root.clone(), |p, c| p.join(c));
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&dest, pkg.file(&f.from).unwrap_or_default())
            .with_context(|| format!("cannot write `{}`", dest.display()))?;
    }
    vitrine_core::template::insert::mark_samples(root, &pkg, &plan)?;
    if let Err(e) = project.apply_ops(plan.ops.clone()) {
        bail!("cannot insert: {e}");
    }
    project.save(root)?;
    if !global.quiet && a.format == Format::Text {
        println!(
            "{} {} canvas(es) from template `{}`",
            Style::stdout().ok("Inserted"),
            plan.report.canvases.len(),
            pkg.manifest.id
        );
    }
    Ok(EXIT_OK)
}
