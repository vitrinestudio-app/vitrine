//! `vitrine import screenshots <dir>`: plan (and apply) a screenshot
//! import with `core::import` (Vitrine Studio folders, fastlane snapshot output,
//! flat folders). Prints the plan grouped by language × variant.

use std::collections::BTreeMap;
use std::path::PathBuf;

use anyhow::{Context, bail};
use clap::{Args, Subcommand, ValueEnum};
use serde::Serialize;
use vitrine_core::import::{
    Action, ApplyReport, Confidence, ImportPlan, ImportRules, MissingStatus, PlannedFile,
    SourceFormat, apply, plan_screenshot_import,
};
use vitrine_core::model::{ImportSettings, Lang};
use vitrine_core::ops::Op;

use crate::term::{Style, plural};
use crate::{EXIT_OK, EXIT_PROBLEMS, Format, Global, project};

#[derive(Debug, Subcommand)]
pub enum ImportCommand {
    /// Copy screenshots from a folder into `screenshots/<lang>/<variant>/<key>.png`.
    Screenshots(ScreenshotsArgs),
}

/// Source layout (`--format`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum SourceArg {
    Auto,
    Vitrine,
    Fastlane,
    Flat,
}

impl From<SourceArg> for SourceFormat {
    fn from(f: SourceArg) -> Self {
        match f {
            SourceArg::Auto => Self::Auto,
            SourceArg::Vitrine => Self::Vitrine,
            SourceArg::Fastlane => Self::Fastlane,
            SourceArg::Flat => Self::Flat,
        }
    }
}

#[derive(Debug, Args)]
pub struct ScreenshotsArgs {
    /// Source folder (fastlane `screenshots/`, a Vitrine Studio `screenshots/`
    /// tree or a folder of files).
    pub dir: PathBuf,
    /// Source layout.
    #[arg(long, value_enum, default_value_t = SourceArg::Auto)]
    pub format: SourceArg,
    /// Locale → language, e.g. `de-DE=de`; repeat for more.
    #[arg(long = "lang-map", value_name = "LOCALE=LANG")]
    pub lang_map: Vec<String>,
    /// Device name → screenshot variant, e.g. `iPhone 16 Pro Max=ios-phone`.
    #[arg(long = "device-map", value_name = "DEVICE=VARIANT")]
    pub device_map: Vec<String>,
    /// Screen name → screenshot key, e.g. `01_home=home`.
    #[arg(long = "key-map", value_name = "SCREEN=KEY")]
    pub key_map: Vec<String>,
    /// Import frameit's `*_framed.png` instead of skipping them.
    #[arg(long)]
    pub include_framed: bool,
    /// Only print the plan; copy nothing.
    #[arg(long)]
    pub dry_run: bool,
    /// Store the effective device/locale/key mapping in `project.yaml`
    /// (`import.fastlane`), so later imports map the same way.
    #[arg(long)]
    pub save_mapping: bool,
    /// Output format (`json`: plan and apply report).
    #[arg(long = "format-output", value_enum, default_value_t = Format::Text)]
    pub output: Format,
}

fn pairs(list: &[String], what: &str) -> anyhow::Result<Vec<(String, String)>> {
    list.iter()
        .map(|s| {
            let (a, b) = s
                .rsplit_once('=')
                .with_context(|| format!("--{what} `{s}`: expected NAME=VALUE"))?;
            let (a, b) = (a.trim(), b.trim());
            if a.is_empty() || b.is_empty() {
                bail!("--{what} `{s}`: expected NAME=VALUE");
            }
            Ok((a.to_owned(), b.to_owned()))
        })
        .collect()
}

#[derive(Serialize)]
struct JsonOut<'a> {
    ok: bool,
    dry_run: bool,
    plan: &'a ImportPlan,
    #[serde(skip_serializing_if = "Option::is_none")]
    applied: Option<&'a ApplyReport>,
    mapping_saved: bool,
}

pub fn run(cmd: &ImportCommand, global: &Global) -> anyhow::Result<u8> {
    match cmd {
        ImportCommand::Screenshots(args) => screenshots(args, global),
    }
}

