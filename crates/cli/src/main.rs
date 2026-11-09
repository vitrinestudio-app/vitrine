//! `arrangia` – command line interface.
//!
//! Every command goes through the operations API in `arrangia-core`; the CLI
//! never edits project fields directly. All commands are stubs for now.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};

/// Create App Store and Play Store screenshots from a single layout.
#[derive(Debug, Parser)]
#[command(name = "arrangia", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Create a new project folder.
    New {
        /// Path of the project folder to create.
        path: PathBuf,
    },
    /// Render a preview of a page or strip.
    Render {
        /// Path of the project folder.
        #[arg(long, default_value = ".")]
        project: PathBuf,
    },
    /// Export pages, banners or gallery previews for targets and languages.
    Export {
        /// Path of the project folder.
        #[arg(long, default_value = ".")]
        project: PathBuf,
    },
    /// Validate a project and report warnings and errors.
    Validate {
        /// Path of the project folder.
        #[arg(long, default_value = ".")]
        project: PathBuf,
    },
}

impl Command {
    fn name(&self) -> &'static str {
        match self {
            Self::New { .. } => "new",
            Self::Render { .. } => "render",
            Self::Export { .. } => "export",
            Self::Validate { .. } => "validate",
        }
    }
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    eprintln!("arrangia {}: not yet implemented", cli.command.name());
    ExitCode::FAILURE
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
    }
}
