//! Style tokens: colours, text styles, span styles, shadows, frame style.

use std::collections::BTreeMap;

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::CallToolResult;
use rmcp::service::RequestContext;
use rmcp::{RoleServer, tool, tool_router};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Map, Value, json};

use super::project::NoParams;
use crate::output::ToolOutput;
use crate::params::{edit_line, edit_summary};
use crate::server::{VitrineMcp, cmd};

/// Kind of a style token (for renames).
#[derive(Debug, Clone, Copy, Deserialize, serde::Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum StyleKind {
    Color,
    Text,
    Span,
    Shadow,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StyleRename {
    pub kind: StyleKind,
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StylesUpdateParams {
    /// name → `#RRGGBB[AA]` (null removes), e.g. `{"accent": "#F6C453"}`.
    #[serde(default)]
    pub colors: BTreeMap<String, Option<String>>,
    /// name → text style fields {font, weight, italic, size (lu),
    /// `line_height`, tracking, color, transform, shadow, hyphenate …}.
    /// Merged into an existing style (a field set to null is removed);
    /// null removes the style. E.g. `{"headline": {"size": 84, "color":
    /// "@paper"}}`.
    #[serde(default)]
    pub text: BTreeMap<String, Option<Map<String, Value>>>,
    /// name → span style {color, weight, italic, background …} for
    /// `[name]…[/]` markup (merged like `text`).
    #[serde(default)]
    pub spans: BTreeMap<String, Option<Map<String, Value>>>,
    /// name → shadow preset {layers: [{color, blur, x, y}]} (merged like
    /// `text`).
    #[serde(default)]
    pub shadows: BTreeMap<String, Option<Map<String, Value>>>,
    /// Device frame style of the project: `stylized`, `detailed` or
    /// `imported:apple` (empty string = default).
    #[serde(default)]
    pub frame: Option<String>,
    /// Rename a token; every reference follows (same undo step).
    #[serde(default)]
    pub rename: Option<StyleRename>,
}

/// `patch` entries merged over the current entries of `current` (objects
/// field by field; `null` fields removed).
fn merge_entries(
    current: &Value,
    patch: BTreeMap<String, Option<Map<String, Value>>>,
) -> Map<String, Value> {
    let mut out = Map::new();
    for (name, v) in patch {
        let merged = v.map_or(Value::Null, |fields| {
            let mut base = current
                .get(&name)
                .and_then(Value::as_object)
                .cloned()
                .unwrap_or_default();
            for (k, val) in fields {
                if val.is_null() {
                    base.remove(&k);
                } else {
                    base.insert(k, val);
                }
            }
            Value::Object(base)
        });
        out.insert(name, merged);
    }
    out
}

#[tool_router(router = style_tools, vis = "pub(crate)")]
impl VitrineMcp {
    #[tool(
        description = "All style tokens of the project: colors (name → hex), text styles (font, weight, size in lu, line height, colour …), span styles, shadow presets, font settings, frame style.",
        annotations(read_only_hint = true)
    )]
    async fn styles_get(
        &self,
        Parameters(_): Parameters<NoParams>,
        ctx: RequestContext<RoleServer>,
    ) -> CallToolResult {
        self.run(&ctx, move |app| {
            let v = cmd(app, "styles_get", Value::Null)?;
            Ok(ToolOutput::new("styles", v))
        })
        .await
    }

    #[tool(
        description = "Change style tokens in one undo step: colours, text styles (merged field by field), span styles, shadows, frame style, or rename a token. Example: {\"colors\": {\"accent\": \"#FF7A45\"}, \"text\": {\"headline\": {\"size\": 84}}}."
    )]
    async fn styles_update(
        &self,
        Parameters(p): Parameters<StylesUpdateParams>,
        ctx: RequestContext<RoleServer>,
    ) -> CallToolResult {
        self.run(&ctx, move |app| {
            let current = cmd(app, "styles_get", Value::Null)?;
            let mut args = json!({
                "colors": p.colors,
                "text": merge_entries(&current["text"], p.text),
                "spans": merge_entries(&current["spans"], p.spans),
                "shadows": merge_entries(&current["shadows"], p.shadows),
            });
            if let Some(f) = p.frame {
                args["frame"] = if f.is_empty() { Value::Null } else { json!(f) };
            }
            if let Some(r) = p.rename {
                args["rename"] = json!({ "kind": r.kind, "from": r.from, "to": r.to });
            }
            let s = edit_summary(&cmd(app, "styles_patch", args)?);
            Ok(ToolOutput::new(edit_line(&s), s))
        })
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entries_merge_field_by_field() {
        let current = json!({"headline": {"font": "Inter", "size": 90, "color": "@paper"}});
        let mut patch = BTreeMap::new();
        let mut fields = Map::new();
        fields.insert("size".into(), json!(80));
        fields.insert("color".into(), Value::Null);
        patch.insert("headline".to_owned(), Some(fields));
        patch.insert("old".to_owned(), None);
        let m = merge_entries(&current, patch);
        assert_eq!(m["headline"], json!({"font": "Inter", "size": 80}));
        assert_eq!(m["old"], Value::Null);
    }
}
