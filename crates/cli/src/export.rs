//! `arrangia export`: all files of targets × languages, with a summary
//! table and the issues found while resolving and rendering.

use std::path::PathBuf;

use arrangia_core::resolve::Level;
use arrangia_render::export::{ExportOptions, ExportReport, IssueSource, export};
use clap::Args;

use crate::term::{Style, plural};
use crate::{EXIT_OK, EXIT_PROBLEMS, Format, Global, project};

#[derive(Debug, Args)]
pub struct ExportArgs {
    /// Only this target; repeat for more (default: all targets).
    #[arg(long = "target", value_name = "ID")]
    pub targets: Vec<String>,
    /// Only this language; repeat for more (default: each target's languages).
    #[arg(long = "lang", value_name = "LANG")]
    pub langs: Vec<String>,
    /// Export folder (default: `<project>/export`).
    #[arg(long, value_name = "DIR")]
    pub out: Option<PathBuf>,
    /// Do not write `_gallery.png` store gallery previews.
    #[arg(long)]
    pub no_gallery: bool,
    /// Render every file, ignoring the incremental export cache.
    #[arg(long)]
    pub force: bool,
    /// Output format (`json`: the full export report).
    #[arg(long, value_enum, default_value_t = Format::Text)]
    pub format: Format,
}

pub fn run(args: &ExportArgs, global: &Global) -> anyhow::Result<u8> {
    let root = &global.project;
    let project = project::load(root)?;
    project::check_targets(&project, &args.targets)?;
    project::check_langs(&args.langs)?;
    let (ctx, fonts) = crate::frames::render_context(root);
    if !global.quiet
        && let Some(r) = &fonts
    {
        project::report_font_errors(&r.errors);
    }
    let report = export(
        &project,
        &ctx,
        &ExportOptions {
            out_dir: args.out.clone(),
            targets: args.targets.clone(),
            langs: args.langs.clone(),
            gallery: !args.no_gallery,
            incremental: !args.force,
            ..ExportOptions::default()
        },
    );
    match args.format {
        Format::Json => println!("{}", serde_json::to_string_pretty(&report)?),
        Format::Text => print_text(&report, global),
    }
    Ok(if report.errors.is_empty() {
        EXIT_OK
    } else {
        EXIT_PROBLEMS
    })
}

#[derive(Default)]
struct Row {
    target: String,
    lang: String,
    written: usize,
    skipped: usize,
    ms: f64,
}

fn rows(report: &ExportReport) -> Vec<Row> {
    let mut rows: Vec<Row> = Vec::new();
    for f in &report.files {
        let i = if let Some(i) = rows
            .iter()
            .position(|r| r.target == f.target && r.lang == f.lang)
        {
            i
        } else {
            rows.push(Row {
                target: f.target.clone(),
                lang: f.lang.clone(),
                ..Row::default()
            });
            rows.len() - 1
        };
        let row = &mut rows[i];
        if f.skipped {
            row.skipped += 1;
        } else {
            row.written += 1;
        }
        row.ms += f.render_ms + f.encode_ms;
    }
    rows
}

fn print_text(report: &ExportReport, global: &Global) {
    let style = Style::stdout();
    let rows = rows(report);
    if !global.quiet && !rows.is_empty() {
        let tw = rows
            .iter()
            .map(|r| r.target.len())
            .max()
            .unwrap_or(0)
            .max(6);
        let lw = rows.iter().map(|r| r.lang.len()).max().unwrap_or(0).max(4);
        println!(
            "{}",
            style.bold(&format!(
                "{:tw$}  {:lw$}  {:>7}  {:>9}  {:>8}",
                "target", "lang", "written", "unchanged", "cpu ms"
            ))
        );
        for r in &rows {
            println!(
                "{:tw$}  {:lw$}  {:>7}  {:>9}  {:>8.0}",
                r.target, r.lang, r.written, r.skipped, r.ms
            );
        }
    }
    if global.verbose {
        for f in &report.files {
            let state = if f.skipped { "unchanged" } else { "written" };
            println!(
                "  {}  {}x{}  {} B  {}",
                f.path,
                f.width,
                f.height,
                f.bytes,
                style.dim(state)
            );
        }
    }
    if !global.quiet {
        for i in &report.issues {
            let (level, message) = match &i.source {
                IssueSource::Resolve(d) => (
                    match d.level {
                        Level::Info => style.info("info "),
                        Level::Warn => style.warn("warn "),
                    },
                    d.message.clone(),
                ),
                IssueSource::Render(r) => (style.warn("warn "), r.to_string()),
            };
            println!(
                "{level} {} {message}",
                style.dim(&format!("[{}/{}]", i.target, i.lang))
            );
        }
    }
    for e in &report.errors {
        println!("{} {e}", style.error("error"));
    }
    if !global.quiet {
        let written = report.written().count();
        let skipped = report.files.len() - written;
        let summary = format!(
            "{} written, {} unchanged → {} ({:.0} ms)",
            plural(written, "file"),
            skipped,
            report.out_dir.display(),
            report.total_ms
        );
        if report.errors.is_empty() {
            println!("{} {summary}", style.ok("Exported:"));
        } else {
            println!(
                "{} {summary}; {}",
                style.error("Export failed:"),
                plural(report.errors.len(), "error")
            );
        }
    }
}
