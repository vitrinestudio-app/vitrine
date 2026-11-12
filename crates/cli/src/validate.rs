//! `vitrine validate`: the `core::validate` report, grouped by target and
//! language (text), as JSON, or in a CI format (`github` annotations,
//! `gitlab` Code Quality, `junit` XML; `crate::ci_report`).

use std::path::PathBuf;

use anyhow::Context as _;
use clap::{Args, ValueEnum};
use serde::Serialize;
use vitrine_core::resolve::FsAssets;
use vitrine_core::validate::{
    Finding, Severity, ValidateMode, ValidateOptions, ValidationReport, validate,
};

use crate::ci_report::{self, Locator};
use crate::term::{Style, plural};
use crate::{EXIT_OK, EXIT_PROBLEMS, Global, project};

/// Output format of `validate`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ValidateFormat {
    /// Grouped by target and language.
    Text,
    /// The report with counts.
    Json,
    /// GitHub Actions annotations (workflow commands) plus the text report.
    Github,
    /// GitLab Code Quality report (JSON).
    Gitlab,
    /// JUnit XML (GitLab, Jenkins, Azure Pipelines, CircleCI, Bitrise).
    Junit,
}

#[derive(Debug, Args)]
pub struct ValidateArgs {
    /// Only this target; repeat for more (default: all targets).
    #[arg(long = "target", value_name = "ID")]
    pub targets: Vec<String>,
    /// Only this language; repeat for more (default: each target's languages).
    #[arg(long = "lang", value_name = "LANG")]
    pub langs: Vec<String>,
    /// Output format (`github`, `gitlab`, `junit`: CI reports).
    #[arg(long, value_enum, default_value_t = ValidateFormat::Text)]
    pub format: ValidateFormat,
    /// Write the `--format` report to this file and print the text report
    /// (e.g. `--format junit -o validate.xml`).
    #[arg(short, long, value_name = "FILE")]
    pub output: Option<PathBuf>,
    /// Validate for previewing: missing screenshots are warnings, not errors.
    #[arg(long)]
    pub preview: bool,
    /// Fail (exit code 1) on warnings too.
    #[arg(long)]
    pub strict: bool,
}

/// JSON output: the report plus counts.
#[derive(Serialize)]
struct JsonReport<'a> {
    ok: bool,
    errors: usize,
    warnings: usize,
    infos: usize,
    findings: &'a [Finding],
}

pub fn run(args: &ValidateArgs, global: &Global) -> anyhow::Result<u8> {
    let root = &global.project;
    let project = project::load(root)?;
    project::check_targets(&project, &args.targets)?;
    project::check_langs(&args.langs)?;
    let fs = FsAssets::new(root);
    let frames = crate::frames::installed(root);
    let mut opts = ValidateOptions::new(&fs);
    opts.frames = frames.as_deref();
    opts.fonts = project::fonts(root, global.quiet);
    opts.targets = (!args.targets.is_empty()).then(|| args.targets.clone());
    opts.langs = (!args.langs.is_empty()).then(|| args.langs.clone());
    opts.mode = if args.preview {
        ValidateMode::Preview
    } else {
        ValidateMode::Export
    };
    let report = validate(&project, &opts);
    let errors = report.count(Severity::Error);
    let warnings = report.count(Severity::Warn);
    let failed = errors > 0 || (args.strict && warnings > 0);

    let checked: Vec<String> = project
        .targets
        .iter()
        .map(|t| t.id.to_string())
        .filter(|t| args.targets.is_empty() || args.targets.contains(t))
        .collect();
    let formatted = match args.format {
        ValidateFormat::Text => None,
        ValidateFormat::Json => {
            let json = JsonReport {
                ok: !failed,
                errors,
                warnings,
                infos: report.count(Severity::Info),
                findings: &report.findings,
            };
            Some(serde_json::to_string_pretty(&json)? + "\n")
        }
        ValidateFormat::Github => Some(ci_report::github(
            &report.findings,
            &Locator::new(root),
            global.verbose,
        )),
        ValidateFormat::Gitlab => Some(
            serde_json::to_string_pretty(&ci_report::gitlab(
                &report.findings,
                &Locator::new(root),
            ))? + "\n",
        ),
        ValidateFormat::Junit => Some(ci_report::junit(
            &report.findings,
            &checked,
            &Locator::new(root),
            args.strict,
        )),
    };
    match (formatted, &args.output) {
        (None, Some(_)) => {
            anyhow::bail!("--output needs a report format (--format json|github|gitlab|junit)")
        }
        (None, None) => print_text(&report, global),
        (Some(out), Some(file)) => {
            std::fs::write(file, out)
                .with_context(|| format!("cannot write {}", file.display()))?;
            print_text(&report, global);
        }
        (Some(out), None) => {
            print!("{out}");
            // GitHub: the annotations plus the readable report in the log.
            if args.format == ValidateFormat::Github {
                print_text(&report, global);
            }
        }
    }
    Ok(if failed { EXIT_PROBLEMS } else { EXIT_OK })
}

/// Group heading of a finding: `target (lang)`, `target` or `project`.
fn group(f: &Finding) -> String {
    match (&f.target, &f.lang) {
        (Some(t), Some(l)) => format!("{t} ({l})"),
        (Some(t), None) => t.to_string(),
        (None, Some(l)) => format!("({l})"),
        (None, None) => "project".to_owned(),
    }
}

fn print_text(report: &ValidationReport, global: &Global) {
    let style = Style::stdout();
    // Groups in order of first appearance.
    let mut groups: Vec<(String, Vec<&Finding>)> = Vec::new();
    for f in &report.findings {
        if global.quiet && f.level == Severity::Info {
            continue;
        }
        let key = group(f);
        match groups.iter_mut().find(|(k, _)| *k == key) {
            Some((_, list)) => list.push(f),
            None => groups.push((key, vec![f])),
        }
    }
    for (key, findings) in &groups {
        println!("{}", style.bold(key));
        for f in findings {
            let mut place = Vec::new();
            if let Some(p) = f.page {
                place.push(format!("page {}", p + 1));
            }
            if let Some(e) = &f.element {
                place.push(format!("`{e}`"));
            }
            let place = if place.is_empty() {
                String::new()
            } else {
                format!("{} ", place.join(" "))
            };
            println!(
                "  {} {} {place}{}",
                style.level(f.level),
                style.dim(&format!("{:<20}", f.code.as_str())),
                f.message
            );
        }
    }
    if global.quiet {
        return;
    }
    let errors = report.count(Severity::Error);
    let warnings = report.count(Severity::Warn);
    let infos = report.count(Severity::Info);
    let counts = format!(
        "{}, {}, {}",
        plural(errors, "error"),
        plural(warnings, "warning"),
        plural(infos, "info")
    );
    if errors > 0 {
        println!("{} {counts}", style.error("Invalid:"));
    } else if report.is_empty() {
        println!("{} no findings", style.ok("OK:"));
    } else {
        println!("{} {counts}", style.ok("OK:"));
    }
}
