//! `arrangia mcp`: the MCP server over stdio (KONZEPT §8, ADR 0014).
//!
//! MCP clients (Claude Code, Claude Desktop) start this command themselves
//! and speak JSON-RPC over stdin/stdout, so nothing else may be printed to
//! stdout. Until the GUI hosts the server (Phase 4.3b) it always runs
//! headless with its own session; `--headless` already forces that.

use std::path::PathBuf;

use anyhow::anyhow;
use arrangia_core::io::PROJECT_FILE;
use clap::Args;

use crate::{EXIT_OK, Global};

#[derive(Debug, Args)]
pub struct McpArgs {
    /// App data folder (settings, recent projects, installed device
    /// frames). Default: `$ARRANGIA_APP_DIR` or the platform folder.
    #[arg(long, value_name = "DIR")]
    pub app_dir: Option<PathBuf>,
    /// Work on the project folder directly, never through a running
    /// Arrangia GUI (the GUI connection comes later; until then every run
    /// is headless).
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
    let opts = arrangia_mcp::stdio::StdioOptions {
        project: start_project(global),
        app_dir: args.app_dir.clone(),
        headless: args.headless,
    };
    arrangia_mcp::stdio::run(&opts).map_err(|e| anyhow!(e))?;
    Ok(EXIT_OK)
}