fn screenshots(args: &ScreenshotsArgs, global: &Global) -> anyhow::Result<u8> {
    let root = &global.project;
    let mut project = project::load(root)?;
    let mut rules = ImportRules {
        format: args.format.into(),
        include_framed: args.include_framed,
        ..ImportRules::default()
    };
    for (locale, lang) in pairs(&args.lang_map, "lang-map")? {
        let l = Lang::parse(&lang).map_err(|e| anyhow::anyhow!("--lang-map: {e}"))?;
        rules.mapping.locales.insert(locale, l);
    }
    for (device, variant) in pairs(&args.device_map, "device-map")? {
        rules.mapping.devices.insert(device, variant);
    }
    for (screen, key) in pairs(&args.key_map, "key-map")? {
        rules.mapping.keys.insert(screen, key);
    }
    rules.mapping.check().map_err(|e| anyhow::anyhow!("{e}"))?;

    let plan = plan_screenshot_import(&project, root, &args.dir, &rules)?;
    let report = (!args.dry_run).then(|| apply(&plan, root));

    let mut mapping_saved = false;
    if args.save_mapping && !args.dry_run && plan.mapping != project.import.fastlane {
        let import = ImportSettings {
            fastlane: plan.mapping.clone(),
        };
        project
            .apply(Op::SetImport { import })
            .map_err(|e| anyhow::anyhow!("cannot store the mapping: {e}"))?;
        project.save(root).context("cannot save project.yaml")?;
        mapping_saved = true;
    }

    let failed = report.as_ref().is_some_and(|r| !r.failed.is_empty());
    let ok = !failed && !plan.has_conflicts();
    match args.output {
        Format::Json => {
            let out = JsonOut {
                ok,
                dry_run: args.dry_run,
                plan: &plan,
                applied: report.as_ref(),
                mapping_saved,
            };
            println!("{}", serde_json::to_string_pretty(&out)?);
        }
        Format::Text => print_text(&plan, report.as_ref(), mapping_saved, args, global),
    }
    Ok(if ok { EXIT_OK } else { EXIT_PROBLEMS })
}

fn action_label(a: Action) -> &'static str {
    match a {
        Action::New => "new",
        Action::Overwrite => "overwrite",
        Action::Unchanged => "unchanged",
        Action::Duplicate => "duplicate",
        Action::Unresolved => "unresolved",
    }
}

fn confidence_label(c: Confidence) -> &'static str {
    match c {
        Confidence::High => "",
        Confidence::Medium => "medium",
        Confidence::Low => "low",
    }
}

fn file_name(dest: &str) -> &str {
    dest.rsplit('/').next().unwrap_or(dest)
}

