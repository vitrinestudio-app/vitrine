//! Loading the project and shared helpers.

use std::path::Path;

use anyhow::{Context, bail};
use vitrine_core::Project;
use vitrine_core::io::PROJECT_FILE;
use vitrine_core::layout::FontLibrary;

/// Loads the project in `dir` with a helpful message if there is none.
pub fn load(dir: &Path) -> anyhow::Result<Project> {
    if !dir.join(PROJECT_FILE).is_file() {
        bail!(
            "no {PROJECT_FILE} in `{}` (pass --project <DIR> or create a project with `vitrine new <DIR>`)",
            dir.display()
        );
    }
    Project::load(dir).context("cannot load project")
}

/// Bundled fonts, the project's `fonts/` folder (with stored system
/// fonts) and – unless `--no-system-fonts` – the installed system fonts
/// the project needs; unreadable font files are reported on stderr unless
/// `--quiet`.
pub fn fonts(dir: &Path, project: &Project, global: &crate::Global) -> FontLibrary {
    let (fonts, report) =
        vitrine_core::layout::setup::project_library(dir, Some(project), global.system_fonts());
    if !global.quiet {
        report_font_errors(&report.errors);
    }
    fonts
}

pub fn report_font_errors(errors: &[vitrine_core::layout::FontError]) {
    let style = crate::term::Style::stderr();
    for e in errors {
        eprintln!("{} {e}", style.warn("warning:"));
    }
}

/// Checks that every id in `targets` names a target of the project.
pub fn check_targets(project: &Project, targets: &[String]) -> anyhow::Result<()> {
    for t in targets {
        if project.target(t).is_none() {
            let known: Vec<&str> = project.targets.iter().map(|t| t.id.as_str()).collect();
            bail!(
                "unknown target `{t}` (project targets: {})",
                if known.is_empty() {
                    "none".to_owned()
                } else {
                    known.join(", ")
                }
            );
        }
    }
    Ok(())
}

/// Checks that every entry in `langs` is a valid language code.
pub fn check_langs(langs: &[String]) -> anyhow::Result<()> {
    for l in langs {
        vitrine_core::model::Lang::parse(l)
            .map_err(|e| anyhow::anyhow!("invalid language `{l}`: {e}"))?;
    }
    Ok(())
}
