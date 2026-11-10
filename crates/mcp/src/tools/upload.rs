//! Store upload (Phase 5.5, ADR 0018): `upload_plan`, `upload_execute`,
//! `upload_status`, `upload_log`.
//!
//! Uploads are outward-facing (they change the public store listing), so
//! `upload_execute` goes through `arrangia_app::upload::policy`:
//! GUI-hosted → the user must click "Confirm upload" in the app (the call
//! waits up to 5 minutes, else `not_confirmed`); headless → refused unless
//! the app setting `mcp_allow_headless_upload` is on.

use arrangia_app::upload::policy::{self, McpHost};
use arrangia_core::model::StoreKind;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::CallToolResult;
use rmcp::service::RequestContext;
use rmcp::{RoleServer, tool, tool_router};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

use super::project::NoParams;
use crate::output::ToolOutput;
use crate::server::{ArrangiaMcp, Host, cmd};

#[derive(Debug, Clone, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct UploadPlanParams {
    /// `apple` or `google` (default: every store the project is mapped to).
    #[serde(default)]
    pub store: Option<String>,
    /// Only these targets (default: every target with a store slot).
    #[serde(default)]
    pub targets: Vec<String>,
    /// Only these languages (default: each target's languages).
    #[serde(default)]
    pub langs: Vec<String>,
    /// Also read the store state (network): Apple editable version,
    /// localizations, existing screenshots; Google listing images.
    #[serde(default)]
    pub remote: bool,
    /// Upload sets even when they look unchanged.
    #[serde(default)]
    pub force: bool,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct UploadExecuteParams {
    /// `confirm_token` from `upload_plan` (valid 30 minutes, only for the
    /// unchanged project).
    pub confirm_token: String,
    /// Google: commit with "changes not sent for review" (only when Play
    /// rejected the commit without it).
    #[serde(default)]
    pub changes_not_sent_for_review: bool,
}

#[derive(Debug, Clone, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct UploadLogParams {
    /// Number of runs, newest first (default 5).
    #[serde(default)]
    pub limit: Option<usize>,
}

