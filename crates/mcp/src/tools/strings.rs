//! Translations: read, set (batch), missing, key rename, CSV/XLIFF
//! exchange.

use std::collections::BTreeMap;
use std::path::PathBuf;

use arrangia_app::CommandError;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::CallToolResult;
use rmcp::service::RequestContext;
use rmcp::{RoleServer, tool, tool_router};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Map, Value, json};

use crate::output::ToolOutput;
use crate::params::{edit_line, edit_summary};
use crate::server::{ArrangiaMcp, cmd};

#[derive(Debug, Clone, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StringsGetParams {
    /// Only this language (default: all).
    #[serde(default)]
    pub lang: Option<String>,
    /// Only these keys (default: all).
    #[serde(default)]
    pub keys: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StringsSetParams {
    /// Language of the texts, e.g. `de`.
    pub lang: String,
    /// key → text (markup allowed: `**bold**`, `_italic_`, `[span]…[/]`,
    /// `\n`, `\-`); null removes the translation (falls back again).
    pub values: BTreeMap<String, Option<String>>,
}

#[derive(Debug, Clone, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StringsMissingParams {
    /// Only this language (default: all languages).
    #[serde(default)]
    pub lang: Option<String>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct KeyRenameParams {
    pub from: String,
    pub to: String,
}

