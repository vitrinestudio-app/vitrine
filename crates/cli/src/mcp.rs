//! `vitrine mcp`: the MCP server over stdio (KONZEPT §8, ADR 0014).
//!
//! MCP clients (Claude Code, Claude Desktop) start this command themselves
//! and speak JSON-RPC over stdin/stdout, so nothing else may be printed to
//! stdout. With a running Vitrine Studio app it forwards everything to the app's
//! in-app endpoint (the assistant edits what the user sees, ADR 0015); without one
//! it starts the app (setting `mcp_autostart_gui`, default on) or works
//! headless with its own session. `--headless` forces the latter.

use std::path::PathBuf;

use anyhow::anyhow;
use clap::Args;
use vitrine_core::io::PROJECT_FILE;

use crate::{EXIT_OK, Global};

#[derive(Debug, Args)]
pub struct McpArgs {
    /// App data folder (settings, recent projects, installed device
    /// frames). Default: `$VITRINE_APP_DIR` or the platform folder.
    #[arg(long, value_name = "DIR")]
    pub app_dir: Option<PathBuf>,
    /// Work on the project folder directly with an own session, never
    /// through (or starting) the Vitrine Studio app. Default: connect to the
    /// running app, start it if needed, else headless.
    #[arg(long)]
    pub headless: bool,
}

/// The project to open: `--project` (default `.`) when it holds a
/// `project.yaml`; otherwise the server starts without a project
/// (`project_open` / `project_create`).
fn start_project(global: &Global) -> Option<PathBuf> {
    global
        .project
        .join(PROJECT_FILE)
        .is_file()
        .then(|| global.project.clone())
}

pub fn run(args: &McpArgs, global: &Global) -> anyhow::Result<u8> {
    let opts = vitrine_mcp::stdio::StdioOptions {
        project: start_project(global),
        app_dir: args.app_dir.clone(),
        headless: args.headless,
    };
    vitrine_mcp::stdio::run(&opts).map_err(|e| anyhow!(e))?;
    Ok(EXIT_OK)
}
