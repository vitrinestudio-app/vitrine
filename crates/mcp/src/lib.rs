//! # arrangia-mcp
//!
//! The MCP server of Arrangia (`docs/KONZEPT.md` §8, ADR 0014), built on
//! the official Rust SDK `rmcp`.
//!
//! [`ArrangiaMcp`] is a thin adapter over the shared application layer
//! (`arrangia_app`): every tool calls `arrangia_app::commands`, so GUI, CLI
//! and MCP share one behaviour (ops, edit intents, validation, revision
//! checks, atomic saves). Edits run as `Actor::Claude` (history labels
//! `Claude: …`). The handler holds an `Arc<AppState>`, so the same router
//! can be served
//!
//! - headless over stdio with its own state ([`stdio::run`], `arrangia mcp
//!   --headless`), or
//! - by the GUI over Streamable HTTP against its live session (Phase 4.3b;
//!   feature `http` enables rmcp's server transport).
//!
//! `render_preview` returns PNG image content so the model sees the result.
//!
//! ```text
//! server     ArrangiaMcp: tool calls (actor, blocking pool, disk sync), resources
//! tools/*    one #[tool_router] per area (project, catalog, structure, elements,
//!            strings, styles, assets, output, history)
//! params     shared arguments (edit scope → view context)
//! output     tool results / tool errors
//! progress   export progress → MCP progress notifications
//! resources  project.yaml, i18n/<lang>.yaml, catalogs, op reference
//! stdio      `arrangia mcp`: GUI discovery seam + headless stdio server
//! http       (feature `http`) Streamable HTTP service for the GUI
//! ```

#[cfg(feature = "http")]
pub mod http;
pub mod output;
pub mod params;
pub mod progress;
pub mod resources;
pub mod server;
pub mod stdio;
pub mod tools;

pub use progress::ProgressRelay;
pub use server::{ArrangiaMcp, Host, INSTRUCTIONS};

/// Name under which the server announces itself to MCP clients.
pub const SERVER_NAME: &str = "arrangia";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_name_is_binary_name() {
        assert_eq!(SERVER_NAME, "arrangia");
        assert!((arrangia_core::PAGE_WIDTH_LU - 1000.0).abs() < f64::EPSILON);
    }
}