#[allow(clippy::too_many_lines)]
fn print_text(
    plan: &ImportPlan,
    report: Option<&ApplyReport>,
    mapping_saved: bool,
    args: &ScreenshotsArgs,
    global: &Global,
) {
    let st = Style::stdout();
    if !global.quiet {
        println!(
            "{} {} ({})",
            st.bold("Import screenshots from"),
            plan.source_dir.display(),
            plan.format
        );
        // The mapping in effect (project + options + detected).
        let m = &plan.mapping;
        let list = |pairs: Vec<String>| pairs.join(", ");
        if !m.devices.is_empty() {
            println!(
                "  devices: {}",
                list(
                    m.devices
                        .iter()
                        .map(|(d, v)| format!("{d} → {v}"))
                        .collect()
                )
            );
        }
        if !m.locales.is_empty() {
            println!(
                "  locales: {}",
                list(
                    m.locales
                        .iter()
                        .map(|(l, v)| format!("{l} → {v}"))
                        .collect()
                )
            );
        }
        if !m.keys.is_empty() {
            println!(
                "  keys:    {}",
                list(m.keys.iter().map(|(s, k)| format!("{s} → {k}")).collect())
            );
        }
    }

    // Planned files grouped by language · variant.
    let mut groups: BTreeMap<(String, String), Vec<&PlannedFile>> = BTreeMap::new();
    let mut unresolved: Vec<&PlannedFile> = Vec::new();
    for it in &plan.items {
        match (&it.lang, &it.dest) {
            (Some(l), Some(_)) => groups
                .entry((
                    l.clone(),
                    it.variant
                        .clone()
                        .unwrap_or_else(|| "(all variants)".into()),
                ))
                .or_default()
                .push(it),
            _ => unresolved.push(it),
        }
    }
    let src_w = plan
        .items
        .iter()
        .map(|i| i.source.chars().count())
        .max()
        .unwrap_or(0)
        .min(60);
    let line = |it: &PlannedFile| {
        let label = format!("{:<10}", action_label(it.action));
        let label = match it.action {
            Action::New => st.ok(&label),
            Action::Overwrite | Action::Duplicate => st.warn(&label),
            Action::Unresolved => st.error(&label),
            Action::Unchanged => st.dim(&label),
        };
        let dest = it.dest.as_deref().map_or("?", file_name);
        let conf = confidence_label(it.confidence);
        let row = format!(
            "  {label} {dest:<24} ← {:<src_w$}  {}",
            it.source,
            st.dim(conf)
        );
        println!("{}", row.trim_end());
        let show_notes =
            global.verbose || it.confidence < Confidence::High || it.action != Action::New;
        if show_notes && !global.quiet {
            let notes: Vec<&String> = if global.verbose || it.action == Action::Unresolved {
                it.notes.iter().collect()
            } else {
                // Only the notes that explain a doubt or conflict.
                it.notes
                    .iter()
                    .filter(|n| {
                        n.contains("similar")
                            || n.contains("ambiguous")
                            || n.contains("aspect")
                            || n.contains("not used")
                            || n.contains("default language")
                            || n.contains("replaces")
                            || n.contains("same destination")
                            || n.contains("not a known")
                    })
                    .collect()
            };
            for n in notes {
                println!("  {:<10} {}", "", st.dim(n));
            }
        }
    };
    for ((lang, variant), items) in &groups {
        println!("\n{} · {}", st.bold(lang), variant);
        for it in items {
            line(it);
        }
    }
    if !unresolved.is_empty() {
        println!("\n{}", st.error("unresolved"));
        for it in &unresolved {
            line(it);
        }
    }
    if !plan.skipped.is_empty() && !global.quiet {
        println!("\n{}", st.bold("skipped"));
        for s in &plan.skipped {
            println!("  {:<src_w$}  {}", s.source, st.dim(&s.reason));
        }
    }

    // Still missing after the import.
    let mut missing: BTreeMap<(String, String), Vec<&str>> = BTreeMap::new();
    let mut fallback: BTreeMap<String, usize> = BTreeMap::new();
    for m in &plan.missing {
        match m.status {
            MissingStatus::Missing => missing
                .entry((m.lang.clone(), m.variant.clone()))
                .or_default()
                .push(&m.key),
            MissingStatus::Fallback => *fallback.entry(m.lang.clone()).or_default() += 1,
        }
    }
    if !global.quiet && (!missing.is_empty() || !fallback.is_empty()) {
        println!("\n{}", st.bold("after the import"));
        for ((lang, variant), keys) in &missing {
            println!(
                "  {} {lang} · {variant}: {}",
                st.warn("missing "),
                keys.join(", ")
            );
        }
        for (lang, n) in &fallback {
            println!(
                "  {} {lang}: {} from a fallback language",
                st.info("fallback"),
                plural(*n, "screenshot")
            );
        }
    }

    let counts = [
        Action::New,
        Action::Overwrite,
        Action::Unchanged,
        Action::Duplicate,
        Action::Unresolved,
    ]
    .iter()
    .filter_map(|a| {
        let n = plan.count(*a);
        (n > 0).then(|| format!("{n} {}", action_label(*a)))
    })
    .collect::<Vec<_>>();
    println!(
        "\n{}{}",
        if counts.is_empty() {
            "no screenshots found".to_owned()
        } else {
            counts.join(", ")
        },
        if plan.skipped.is_empty() {
            String::new()
        } else {
            format!(", {} skipped", plural(plan.skipped.len(), "file"))
        }
    );
    match report {
        None => println!(
            "{}",
            st.dim("dry run: nothing copied (run without --dry-run to import)")
        ),
        Some(r) => {
            println!("{} {}", st.ok("copied"), plural(r.copied.len(), "file"));
            for (dest, e) in &r.failed {
                println!("{} {dest}: {e}", st.error("failed"));
            }
        }
    }
    if mapping_saved {
        println!("mapping saved to project.yaml (import.fastlane)");
    } else if args.save_mapping && args.dry_run {
        println!("{}", st.dim("--save-mapping ignored in a dry run"));
    }
}
