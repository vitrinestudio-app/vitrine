//! Streamable HTTP for the GUI-hosted server (feature `http`, ADR 0015).
//!
//! The GUI builds one [`VitrineMcp`] over its live `AppState` (with
//! `Host::Gui` and a [`crate::ProgressRelay`] wrapping its Tauri event
//! sink) and serves [`service`] on `127.0.0.1` behind its own bearer-token
//! check ([`crate::host`]). Every MCP session gets a clone of the server
//! ([`VitrineMcp::for_session`]): all share the one session the user
//! sees.

use std::sync::Arc;

use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use rmcp::transport::{StreamableHttpServerConfig, StreamableHttpService};

use crate::VitrineMcp;

/// A tower service answering MCP Streamable HTTP for `server`. Only
/// loopback `Host` headers are accepted (DNS rebinding protection); the
/// caller adds authentication and binds to `127.0.0.1`.
pub fn service(
    server: VitrineMcp,
    config: StreamableHttpServerConfig,
) -> StreamableHttpService<VitrineMcp, LocalSessionManager> {
    StreamableHttpService::new(
        move || Ok(server.for_session()),
        Arc::new(LocalSessionManager::default()),
        config.with_allowed_hosts(["localhost", "127.0.0.1", "::1"]),
    )
}
