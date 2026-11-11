//! `vitrine install-cli [--remove] [--status] [--yes]`: makes this CLI
//! available as `vitrine` in terminals (ADR 0026) – macOS symlink in
//! `/usr/local/bin` (password prompt when needed), Windows user `PATH`
//! (the NSIS installer runs `install-cli --yes`), Linux symlink in
//! `~/.local/bin`; deb/rpm installs are already on `PATH`. Same code as
//! the app's "Install command line tool" (`vitrine_app::cli_install`).

use std::io::{BufRead, IsTerminal, Write};

use anyhow::{Context, bail};
use clap::Args;
use vitrine_app::cli_install::{self, Method, Status};

use crate::{EXIT_OK, Format, Global};

#[derive(Debug, Args)]
pub struct InstallCliArgs {
    /// Remove the symlink / PATH entry instead.
    #[arg(long, conflicts_with = "status")]
    remove: bool,
    /// Only show the current state.
    #[arg(long)]
    status: bool,
    /// Do not ask for confirmation.
    #[arg(long, short = 'y')]
    yes: bool,
    /// Output format.
    #[arg(long, value_enum, default_value_t = Format::Text)]
    format: Format,
}

pub fn run(args: &InstallCliArgs, global: &Global) -> anyhow::Result<u8> {
    let cli = vitrine_app::claude_desktop::cli_path();
    let before = cli_install::status(&cli, None);
    if args.status {
        print(&before, args.format, global)?;
        return Ok(EXIT_OK);
    }
    let changes = before.installed == args.remove
        && matches!(before.method, Method::Symlink | Method::UserPath);
    if changes && !args.yes {
        confirm(&before, args.remove)?;
    }
    let after = cli_install::apply(&cli, None, args.remove, true)
        .map_err(|e| anyhow::anyhow!(e.message))?;
    print(&after, args.format, global)?;
    Ok(EXIT_OK)
}

fn describe(s: &Status, remove: bool) -> String {
    let loc = s
        .location
        .as_ref()
        .map_or_else(String::new, |p| p.display().to_string());
    match (s.method, remove) {
        (Method::Symlink, false) => format!("Create {loc} -> {}?", s.cli_path.display()),
        (Method::Symlink, true) => format!("Remove {loc}?"),
        (Method::UserPath, false) => format!("Add {loc} to your user PATH?"),
        (Method::UserPath, true) => format!("Remove {loc} from your user PATH?"),
        _ => String::new(),
    }
}

fn confirm(s: &Status, remove: bool) -> anyhow::Result<()> {
    let stdin = std::io::stdin();
    if !stdin.is_terminal() {
        bail!("not a terminal: pass --yes to change it without asking");
    }
    eprint!("{} [y/N] ", describe(s, remove));
    std::io::stderr().flush().ok();
    let mut line = String::new();
    stdin
        .lock()
        .read_line(&mut line)
        .context("reading the answer")?;
    if matches!(line.trim(), "y" | "Y" | "yes") {
        Ok(())
    } else {
        bail!("cancelled – nothing was changed")
    }
}

fn print(s: &Status, format: Format, global: &Global) -> anyhow::Result<()> {
    if format == Format::Json {
        println!("{}", serde_json::to_string_pretty(s)?);
        return Ok(());
    }
    if global.quiet {
        return Ok(());
    }
    let loc = s
        .location
        .as_ref()
        .map_or_else(|| "-".to_owned(), |p| p.display().to_string());
    match s.method {
        Method::Symlink if s.installed => println!("installed: {loc} -> {}", s.cli_path.display()),
        Method::Symlink if s.conflict => {
            println!("not installed: {loc} belongs to another program");
        }
        Method::Symlink => println!("not installed (would create {loc})"),
        Method::UserPath if s.installed => {
            println!("installed: {loc} is in your user PATH (open a new terminal)");
        }
        Method::UserPath => println!("not installed ({loc} is not in your user PATH)"),
        Method::Package => println!("installed by the system package: {loc}"),
        Method::Unavailable => println!("{}", s.note.as_deref().unwrap_or("not available")),
    }
    Ok(())
}
