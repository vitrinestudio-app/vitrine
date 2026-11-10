//! Undo/redo of the shared session history (the GUI's, when hosted there).

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::CallToolResult;
use rmcp::service::RequestContext;
use rmcp::{RoleServer, tool, tool_router};
use serde_json::{Value, json};

use super::project::NoParams;
use crate::output::ToolOutput;
use crate::server::{ArrangiaMcp, cmd};

fn stepped(r: &Value, verb: &str) -> ToolOutput {
    let label = if verb == "undid" {
        r["history"]["redo"]
            .as_array()
            .and_then(|a| a.last())
            .cloned()
    } else {
        r["history"]["undo"]
            .as_array()
            .and_then(|a| a.last())
            .cloned()
    };
    let out = json!({
        "applied": r["applied"],
        "revision": r["revision"],
        "step": label,
        "can_undo": r["history"]["can_undo"],
        "can_redo": r["history"]["can_redo"],
    });
    let line = if r["applied"] == true {
        format!(
            "{verb}: {}",
            label.as_ref().and_then(Value::as_str).unwrap_or("step")
        )
    } else {
        format!(
            "nothing to {}",
            if verb == "undid" { "undo" } else { "redo" }
        )
    };
    ToolOutput::new(line, out)
}

#[tool_router(router = history_tools, vis = "pub(crate)")]
impl ArrangiaMcp {
    #[tool(
        description = "Undo the most recent step of the project history. The history is shared with the GUI user: check history_state first; your own steps are labelled \"Claude: …\"."
    )]
    async fn history_undo(
        &self,
        Parameters(_): Parameters<NoParams>,
        ctx: RequestContext<RoleServer>,
    ) -> CallToolResult {
        self.run(&ctx, move |app| {
            Ok(stepped(&cmd(app, "undo", Value::Null)?, "undid"))
        })
        .await
    }

    #[tool(description = "Redo the most recently undone step.")]
    async fn history_redo(
        &self,
        Parameters(_): Parameters<NoParams>,
        ctx: RequestContext<RoleServer>,
    ) -> CallToolResult {
        self.run(&ctx, move |app| {
            Ok(stepped(&cmd(app, "redo", Value::Null)?, "redid"))
        })
        .await
    }

    #[tool(
        description = "Undo/redo availability and the step labels (most recent last; steps by Claude start with \"Claude: \"). External edits of the files clear the history.",
        annotations(read_only_hint = true)
    )]
    async fn history_state(
        &self,
        Parameters(_): Parameters<NoParams>,
        ctx: RequestContext<RoleServer>,
    ) -> CallToolResult {
        self.run(&ctx, move |app| {
            let h = cmd(app, "history_state", Value::Null)?;
            let line = format!(
                "{} undo step(s), {} redo step(s)",
                h["undo"].as_array().map_or(0, Vec::len),
                h["redo"].as_array().map_or(0, Vec::len)
            );
            Ok(ToolOutput::new(line, h))
        })
        .await
    }
}
