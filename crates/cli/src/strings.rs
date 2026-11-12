//! `vitrine strings export|import`: translation exchange with CSV and
//! XLIFF 1.2/2.0 (`core::exchange`).

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use anyhow::{Context, bail};
use clap::{Args, Subcommand, ValueEnum};
use vitrine_core::exchange::{
    self, ChangeKind, Delimiter, EmptyCells, ExportOptions, ImportInput, ImportOptions, ImportPlan,
    MarkupMode,
};
use vitrine_core::resolve::FsAssets;
use vitrine_core::validate::{Severity, ValidateMode, ValidateOptions};

use crate::term::{Style, plural};
use crate::{EXIT_OK, EXIT_PROBLEMS, Global, project};

#[derive(Debug, Args)]
pub struct StringsArgs {
    #[command(subcommand)]
    pub command: StringsCommand,
}

#[derive(Debug, Subcommand)]
pub enum StringsCommand {
    /// Export translations for translators (CSV, XLIFF 1.2 or 2.0).
    Export(ExportArgs),
    /// Import translated CSV/XLIFF files (prints a report; see --dry-run).
    Import(ImportArgs),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ExchangeFormat {
    Csv,
    Xliff12,
    Xliff20,
}

impl From<ExchangeFormat> for exchange::Format {
    fn from(f: ExchangeFormat) -> Self {
        match f {
            ExchangeFormat::Csv => Self::Csv,
            ExchangeFormat::Xliff12 => Self::Xliff12,
            ExchangeFormat::Xliff20 => Self::Xliff20,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum CsvDelimiter {
    Comma,
    Semicolon,
    Tab,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum EmptyArg {
    /// Empty cells change nothing.
    Keep,
    /// Empty cells remove the translation (fallback again).
    Remove,
}

#[derive(Debug, Args)]
#[allow(clippy::struct_excessive_bools)] // independent CLI flags
pub struct ExportArgs {
    /// File format.
    #[arg(long, value_enum, default_value_t = ExchangeFormat::Xliff20)]
    pub format: ExchangeFormat,
    /// Target language; repeat for more (default: all but the default language).
    #[arg(long = "lang", value_name = "LANG")]
    pub langs: Vec<String>,
    /// Output file or folder (default: the current folder). XLIFF writes
    /// one file per language; with a file name `x.xlf` and several
    /// languages they become `x-<lang>.xlf`.
    #[arg(long, short, value_name = "PATH")]
    pub out: Option<PathBuf>,
    /// Only keys missing in a target language.
    #[arg(long)]
    pub missing_only: bool,
    /// Add context for translators (elements, pages, approximate max length).
    #[arg(long)]
    pub context: bool,
    /// CSV delimiter (`semicolon` for Excel in locales with a decimal comma).
    #[arg(long, value_enum, default_value_t = CsvDelimiter::Comma)]
    pub delimiter: CsvDelimiter,
    /// CSV: write a UTF-8 byte order mark (Excel).
    #[arg(long)]
    pub bom: bool,
    /// XLIFF: keep Vitrine Studio markup (`**bold**`, `[accent]…[/]`) as text
    /// instead of inline elements.
    #[arg(long)]
    pub plain_markup: bool,
    /// Print the written files as JSON.
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Args)]
#[allow(clippy::struct_excessive_bools)] // independent CLI flags
pub struct ImportArgs {
    /// CSV or XLIFF files (format from the extension or content).
    #[arg(required = true, value_name = "FILE")]
    pub files: Vec<PathBuf>,
    /// Only print the report; change nothing.
    #[arg(long)]
    pub dry_run: bool,
    /// Create keys that are not in the project (else they are reported).
    #[arg(long)]
    pub create_keys: bool,
    /// Meaning of empty CSV cells.
    #[arg(long, value_enum, default_value_t = EmptyArg::Keep)]
    pub empty: EmptyArg,
    /// Skip the layout check (overflow, emergency breaks) after the import.
    #[arg(long)]
    pub no_layout_check: bool,
    /// Print the report as JSON.
    #[arg(long)]
    pub json: bool,
}

pub fn run(args: &StringsArgs, global: &Global) -> anyhow::Result<u8> {
    match &args.command {
        StringsCommand::Export(a) => run_export(a, global),
        StringsCommand::Import(a) => run_import(a, global),
    }
}

fn run_export(args: &ExportArgs, global: &Global) -> anyhow::Result<u8> {
    let p = project::load(&global.project)?;
    let opts = ExportOptions {
        langs: args.langs.clone(),
        missing_only: args.missing_only,
        context: args.context,
        delimiter: match args.delimiter {
            CsvDelimiter::Comma => Delimiter::Comma,
            CsvDelimiter::Semicolon => Delimiter::Semicolon,
            CsvDelimiter::Tab => Delimiter::Tab,
        },
        bom: args.bom,
        markup: if args.plain_markup {
            MarkupMode::Plain
        } else {
            MarkupMode::Inline
        },
    };
    let files = exchange::export(&p, args.format.into(), &opts)?;
    let out = args.out.clone().unwrap_or_else(|| PathBuf::from("."));
    let is_dir = out.is_dir() || out.as_os_str().to_string_lossy().ends_with(['/', '\\']);
    let paths = exchange::output_paths(&files, &out, is_dir);
    for (f, path) in files.iter().zip(&paths) {
        if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
            std::fs::create_dir_all(dir)
                .with_context(|| format!("cannot create {}", dir.display()))?;
        }
        std::fs::write(path, &f.content)
            .with_context(|| format!("cannot write {}", path.display()))?;
    }
    if args.json {
        let list: Vec<_> = files
            .iter()
            .zip(&paths)
            .map(|(f, path)| serde_json::json!({ "path": path, "file": f }))
            .collect();
        println!("{}", serde_json::to_string_pretty(&list)?);
    } else if !global.quiet {
        let style = Style::stdout();
        for (f, path) in files.iter().zip(&paths) {
            let mut line = format!(
                "{} {} ({})",
                style.ok("wrote"),
                path.display(),
                plural(f.keys, "key")
            );
            if !f.skipped.is_empty() {
                let _ = write!(
                    line,
                    ", {} without default-language text skipped",
                    plural(f.skipped.len(), "key")
                );
            }
            println!("{line}");
        }
    }
    Ok(EXIT_OK)
}

fn read_inputs(files: &[PathBuf]) -> anyhow::Result<Vec<ImportInput>> {
    files
        .iter()
        .map(|f| {
            let data = std::fs::read(f).with_context(|| format!("cannot read {}", f.display()))?;
            Ok(ImportInput {
                name: f.display().to_string(),
                data,
            })
        })
        .collect()
}

fn run_import(args: &ImportArgs, global: &Global) -> anyhow::Result<u8> {
    let root: &Path = &global.project;
    let mut p = project::load(root)?;
    let inputs = read_inputs(&args.files)?;
    let opts = ImportOptions {
        create_keys: args.create_keys,
        empty: match args.empty {
            EmptyArg::Keep => EmptyCells::Keep,
            EmptyArg::Remove => EmptyCells::Remove,
        },
    };
    let mut plan = exchange::plan_import(&p, &inputs, &opts);
    if !args.no_layout_check {
        let fs = FsAssets::new(root);
        let mut v = ValidateOptions::new(&fs);
        v.fonts = project::fonts(root, &p, global);
        v.mode = ValidateMode::Preview;
        exchange::check_layout(&mut plan, &p, &v);
    }
    let applied = !args.dry_run && !plan.is_empty();
    if applied {
        let ops = plan.ops().map_err(|e| anyhow::anyhow!(e))?;
        p.apply_ops(ops).context("cannot apply the import")?;
        p.save(root).context("cannot save the project")?;
    }
    if args.json {
        let json = serde_json::json!({ "applied": applied, "plan": &plan });
        println!("{}", serde_json::to_string_pretty(&json)?);
    } else {
        print_report(&plan, args.dry_run, applied, global);
    }
    let failed = plan.files.iter().any(|f| f.error.is_some())
        || plan
            .layout
            .iter()
            .any(|f| f.new && f.finding.level == Severity::Error);
    if plan.files.iter().all(|f| f.error.is_some()) {
        bail!("no file could be read");
    }
    Ok(if failed { EXIT_PROBLEMS } else { EXIT_OK })
}

fn one_line(s: &str) -> String {
    s.replace('\n', "⏎")
}

#[allow(clippy::too_many_lines)]
fn print_report(plan: &ImportPlan, dry_run: bool, applied: bool, global: &Global) {
    let style = Style::stdout();
    for f in &plan.files {
        match &f.error {
            Some(e) => println!("{} {}: {e}", style.error("error"), f.name),
            None => println!(
                "{} {} ({}, {}, {})",
                style.bold("file"),
                f.name,
                f.format.map_or("?", exchange::Format::as_str),
                f.langs.join(", "),
                plural(f.entries, "value")
            ),
        }
        for c in &f.unknown_columns {
            println!("  {} unknown column `{c}` ignored", style.warn("warn "));
        }
        for w in &f.warnings {
            println!("  {} {w}", style.warn("warn "));
        }
    }
    for l in &plan.langs {
        println!(
            "{} {}: {} new, {} changed, {} removed, {} unchanged, {} skipped",
            style.bold("lang"),
            l.lang,
            l.new.len(),
            l.changed.len(),
            l.removed.len(),
            l.unchanged,
            l.skipped.len()
        );
    }
    if !global.quiet {
        for c in &plan.changes {
            let (mark, text) = match c.kind {
                ChangeKind::New => (
                    style.ok("+"),
                    one_line(c.new.as_deref().unwrap_or_default()),
                ),
                ChangeKind::Changed => (
                    style.warn("~"),
                    format!(
                        "{} → {}",
                        style.dim(&one_line(c.old.as_deref().unwrap_or_default())),
                        one_line(c.new.as_deref().unwrap_or_default())
                    ),
                ),
                ChangeKind::Removed => (
                    style.error("-"),
                    style.dim(&one_line(c.old.as_deref().unwrap_or_default())),
                ),
            };
            println!("  {mark} {} ({}): {text}", c.key, c.lang);
        }
    }
    for k in &plan.unknown_keys {
        println!(
            "{} unknown key `{}` in {} (use --create-keys to add it)",
            style.warn("warn "),
            k.key,
            k.file
        );
    }
    for i in &plan.issues {
        let level = if i.kind == exchange::IssueKind::DiffersFromSource {
            style.info("info ")
        } else {
            style.warn("warn ")
        };
        if global.quiet && i.kind == exchange::IssueKind::DiffersFromSource {
            continue;
        }
        println!("{level} {} ({}): {}", i.key, i.lang, i.message);
    }
    for f in &plan.layout {
        let target = f
            .finding
            .target
            .as_ref()
            .map(ToString::to_string)
            .unwrap_or_default();
        let lang = f
            .finding
            .lang
            .as_ref()
            .map(ToString::to_string)
            .unwrap_or_default();
        println!(
            "{} {} {target} ({lang}){}: {}",
            style.level(f.finding.level),
            style.dim(&format!("{:<15}", f.finding.code.as_str())),
            if f.new { "" } else { " [already before]" },
            f.finding.message
        );
    }
    let n = plan.changes.len();
    if n == 0 {
        println!("{} nothing to change", style.ok("OK:"));
    } else if dry_run {
        println!(
            "{} {} (dry run, nothing written)",
            style.info("Plan:"),
            plural(n, "change")
        );
    } else if applied {
        println!("{} {} applied", style.ok("Imported:"), plural(n, "change"));
    }
}
