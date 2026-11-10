//! The GUI-hosted MCP endpoint (ADR 0015): status, on/off, how any MCP
//! client connects, plus the Claude-specific setup ("Add to Claude
//! Desktop", the Claude Code commands).

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::{AppState, NoArgs};
use crate::claude_desktop::{self, Installed, Plan};
use crate::error::CmdResult;
use crate::mcp::McpStatus;

/// Ways to connect Claude Code.
#[derive(Debug, Clone, Serialize)]
pub struct ClaudeCodeCommands {
    /// `claude mcp add arrangia -- <cli> mcp` (stdio proxy; always works,
    /// recommended).
    pub stdio: String,
    /// `claude mcp add --transport http …` with this run's port and token
    /// (both change per start unless a fixed port is set; the token always
    /// changes).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub http: Option<String>,
}

/// How any MCP client connects (shown neutrally in the settings).
#[derive(Debug, Clone, Serialize)]
pub struct GenericConnect {
    /// The stdio command: `<cli> mcp` (proxies to this app; always works).
    pub stdio: String,
    /// `Authorization: Bearer <token>` for the HTTP endpoint (`url` in the
    /// status) while it runs.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub http_header: Option<String>,
}

/// Answer of `mcp_status`.
#[derive(Debug, Clone, Serialize)]
pub struct McpStatusReply {
    #[serde(flatten)]
    pub status: McpStatus,
    /// Settings `mcp_port` (preferred port).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preferred_port: Option<u16>,
    /// Bearer token of the running endpoint.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
    /// The `arrangia` CLI that stdio clients (Claude Desktop, Claude Code,
    /// others) start.
    pub cli_path: PathBuf,
    pub cli_exists: bool,
    /// For any MCP client.
    pub generic: GenericConnect,
    pub claude_code: ClaudeCodeCommands,
}

/// Quotes `s` for a POSIX shell when needed.
fn shell_quote(s: &str) -> String {
    if !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || "/._-+:=@".contains(c))
    {
        s.to_owned()
    } else {
        format!("'{}'", s.replace('\'', r"'\''"))
    }
}

pub fn mcp_status(app: &AppState, _: NoArgs) -> CmdResult<McpStatusReply> {
    let status = app.mcp_status();
    let token = app.mcp_token().filter(|_| status.running);
    let cli = claude_desktop::cli_path();
    let generic = GenericConnect {
        stdio: format!("{} mcp", shell_quote(&cli.to_string_lossy())),
        http_header: token
            .as_ref()
            .filter(|_| status.url.is_some())
            .map(|t| format!("Authorization: Bearer {t}")),
    };
    let stdio = format!("claude mcp add arrangia -- {}", generic.stdio);
    let http = match (&status.url, &token) {
        (Some(url), Some(token)) => Some(format!(
            "claude mcp add --transport http arrangia {url} --header \"Authorization: Bearer {token}\""
        )),
        _ => None,
    };
    Ok(McpStatusReply {
        preferred_port: app.settings_store().load().ok().and_then(|s| s.mcp_port),
        token,
        cli_exists: cli.is_file(),
        cli_path: cli,
        generic,
        claude_code: ClaudeCodeCommands { stdio, http },
        status,
    })
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct McpSetEnabledArgs {
    pub enabled: bool,
}

/// Saves the setting `mcp_enabled` and starts/stops the endpoint (when a
/// host is registered).
pub fn mcp_set_enabled(app: &AppState, args: McpSetEnabledArgs) -> CmdResult<McpStatusReply> {
    app.settings_store()
        .update(&serde_json::json!({ "mcp_enabled": args.enabled }))?;
    if let Some(control) = app.mcp_control() {
        control.set_enabled(args.enabled)?;
    } else {
        app.update_mcp_status(|s| s.enabled = args.enabled);
    }
    mcp_status(app, NoArgs {})
}

/// What "Add to Claude Desktop" would write, and where.
pub fn claude_desktop_plan(app: &AppState, _: NoArgs) -> CmdResult<Plan> {
    Ok(claude_desktop::plan(
        &app.claude_desktop_config()?,
        &claude_desktop::cli_path(),
    ))
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClaudeDesktopInstallArgs {
    /// The user confirmed the plan in the app (required).
    #[serde(default)]
    pub confirm: bool,
}

/// Writes the `mcpServers.arrangia` entry (backup first; needs `confirm`).
pub fn claude_desktop_install(
    app: &AppState,
    args: ClaudeDesktopInstallArgs,
) -> CmdResult<Installed> {
    claude_desktop::install(
        &app.claude_desktop_config()?,
        &claude_desktop::cli_path(),
        args.confirm,
    )
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::dialogs::NoDialogs;
    use crate::events::{MCP_STATUS, RecordingSink};
    use crate::settings::AppPaths;

    #[test]
    fn status_commands_and_claude_desktop_with_override() {
        let tmp = tempfile::tempdir().unwrap();
        let events = Arc::new(RecordingSink::default());
        let config = tmp.path().join("claude").join("claude_desktop_config.json");
        let app = AppState::new(
            AppPaths::in_dir(&tmp.path().join("app")),
            events.clone(),
            Arc::new(NoDialogs),
        )
        .with_claude_desktop_config(config.clone());

        let s = mcp_status(&app, NoArgs {}).unwrap();
        assert!(!s.status.running && s.token.is_none() && s.claude_code.http.is_none());
        assert!(s.generic.http_header.is_none() && s.generic.stdio.ends_with(" mcp"));
        assert!(
            s.claude_code
                .stdio
                .starts_with("claude mcp add arrangia -- ")
        );
        assert!(s.claude_code.stdio.ends_with(" mcp"));

        app.set_mcp_token(Some("tok".into()));
        app.update_mcp_status(|s| {
            s.hosted = true;
            s.running = true;
            s.url = Some("http://127.0.0.1:4100/mcp".into());
            s.port = Some(4100);
            s.session_opened("claude-ai", None);
        });
        let ev = events.named(MCP_STATUS);
        assert_eq!(ev.last().unwrap()["clients"][0]["name"], "claude-ai");
        assert!(ev.last().unwrap().get("token").is_none());
        let s = mcp_status(&app, NoArgs {}).unwrap();
        assert_eq!(
            s.generic.http_header.as_deref(),
            Some("Authorization: Bearer tok")
        );
        assert_eq!(
            s.claude_code.http.as_deref(),
            Some(
                "claude mcp add --transport http arrangia http://127.0.0.1:4100/mcp --header \"Authorization: Bearer tok\""
            )
        );

        // Without a host the switch only saves the setting.
        let s = mcp_set_enabled(&app, McpSetEnabledArgs { enabled: false }).unwrap();
        assert!(!s.status.enabled);
        assert!(!app.settings_store().load().unwrap().mcp_enabled);

        let plan = claude_desktop_plan(&app, NoArgs {}).unwrap();
        assert_eq!(plan.config_path, config);
        assert!(claude_desktop_install(&app, ClaudeDesktopInstallArgs { confirm: false }).is_err());
        assert!(!config.exists());
        let done =
            claude_desktop_install(&app, ClaudeDesktopInstallArgs { confirm: true }).unwrap();
        assert!(done.written && config.is_file());
    }

    #[test]
    fn shell_quoting() {
        assert_eq!(shell_quote("/usr/bin/arrangia"), "/usr/bin/arrangia");
        assert_eq!(
            shell_quote("/Users/me/My Apps/arrangia"),
            "'/Users/me/My Apps/arrangia'"
        );
        assert_eq!(shell_quote("it's"), r"'it'\''s'");
    }
}