/// Exchange file format.
#[derive(Debug, Clone, Copy, Deserialize, serde::Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExchangeFormat {
    Csv,
    Xliff12,
    Xliff20,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StringsExportParams {
    pub format: ExchangeFormat,
    /// Target languages (default: all but the default language).
    #[serde(default)]
    pub langs: Vec<String>,
    /// File or folder to write (relative = inside the project). Several
    /// XLIFF languages go to `<stem>-<lang>.xlf` next to it.
    pub path: PathBuf,
    /// Only keys missing in a target language.
    #[serde(default)]
    pub missing_only: bool,
    /// Context for translators (elements, pages, box width).
    #[serde(default)]
    pub context: bool,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StringsImportParams {
    /// CSV/XLIFF files (absolute or relative to the project).
    pub paths: Vec<PathBuf>,
    /// Only report what would change (default false: apply as one undo
    /// step).
    #[serde(default)]
    pub dry_run: bool,
    /// Create keys the project does not have yet.
    #[serde(default)]
    pub create_keys: bool,
}

fn edited(r: &Value) -> ToolOutput {
    let s = edit_summary(r);
    ToolOutput::new(edit_line(&s), s)
}

#[tool_router(router = string_tools, vis = "pub(crate)")]
impl ArrangiaMcp {
    #[tool(
        description = "Translations: languages, keys, lang → key → text, and which text elements use each key. Example: {\"lang\": \"de\"}.",
        annotations(read_only_hint = true)
    )]
    async fn strings_get(
        &self,
        Parameters(p): Parameters<StringsGetParams>,
        ctx: RequestContext<RoleServer>,
    ) -> CallToolResult {
        self.run(&ctx, move |app| {
            let mut args = json!({});
            if let Some(l) = &p.lang {
                args["lang"] = json!(l);
            }
            let mut t = cmd(app, "strings_get", args)?;
            if !p.keys.is_empty() {
                let keep = |m: &mut Value| {
                    if let Some(o) = m.as_object_mut() {
                        o.retain(|k, _| p.keys.contains(k));
                    }
                };
                if let Some(langs) = t["strings"].as_object_mut() {
                    langs.values_mut().for_each(keep);
                }
                keep(&mut t["used_by"]);
                t["keys"] = json!(p.keys);
            }
            let n = t["keys"].as_array().map_or(0, Vec::len);
            Ok(ToolOutput::new(format!("{n} key(s)"), t))
        })
        .await
    }

    #[tool(
        description = "Set translations of one language in one undo step. Keep the markup of the source text. Example: {\"lang\": \"de\", \"values\": {\"hero.title\": \"Plane deine Beete **in Minuten**\", \"hero.sub\": \"Mit [accent]Mischkultur[/] zu mehr Ernte\"}}. Then render_preview with lang to check the layout."
    )]
    async fn strings_set(
        &self,
        Parameters(p): Parameters<StringsSetParams>,
        ctx: RequestContext<RoleServer>,
    ) -> CallToolResult {
        self.run(&ctx, move |app| {
            if p.values.is_empty() {
                return Err(CommandError::invalid("`values` is empty"));
            }
            let current = cmd(app, "strings_get", json!({ "lang": p.lang }))?;
            let ops: Vec<Value> = p
                .values
                .iter()
                .filter(|(k, v)| {
                    current["strings"][&p.lang].get(*k).and_then(Value::as_str) != v.as_deref()
                })
                .map(|(k, v)| json!({ "op": "set_string", "lang": p.lang, "key": k, "value": v }))
                .collect();
            if ops.is_empty() {
                return Ok(ToolOutput::new(
                    "nothing changed (all values already set)",
                    json!({ "applied": false }),
                ));
            }
            let n = ops.len();
            let label = format!("Set {n} {} string{}", p.lang, if n == 1 { "" } else { "s" });
            Ok(edited(&cmd(
                app,
                "apply_ops",
                json!({ "ops": ops, "label": label }),
            )?))
        })
        .await
    }

    #[tool(
        description = "Keys without a translation in a language, with the text shown instead (fallback and its language) and the elements using them – the work list for translating. Example: {\"lang\": \"fr\"}.",
        annotations(read_only_hint = true)
    )]
    async fn strings_missing(
        &self,
        Parameters(p): Parameters<StringsMissingParams>,
        ctx: RequestContext<RoleServer>,
    ) -> CallToolResult {
        self.run(&ctx, move |app| {
            let all = cmd(app, "strings_missing", Value::Null)?;
            let list: Vec<Value> = all
                .as_array()
                .into_iter()
                .flatten()
                .filter(|m| p.lang.as_ref().is_none_or(|l| m["lang"] == l.as_str()))
                .cloned()
                .collect();
            Ok(ToolOutput::new(
                format!("{} missing translation(s)", list.len()),
                json!({ "missing": list }),
            ))
        })
        .await
    }

    #[tool(
        description = "Rename a translation key in every language and every text element using it (one undo step). Example: {\"from\": \"text-2\", \"to\": \"beds.title\"}."
    )]
    async fn string_key_rename(
        &self,
        Parameters(p): Parameters<KeyRenameParams>,
        ctx: RequestContext<RoleServer>,
    ) -> CallToolResult {
        self.run(&ctx, move |app| {
            let ops = json!([{ "op": "rename_string_key", "from": p.from, "to": p.to }]);
            let label = format!("Rename key {} to {}", p.from, p.to);
            Ok(edited(&cmd(
                app,
                "apply_ops",
                json!({ "ops": ops, "label": label }),
            )?))
        })
        .await
    }

    #[tool(
        description = "Write translations for translators as CSV or XLIFF (1.2/2.0). Example: {\"format\": \"csv\", \"path\": \"translations/strings.csv\", \"langs\": [\"de\", \"fr\"], \"missing_only\": true}."
    )]
    async fn strings_export(
        &self,
        Parameters(p): Parameters<StringsExportParams>,
        ctx: RequestContext<RoleServer>,
    ) -> CallToolResult {
        self.run(&ctx, move |app| {
            let args = json!({
                "format": p.format, "langs": p.langs, "path": p.path,
                "options": { "missing_only": p.missing_only, "context": p.context },
            });
            let r = cmd(app, "strings_export", args)?;
            let n = r["files"].as_array().map_or(0, Vec::len);
            Ok(ToolOutput::new(format!("wrote {n} file(s)"), r))
        })
        .await
    }

    #[tool(
        description = "Import translated CSV/XLIFF files: plans the changes (with markup and layout checks) and applies them as one undo step unless dry_run. Example: {\"paths\": [\"translations/strings.csv\"], \"dry_run\": true}."
    )]
    async fn strings_import(
        &self,
        Parameters(p): Parameters<StringsImportParams>,
        ctx: RequestContext<RoleServer>,
    ) -> CallToolResult {
        self.run(&ctx, move |app| {
            let plan = cmd(
                app,
                "strings_import_plan",
                json!({ "paths": p.paths, "options": { "create_keys": p.create_keys } }),
            )?;
            let n = plan["changes"].as_array().map_or(0, Vec::len);
            if p.dry_run || n == 0 {
                return Ok(ToolOutput::new(
                    format!(
                        "plan: {n} change(s){}",
                        if n == 0 { "" } else { " (dry run)" }
                    ),
                    json!({ "plan": plan, "applied": false }),
                ));
            }
            let r = cmd(app, "strings_import_apply", json!({ "plan": plan }))?;
            let mut out = Map::new();
            out.insert("plan".into(), plan);
            out.insert("result".into(), edit_summary(&r));
            Ok(ToolOutput::new(
                format!("imported {n} change(s)"),
                Value::Object(out),
            ))
        })
        .await
    }
}
