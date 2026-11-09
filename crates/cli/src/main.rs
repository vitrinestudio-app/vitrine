//! `arrangia` – command line interface.
//!
//! Commands: `new` (project from a template), `render` (preview/debug PNG),
//! `export` (all files of targets × languages) and `validate`. The CLI only
//! formats what `arrangia-core` and `arrangia-render` compute; it never
//! edits project fields directly.
//!
//! Exit codes (all commands):
//!
//! | code | meaning |
//! |---|---|
//! | 0 | success; `validate`: no errors (warnings only fail with `--strict`) |
//! | 1 | the project has problems: validation errors, export failures |
//! | 2 | the command could not run: usage error (clap), project not loadable, IO error |

mod export;
mod new;
mod project;
mod render;
mod term;
mod validate;

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Args, Parser, Subcommand, ValueEnum};

/// Exit code: success.
pub const EXIT_OK: u8 = 0;
/// Exit code: the project has errors (validation errors, export failures).
pub const EXIT_PROBLEMS: u8 = 1;
/// Exit code: the command could not run (same as clap's usage errors).
pub const EXIT_FAILURE: u8 = 2;

/// Create App Store and Play Store screenshots from a single layout.
#[derive(Debug, Parser)]
#[command(name = "arrangia", version, about)]
struct Cli {
    #[command(flatten)]
    global: Global,
    #[command(subcommand)]
    command: Command,
}

/// Options shared by all commands.
#[derive(Debug, Args)]
pub struct Global {
    /// Project folder (the folder containing `project.yaml`).
    #[arg(long, global = true, default_value = ".", value_name = "DIR")]
    pub project: PathBuf,
    /// Print only results and errors.
    #[arg(short, long, global = true, conflicts_with = "verbose")]
    pub quiet: bool,
    /// Print more details (every exported file, font loading).
    #[arg(short, long, global = true)]
    pub verbose: bool,
}

/// Output format of `validate` and `export`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Format {
    Text,
    Json,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Create a new project folder from a template.
    New(new::NewArgs),
    /// Render a preview PNG of a page, output image, canvas or store gallery.
    Render(render::RenderArgs),
    /// Export pages, banners and gallery previews for targets and languages.
    Export(export::ExportArgs),
    /// Validate a project and report infos, warnings and errors.
    Validate(validate::ValidateArgs),
}

impl Command {
    #[cfg(test)]
    fn name(&self) -> &'static str {
        match self {
            Self::New(_) => "new",
            Self::Render(_) => "render",
            Self::Export(_) => "export",
            Self::Validate(_) => "validate",
        }
    }
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let result = match &cli.command {
        Command::New(args) => new::run(args, &cli.global),
        Command::Render(args) => render::run(args, &cli.global),
        Command::Export(args) => export::run(args, &cli.global),
        Command::Validate(args) => validate::run(args, &cli.global),
    };
    match result {
        Ok(code) => ExitCode::from(code),
        Err(e) => {
            let style = term::Style::stderr();
            eprintln!("{} {e:#}", style.error("error:"));
            ExitCode::from(EXIT_FAILURE)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn cli_definition_is_valid() {
        Cli::command().debug_assert();
    }

    #[test]
    fn parses_validate_subcommand() {
        let cli = Cli::try_parse_from(["arrangia", "validate", "--project", "demo"]).unwrap();
        assert_eq!(cli.command.name(), "validate");
        assert_eq!(cli.global.project, PathBuf::from("demo"));
    }

    #[test]
    fn project_is_global() {
        let cli = Cli::try_parse_from(["arrangia", "--project", "demo", "export"]).unwrap();
        assert_eq!(cli.command.name(), "export");
        assert_eq!(cli.global.project, PathBuf::from("demo"));
    }

    #[test]
    fn render_selection_is_exclusive() {
        assert!(Cli::try_parse_from(["arrangia", "render", "--page", "1", "--canvas"]).is_err());
        assert!(
            Cli::try_parse_from(["arrangia", "render", "--width", "1", "--scale", "1"]).is_err()
        );
        assert!(Cli::try_parse_from(["arrangia", "render", "--canvas", "--scale", "0.5"]).is_ok());
    }
}
