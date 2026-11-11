//! `docs_get`: the reference documentation as a tool (clients such as
//! Claude Desktop do not let the model read resources by itself).

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::CallToolResult;
use rmcp::service::RequestContext;
use rmcp::{RoleServer, tool, tool_router};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::json;

use crate::output::ToolOutput;
use crate::server::VitrineMcp;

/// A documentation topic.
#[derive(Debug, Clone, Copy, Deserialize, serde::Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DocTopic {
    /// Concepts and workflow.
    Overview,
    /// Every element kind with all fields, types, defaults, allowed values.
    Elements,
    /// JSON shape of every op for `apply_ops` (incl. override ops).
    Ops,
    /// `project.yaml` structure: languages, styles, canvases, variants, targets.
    Project,
    /// Inline text markup, auto-fit, hyphenation.
    Markup,
    /// Targets: presets, sizes, gallery gaps, group, banners.
    Targets,
    /// Base / variant / language overrides: setting and resetting.
    Scopes,
}

impl DocTopic {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Overview => "overview",
            Self::Elements => "elements",
            Self::Ops => "ops",
            Self::Project => "project",
            Self::Markup => "markup",
            Self::Targets => "targets",
            Self::Scopes => "scopes",
        }
    }
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DocsGetParams {
    /// overview | elements | ops | project | markup | targets | scopes.
    pub topic: DocTopic,
}

#[tool_router(router = docs_tools, vis = "pub(crate)")]
impl VitrineMcp {
    #[tool(
        description = "Reference documentation as markdown. Topics: overview (concepts, workflow), elements (every element kind with all fields, types, defaults and allowed values, fill shapes incl. image fit/focus), ops (every apply_ops op incl. override ops, examples), project (project.yaml structure), markup (text markup, auto-fit, hyphenation), targets (presets, sizes, gallery gaps, group), scopes (base/variant/language overrides, resetting). Call it before guessing field names. Example: {\"topic\": \"elements\"}.",
        annotations(read_only_hint = true)
    )]
    async fn docs_get(
        &self,
        Parameters(p): Parameters<DocsGetParams>,
        ctx: RequestContext<RoleServer>,
    ) -> CallToolResult {
        self.run_unsynced(&ctx, move |_| {
            let name = p.topic.as_str();
            let text = crate::docs::topic(name).unwrap_or_default();
            // The markdown is the text block; structured content stays small.
            Ok(ToolOutput::new(text, json!({ "topic": name })).text_only())
        })
        .await
    }
}