/// The plan's setup checklist as numbered lines with the place to fix each
/// step (the same steps the upload dialog shows).
fn readiness_lines(p: &Value) -> String {
    let steps = p["readiness"]["steps"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    if steps.is_empty() {
        return String::new();
    }
    let lines: Vec<String> = steps
        .iter()
        .enumerate()
        .map(|(n, s)| {
            format!(
                "{}. {}{}",
                n + 1,
                s["message"].as_str().unwrap_or_default(),
                if s["blocking"] == true {
                    ""
                } else {
                    " (warning)"
                }
            )
        })
        .collect();
    format!(
        "\nBefore uploading, the user has to set this up in the Arrangia app (store keys \
         and the project's store mapping cannot be changed over MCP):\n{}",
        lines.join("\n")
    )
}

fn plan_line(p: &Value) -> String {
    let t = &p["totals"];
    let line = format!(
        "{} set(s) to upload ({} image(s)), {} unchanged, {} skipped, {} blocked{}; confirm_token {}",
        t["upload_sets"],
        t["images"],
        t["unchanged_sets"],
        t["skipped_sets"],
        t["blocked_sets"],
        if p["blocked"] == true {
            " – BLOCKED, fix the problems first"
        } else {
            ""
        },
        p["confirm_token"].as_str().unwrap_or_default()
    );
    line + &readiness_lines(p)
}

#[tool_router(router = upload_tools, vis = "pub(crate)")]
impl ArrangiaMcp {
    #[tool(
        description = "Dry run of a store upload (App Store Connect / Google Play): which store sets (target × language → store locale × display/image type) would be uploaded, replaced or skipped as unchanged, the images in store order with hashes, store-rule problems and preconditions (`remote: true` also reads the store; needs network). Nothing is uploaded. Returns a `confirm_token` for upload_execute. Show the plan to the user before asking for an upload.",
        annotations(read_only_hint = true)
    )]
    async fn upload_plan(
        &self,
        Parameters(p): Parameters<UploadPlanParams>,
        ctx: RequestContext<RoleServer>,
    ) -> CallToolResult {
        self.run(&ctx, move |app| {
            if let Some(s) = &p.store {
                StoreKind::parse(s).map_err(arrangia_app::CommandError::invalid)?;
            }
            let v = cmd(
                app,
                "upload_plan",
                json!({ "store": p.store, "targets": p.targets, "langs": p.langs,
                        "remote": p.remote, "force": p.force }),
            )?;
            Ok(ToolOutput::new(plan_line(&v), v))
        })
        .await
    }

    #[tool(
        description = "Upload a planned set of screenshots to the stores. OUTWARD-FACING: this changes the app's public store listing. Only call it when the user explicitly asked for the upload after seeing the plan (upload_plan). The user must additionally confirm in the Arrangia app (a dialog shows the plan; this call waits up to 5 minutes and fails with `not_confirmed` when declined or unanswered). Without the app (headless) uploads are disabled unless the user turned on the app setting `mcp_allow_headless_upload`. Starts the run and returns right away; follow it with upload_status."
    )]
    async fn upload_execute(
        &self,
        Parameters(p): Parameters<UploadExecuteParams>,
        ctx: RequestContext<RoleServer>,
    ) -> CallToolResult {
        let host = match self.host() {
            Host::Gui => McpHost::Gui,
            Host::Headless => McpHost::Headless,
        };
        self.run(&ctx, move |app| {
            policy::check_mcp_allowed(app, host)?;
            let plan = arrangia_app::commands::upload_verify(app, &p.confirm_token)?;
            policy::authorize_mcp(app, host, &p.confirm_token, &plan)?;
            let v = cmd(
                app,
                "upload_execute",
                json!({ "token": p.confirm_token,
                        "changes_not_sent_for_review": p.changes_not_sent_for_review }),
            )?;
            let line = format!(
                "upload started (run {}): {} – follow it with upload_status",
                v["run"]["run_id"],
                plan.summary()
            );
            Ok(ToolOutput::new(line, v))
        })
        .await
    }

    #[tool(
        description = "Progress of the running (or last) store upload: status (running, paused for a rate limit, completed, partial, failed, cancelled), per set status and errors, images and bytes done; plus upload requests still waiting for the user's confirmation.",
        annotations(read_only_hint = true)
    )]
    async fn upload_status(
        &self,
        Parameters(_): Parameters<NoParams>,
        ctx: RequestContext<RoleServer>,
    ) -> CallToolResult {
        self.run(&ctx, move |app| {
            let v = cmd(app, "upload_status", Value::Null)?;
            let line = if v["run"].is_null() {
                "no upload in this session".to_owned()
            } else {
                format!(
                    "run {}: {} – {} of {} image(s){}",
                    v["run"]["run_id"],
                    v["run"]["status"].as_str().unwrap_or_default(),
                    v["run"]["images_done"],
                    v["run"]["images_total"],
                    v["run"]["message"]
                        .as_str()
                        .map(|m| format!(" ({m})"))
                        .unwrap_or_default()
                )
            };
            Ok(ToolOutput::new(line, v))
        })
        .await
    }

    #[tool(
        description = "The project's upload log (<project>/.cache/uploads.sqlite): past runs, newest first, with per set status, errors and the uploaded files (no secrets).",
        annotations(read_only_hint = true)
    )]
    async fn upload_log(
        &self,
        Parameters(p): Parameters<UploadLogParams>,
        ctx: RequestContext<RoleServer>,
    ) -> CallToolResult {
        self.run(&ctx, move |app| {
            let v = cmd(app, "upload_log", json!({ "limit": p.limit.unwrap_or(5) }))?;
            let n = v["runs"].as_array().map_or(0, Vec::len);
            Ok(ToolOutput::new(format!("{n} upload run(s)"), v))
        })
        .await
    }
}
