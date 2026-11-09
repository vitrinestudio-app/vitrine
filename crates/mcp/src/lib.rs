//! # arrangia-mcp
//!
//! MCP server (stdio) started via `arrangia mcp [--project <path>]`
//! (`docs/KONZEPT.md`, section 8).
//!
//! The server is a thin adapter: every tool maps onto the operations API in
//! `arrangia-core`, rendering in `arrangia-render` and uploads in
//! `arrangia-publish`. There are no special code paths, so the same
//! validations apply as in the GUI and CLI. `render_preview` returns a PNG
//! image so Claude can see the result.

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
