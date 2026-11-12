//! `vitrine fonts`: the font families text styles can use – the bundled
//! set (always available, `vitrine_core::layout::bundled`), the families
//! in the project's `fonts/` folder (with stored system fonts) and the
//! installed system families the project uses (ADR 0028). `fonts embed` /
//! `fonts unembed` store the used system fonts in the project or remove
//! them.

use std::collections::BTreeMap;

use clap::{Args, Subcommand};
use serde::Serialize;
use vitrine_core::Project;
use vitrine_core::layout::bundled::{self, FontCategory};
use vitrine_core::layout::stored::{self, FontRole, FontStorePlan, LicenseKind, StoreAction};
use vitrine_core::layout::system;
use vitrine_core::layout::{FontLibrary, FontSource};

use crate::term::Style;
use crate::{EXIT_OK, Format, Global};

#[derive(Debug, Args)]
pub struct FontsArgs {
    #[command(subcommand)]
    pub command: Option<FontsCommand>,
    #[arg(long, value_enum, default_value_t = Format::Text)]
    pub format: Format,
    /// Also list every font family installed on this computer.
    #[arg(long)]
    pub installed: bool,
}

#[derive(Debug, Subcommand)]
pub enum FontsCommand {
    /// Store the system fonts the project uses in `fonts/system/` (so a
    /// build server renders the same). Check the licences: most system
    /// fonts may not be redistributed – private repositories only.
    Embed(EmbedArgs),
    /// Remove the system fonts stored in the project.
    Unembed(UnembedArgs),
}

#[derive(Debug, Args)]
pub struct EmbedArgs {
    /// Only show what would be stored, updated or removed.
    #[arg(long)]
    pub dry_run: bool,
    #[arg(long, value_enum, default_value_t = Format::Text)]
    pub format: Format,
}

#[derive(Debug, Args)]
pub struct UnembedArgs {
    /// Only show what would be removed.
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Debug, Serialize)]
struct FamilyEntry {
    family: String,
    /// `bundled`, `project`, `stored` (system font stored in the project),
    /// `system` (installed on this computer, used by the project) or
    /// `installed` (`--installed`: not used).
    source: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    category: Option<FontCategory>,
    /// Lightest and heaviest weight (variable fonts: any weight between).
    weights: [u16; 2],
    italic: bool,
}

fn category_name(c: FontCategory) -> &'static str {
    match c {
        FontCategory::SansSerif => "sans serif",
        FontCategory::Serif => "serif",
        FontCategory::Display => "display",
        FontCategory::Handwriting => "handwriting",
        FontCategory::Monospace => "monospace",
        FontCategory::Script => "script fallback",
    }
}

fn families(global: &Global, installed: bool) -> Vec<FamilyEntry> {
    let mut out: Vec<FamilyEntry> = bundled::FAMILIES
        .iter()
        .map(|fam| {
            let files: Vec<_> = bundled::files_of(fam.family).collect();
            FamilyEntry {
                family: fam.family.to_owned(),
                source: "bundled",
                category: Some(fam.category),
                weights: [
                    files.iter().map(|f| f.weights.0).min().unwrap_or(400),
                    files.iter().map(|f| f.weights.1).max().unwrap_or(400),
                ],
                italic: files.iter().any(|f| f.italic),
            }
        })
        .collect();
    let lib = if let Ok(p) = Project::load(&global.project) {
        crate::project::fonts(&global.project, &p, global)
    } else {
        let mut lib = FontLibrary::bundled();
        let report = lib.load_dir(&global.project.join("fonts"));
        if !global.quiet {
            crate::project::report_font_errors(&report.errors);
        }
        lib
    };
    let mut by_family: BTreeMap<String, FamilyEntry> = BTreeMap::new();
    for face in lib.faces().iter().filter(|f| !f.is_bundled()) {
        let source = if matches!(face.source, FontSource::System(_)) {
            "system"
        } else if lib
            .stored_families()
            .iter()
            .any(|s| s.eq_ignore_ascii_case(&face.family))
        {
            "stored"
        } else {
            "project"
        };
        let e = by_family
            .entry(face.family.clone())
            .or_insert_with(|| FamilyEntry {
                family: face.family.clone(),
                source,
                category: None,
                weights: [face.weights.0, face.weights.1],
                italic: false,
            });
        e.weights = [
            e.weights[0].min(face.weights.0),
            e.weights[1].max(face.weights.1),
        ];
        e.italic |= face.italic;
    }
    out.extend(by_family.into_values());
    if installed && global.system_fonts() {
        for f in system::index().infos() {
            if !out.iter().any(|e| e.family.eq_ignore_ascii_case(&f.family)) {
                out.push(FamilyEntry {
                    family: f.family,
                    source: "installed",
                    category: None,
                    weights: f.weights,
                    italic: f.italic,
                });
            }
        }
    }
    out
}

