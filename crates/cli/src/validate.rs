//! `vitrine validate`: the `core::validate` report, grouped by target and
//! language (text) or as JSON.

use clap::Args;
use serde::Serialize;
use vitrine_core::resolve::FsAssets;
use vitrine_core::validate::{
    Finding, Severity, ValidateMode, ValidateOptions, ValidationReport, validate,
};

use crate::term::{Style, plural};
use crate::{EXIT_OK, EXIT_PROBLEMS, Format, Global, project};

#[derive(Debug, Args)]
pub struct ValidateArgs {
    /// Only this target; repeat for more (default: all targets).
    #[arg(long = "target", value_name = "ID")]
    pub targets: Vec<String>,
    /// Only this language; repeat for more (default: each target's languages).
    #[arg(long = "lang", value_name = "LANG")]
    pub langs: Vec<String>,
    /// Output format.
    #[arg(long, value_enum, default_value_t = Format::Text)]
    pub format: Format,
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

    match args.format {
        Format::Json => {
            let json = JsonReport {
                ok: !failed,
                errors,
                warnings,
                infos: report.count(Severity::Info),
                findings: &report.findings,
            };
            println!("{}", serde_json::to_string_pretty(&json)?);
        }
        Format::Text => print_text(&report, global),
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
