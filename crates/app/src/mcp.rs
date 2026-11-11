//! State of the GUI-hosted MCP endpoint (ADR 0015) as the UI sees it.
//!
//! `vitrine-app` does not depend on the MCP server. The host (the GUI,
//! via `vitrine_mcp::host`) reports into [`McpStatus`] through
//! [`crate::AppState::update_mcp_status`] (which emits `mcp-status`) and
//! registers an [`McpControl`] so the `mcp_set_enabled` command can start
//! and stop it. Headless front ends (CLI, tests) leave both alone: the
//! status then says "not running".

use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;

use crate::error::CmdResult;

/// A client counts as connected while it has an open MCP session or was
/// active within this many milliseconds (stateless clients have no
/// session to watch).
pub const ACTIVE_WINDOW_MS: u64 = 5 * 60 * 1000;

/// Starts and stops the MCP endpoint (implemented by the GUI's host).
pub trait McpControl: Send + Sync {
    /// Switches the endpoint on or off (the setting is already saved).
    fn set_enabled(&self, enabled: bool) -> CmdResult<()>;
}

/// One MCP client (by its `clientInfo.name`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct McpClient {
    /// `clientInfo.name`, e.g. `claude-ai` (Claude Desktop), `claude-code`.
    pub name: String,
    /// Display name ([`crate::client_display_name`]: known clients,
    /// `clientInfo.title`, or the prettified name).
    pub display_name: String,
    /// Open MCP sessions of this client.
    pub sessions: u32,
    /// Unix time (ms) of the last request.
    pub last_seen: u64,
    /// Name of the last tool called.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_tool: Option<String>,
    /// Tool calls so far.
    pub calls: u64,
    /// Open session or active within [`ACTIVE_WINDOW_MS`] (at the time
    /// of the report).
    pub connected: bool,
}

/// Status of the in-app MCP endpoint (payload of `mcp-status`; never
/// holds the token).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct McpStatus {
    /// The host is available in this front end (GUI, dev bridge `--mcp`).
    pub hosted: bool,
    /// The endpoint is switched on in the settings.
    pub enabled: bool,
    /// It listens right now.
    pub running: bool,
    /// `http://127.0.0.1:<port>/mcp`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    /// Why it does not run (bind failed …).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// The discovery file `vitrine mcp` reads.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub discovery_file: Option<std::path::PathBuf>,
    pub clients: Vec<McpClient>,
    /// Unix time (ms) of the last request of any client.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_activity: Option<u64>,
}

/// Now as unix milliseconds.
pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

impl McpStatus {
    fn client(&mut self, name: &str) -> &mut McpClient {
        if let Some(i) = self.clients.iter().position(|c| c.name == name) {
            return &mut self.clients[i];
        }
        self.clients.push(McpClient {
            name: name.to_owned(),
            display_name: crate::client_display_name(name, None),
            ..McpClient::default()
        });
        self.clients.last_mut().expect("just pushed")
    }

    fn touch(&mut self, name: &str, title: Option<&str>) -> &mut McpClient {
        let now = now_ms();
        self.last_activity = Some(now);
        let c = self.client(name);
        c.last_seen = now;
        if title.is_some_and(|t| !t.trim().is_empty()) {
            c.display_name = crate::client_display_name(name, title);
        }
        c
    }

    /// A client (`clientInfo.name`, optional `clientInfo.title`) opened an
    /// MCP session (`initialize`).
    pub fn session_opened(&mut self, name: &str, title: Option<&str>) {
        self.touch(name, title).sessions += 1;
    }

    /// A session of `name` ended (closed or expired).
    pub fn session_closed(&mut self, name: &str) {
        let c = self.client(name);
        c.sessions = c.sessions.saturating_sub(1);
    }

    /// `name` called `tool`.
    pub fn tool_called(&mut self, name: &str, title: Option<&str>, tool: &str) {
        let c = self.touch(name, title);
        c.calls += 1;
        c.last_tool = Some(tool.to_owned());
    }

    /// Recomputes `connected` for `now` (unix ms).
    pub fn refresh(&mut self, now: u64) {
        for c in &mut self.clients {
            c.connected = c.sessions > 0 || now.saturating_sub(c.last_seen) < ACTIVE_WINDOW_MS;
        }
    }

    /// The endpoint stopped: no sessions any more.
    pub fn stopped(&mut self) {
        self.running = false;
        self.url = None;
        self.port = None;
        for c in &mut self.clients {
            c.sessions = 0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clients_connect_and_expire() {
        let mut s = McpStatus::default();
        s.session_opened("claude-ai", None);
        s.tool_called("claude-ai", None, "element_add");
        s.tool_called("claude-code", None, "project_summary");
        s.tool_called("some-client", Some("Some Editor"), "view_get");
        let now = now_ms();
        s.refresh(now);
        assert_eq!(s.clients.len(), 3);
        assert_eq!(s.clients[0].display_name, "Claude Desktop");
        assert_eq!(s.clients[2].display_name, "Some Editor");
        assert!(s.clients.iter().all(|c| c.connected));
        assert_eq!(s.clients[0].calls, 1);
        assert_eq!(s.clients[0].last_tool.as_deref(), Some("element_add"));
        // Long after: only the client with an open session stays.
        s.refresh(now + ACTIVE_WINDOW_MS + 1);
        assert!(s.clients[0].connected && !s.clients[1].connected);
        s.session_closed("claude-ai");
        s.session_closed("claude-ai");
        assert_eq!(s.clients[0].sessions, 0);
        s.refresh(now + ACTIVE_WINDOW_MS + 1);
        assert!(!s.clients[0].connected);
        let json = serde_json::to_value(&s).unwrap();
        assert!(json.get("token").is_none());
    }
}