pub fn run(args: &FontsArgs, global: &Global) -> anyhow::Result<u8> {
    match &args.command {
        Some(FontsCommand::Embed(a)) => return run_embed(a, global),
        Some(FontsCommand::Unembed(a)) => return run_unembed(a, global),
        None => {}
    }
    let list = families(global, args.installed);
    if args.format == Format::Json {
        println!("{}", serde_json::to_string_pretty(&list)?);
        return Ok(EXIT_OK);
    }
    let style = Style::stdout();
    for f in &list {
        let weights = if f.weights[0] == f.weights[1] {
            f.weights[0].to_string()
        } else {
            format!("{}–{}", f.weights[0], f.weights[1])
        };
        let kind = f.category.map_or(f.source, category_name);
        println!(
            "{:<22} {}",
            f.family,
            style.dim(&format!(
                "{} · {} · weight {weights}{}",
                f.source,
                kind,
                if f.italic { " · italic" } else { "" }
            ))
        );
    }
    Ok(EXIT_OK)
}

fn load(global: &Global) -> anyhow::Result<Project> {
    crate::project::load(&global.project)
}

fn index(global: &Global) -> std::sync::Arc<system::SystemFontIndex> {
    if global.system_fonts() {
        system::index()
    } else {
        std::sync::Arc::new(system::SystemFontIndex::default())
    }
}

fn run_embed(a: &EmbedArgs, global: &Global) -> anyhow::Result<u8> {
    let project = load(global)?;
    let dir = &global.project;
    let index = index(global);
    let plan = if a.dry_run {
        stored::plan(&project, dir, &index)
    } else {
        stored::apply(&project, dir, &index).map_err(anyhow::Error::msg)?
    };
    match a.format {
        Format::Json => println!("{}", serde_json::to_string_pretty(&plan)?),
        Format::Text => print_plan(&plan, global),
    }
    Ok(EXIT_OK)
}

pub(crate) fn mb(bytes: u64) -> String {
    #[allow(clippy::cast_precision_loss, reason = "display only")]
    let b = bytes as f64;
    if bytes < 1_000_000 {
        format!("{:.0} KB", (b / 1000.0).ceil())
    } else {
        format!("{:.1} MB", b / 1_000_000.0)
    }
}

fn license_label(kind: LicenseKind) -> &'static str {
    match kind {
        LicenseKind::Ofl => "OFL (may be shared)",
        LicenseKind::Restricted => "system font licence (no redistribution)",
        LicenseKind::Other => "own licence (check before sharing)",
        LicenseKind::Unknown => "no licence stated (do not redistribute)",
    }
}

fn print_plan(plan: &FontStorePlan, global: &Global) {
    let st = Style::stdout();
    for i in &plan.items {
        if global.quiet && i.action == StoreAction::Keep {
            continue;
        }
        let tag = match i.action {
            StoreAction::Add => st.ok("add     "),
            StoreAction::Update => st.warn("update  "),
            StoreAction::Keep => st.dim("keep    "),
            StoreAction::Remove => st.warn("remove  "),
        };
        let role = match (i.role, &i.lang) {
            (FontRole::Named, _) => "used by styles".to_owned(),
            (FontRole::Fallback, Some(l)) => format!("fallback for {l}"),
            (FontRole::Fallback, None) => "fallback".to_owned(),
        };
        let vendor = if i.license.vendor.is_empty() {
            String::new()
        } else {
            format!(", {}", i.license.vendor)
        };
        println!(
            "{tag} {:<24} {:<34} {:>9}  {}",
            i.family,
            i.file,
            mb(i.bytes),
            st.dim(&format!(
                "{role} · {}{vendor}",
                license_label(i.license.kind)
            ))
        );
    }
    let n = plan
        .items
        .iter()
        .filter(|i| i.action != StoreAction::Remove)
        .count();
    let state = if n == 0 {
        if plan.applied {
            "no system fonts used; nothing stored".to_owned()
        } else {
            "the project uses no system fonts (dry run)".to_owned()
        }
    } else if plan.up_to_date && plan.stored {
        format!("{n} file(s) stored, up to date ({})", mb(plan.bytes_now))
    } else if plan.applied {
        format!(
            "stored {n} file(s) ({}) → {}",
            mb(plan.bytes_after),
            plan.dir.display()
        )
    } else {
        format!(
            "would store {n} file(s) ({}, now {}) (dry run)",
            mb(plan.bytes_after),
            mb(plan.bytes_now)
        )
    };
    println!("{} {state}", st.bold("fonts:"));
    if !plan.restricted.is_empty() {
        println!(
            "{} {} may not be redistributed (or the licence is unclear): keep the \
             project in a private repository, never publish it with `fonts/system/` \
             (templates never include it), or add the folder to .gitignore",
            st.warn("licence:"),
            plan.restricted.join(", ")
        );
    }
}

fn run_unembed(a: &UnembedArgs, global: &Global) -> anyhow::Result<u8> {
    let dir = &global.project;
    let root = stored::stored_root(dir);
    if a.dry_run {
        println!(
            "would remove {} ({})",
            root.display(),
            mb(vitrine_core::frame_import::embed::dir_bytes(&root))
        );
        return Ok(EXIT_OK);
    }
    let bytes = stored::remove(dir).map_err(anyhow::Error::msg)?;
    if !global.quiet {
        if bytes == 0 {
            println!("no system fonts stored in the project");
        } else {
            println!("removed {} ({})", root.display(), mb(bytes));
        }
    }
    Ok(EXIT_OK)
}
