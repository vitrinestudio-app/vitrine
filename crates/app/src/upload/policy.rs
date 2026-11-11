//! Who may execute an upload (ADR 0018). Small and isolated on purpose:
//! the user may change this policy later (STATUS.md, open question b).
//!
//! | caller | rule |
//! |---|---|
//! | the user (GUI, CLI) | the confirm token of a plan is enough (the GUI's "Confirm & upload" button, the CLI's `--confirm` + prompt/`--yes`) |
//! | AI assistant, GUI-hosted MCP | token **and** a click on "Confirm upload" in the GUI dialog: `upload-confirm-requested` → dialog with the plan summary → `upload_confirm {id, approved}`; the tool call waits up to [`super::UploadConfig::confirm_timeout`] (5 min), then fails with `not_confirmed` |
//! | AI assistant, headless MCP | refused (`forbidden`) unless the app setting `mcp_allow_headless_upload` is on (default off); then the token is enough |
//!
//! A granted confirmation is bound to the token and consumed by the next
//! `upload_execute` of the assistant; `upload_execute` refuses assistant
//! calls without a grant, so a tool cannot skip the gate.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Condvar, Mutex, PoisonError};
use std::time::{Duration, Instant};

use serde::Serialize;
use serde_json::json;

use super::{UPLOAD_CONFIRM_REQUESTED, UPLOAD_CONFIRM_RESOLVED, UploadPlan};
use crate::actor::Actor;
use crate::commands::AppState;
use crate::error::{CmdResult, CommandError, ErrorCode};

/// Where an MCP request comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McpHost {
    /// The GUI hosts the server: the user confirms in the GUI.
    Gui,
    /// `vitrine mcp --headless`: nobody can click.
    Headless,
}

/// What the policy requires for a caller.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Requirement {
    /// The token is enough.
    Token,
    /// The user must click "Confirm upload" in the GUI.
    GuiConfirmation,
    /// Not allowed (reason).
    Refused(String),
}

/// The rule table above.
pub fn requirement(actor: &Actor, host: Option<McpHost>, allow_headless: bool) -> Requirement {
    if actor.is_user() {
        return Requirement::Token;
    }
    match host {
        Some(McpHost::Gui) => Requirement::GuiConfirmation,
        Some(McpHost::Headless) if allow_headless => Requirement::Token,
        Some(McpHost::Headless) | None => Requirement::Refused(
            "uploads by an AI assistant without the Vitrine Studio app are disabled (app setting \
             `mcp_allow_headless_upload`); open the project in the Vitrine Studio app, or let the \
             user run `vitrine upload execute`"
                .into(),
        ),
    }
}

/// A pending confirmation, as sent with `upload-confirm-requested`.
#[derive(Debug, Clone, Serialize)]
pub struct ConfirmRequest {
    pub id: String,
    pub token: String,
    /// The assistant's display name ("Claude Desktop").
    pub client: String,
    /// One-line summary of the plan.
    pub summary: String,
    /// The plan (the dialog shows its table).
    pub plan: UploadPlan,
    /// Unix seconds after which the request times out.
    pub expires_at: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Approved,
    Denied,
    Timeout,
}

#[derive(Debug, Default)]
struct Inner {
    pending: BTreeMap<String, ConfirmRequest>,
    answers: BTreeMap<String, bool>,
    /// Tokens an assistant may execute once.
    grants: BTreeSet<String>,
    next: u64,
}

/// Pending GUI confirmations and the grants they produce.
#[derive(Debug, Default)]
pub struct ConfirmBroker {
    inner: Mutex<Inner>,
    changed: Condvar,
}

impl ConfirmBroker {
    /// Requests that are waiting for the user (the GUI shows them on start).
    pub fn pending(&self) -> Vec<ConfirmRequest> {
        self.lock().pending.values().cloned().collect()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Answers request `id` (from the GUI dialog). `false` when unknown.
    pub fn answer(&self, id: &str, approved: bool) -> bool {
        let mut g = self.lock();
        if !g.pending.contains_key(id) {
            return false;
        }
        g.answers.insert(id.to_owned(), approved);
        drop(g);
        self.changed.notify_all();
        true
    }

    /// Consumes the grant for `token`.
    pub fn take_grant(&self, token: &str) -> bool {
        self.lock().grants.remove(token)
    }

    fn grant(&self, token: &str) {
        self.lock().grants.insert(token.to_owned());
    }

    /// Emits the request and waits for the answer or the timeout.
    fn wait(&self, app: &AppState, mut request: ConfirmRequest, timeout: Duration) -> Outcome {
        let id = {
            let mut g = self.lock();
            g.next += 1;
            let id = format!("confirm-{}", g.next);
            request.id.clone_from(&id);
            g.pending.insert(id.clone(), request.clone());
            id
        };
        app.events().emit(
            UPLOAD_CONFIRM_REQUESTED,
            serde_json::to_value(&request).unwrap_or_default(),
        );
        let deadline = Instant::now() + timeout;
        let mut g = self.lock();
        let outcome = loop {
            if let Some(a) = g.answers.remove(&id) {
                break if a {
                    Outcome::Approved
                } else {
                    Outcome::Denied
                };
            }
            let now = Instant::now();
            if now >= deadline {
                break Outcome::Timeout;
            }
            g = self
                .changed
                .wait_timeout(g, deadline - now)
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        };
        g.pending.remove(&id);
        drop(g);
        app.events().emit(
            UPLOAD_CONFIRM_RESOLVED,
            json!({ "id": id, "outcome": outcome }),
        );
        outcome
    }
}

fn allow_headless(app: &AppState) -> bool {
    app.settings_store()
        .load()
        .is_ok_and(|s| s.mcp_allow_headless_upload)
}

/// First check of an MCP `upload_execute`, before the token is looked at:
/// refuses callers the rule table refuses (headless without the setting).
pub fn check_mcp_allowed(app: &AppState, host: McpHost) -> CmdResult<()> {
    match requirement(&Actor::current(), Some(host), allow_headless(app)) {
        Requirement::Refused(why) => Err(CommandError::new(ErrorCode::Forbidden, why)),
        _ => Ok(()),
    }
}

/// The gate an MCP `upload_execute` passes before calling the command:
/// applies the rule table; for the GUI it asks the user and waits.
/// `plan` is the verified plan of `token`.
pub fn authorize_mcp(
    app: &AppState,
    host: McpHost,
    token: &str,
    plan: &UploadPlan,
) -> CmdResult<()> {
    let actor = Actor::current();
    match requirement(&actor, Some(host), allow_headless(app)) {
        Requirement::Token => {
            app.upload().confirmations().grant(token);
            Ok(())
        }
        Requirement::Refused(why) => Err(CommandError::new(ErrorCode::Forbidden, why)),
        Requirement::GuiConfirmation => {
            let config = app.upload().config();
            let client = match &actor {
                Actor::Assistant { name, .. } => name.clone(),
                Actor::User => "assistant".into(),
            };
            let request = ConfirmRequest {
                id: String::new(),
                token: token.to_owned(),
                client,
                summary: plan.summary(),
                plan: plan.clone(),
                expires_at: super::now_secs() + config.confirm_timeout.as_secs(),
            };
            match app
                .upload()
                .confirmations()
                .wait(app, request, config.confirm_timeout)
            {
                Outcome::Approved => {
                    app.upload().confirmations().grant(token);
                    Ok(())
                }
                Outcome::Denied => Err(CommandError::new(
                    ErrorCode::NotConfirmed,
                    "the user declined the upload in the Vitrine Studio app",
                )
                .with_details(json!({ "reason": "denied" }))),
                Outcome::Timeout => Err(CommandError::new(
                    ErrorCode::NotConfirmed,
                    format!(
                        "the upload was not confirmed in the Vitrine Studio app within {} s",
                        config.confirm_timeout.as_secs()
                    ),
                )
                .with_details(json!({ "reason": "timeout" }))),
            }
        }
    }
}

/// Check inside `upload_execute`: an assistant needs a grant for `token`.
pub(crate) fn check_execute(app: &AppState, token: &str) -> CmdResult<()> {
    if Actor::current().is_user() || app.upload().confirmations().take_grant(token) {
        return Ok(());
    }
    Err(CommandError::new(
        ErrorCode::NotConfirmed,
        "uploads requested by an AI assistant need the user's confirmation (upload_execute \
         over MCP asks for it)",
    )
    .with_details(json!({ "reason": "no_confirmation" })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rule_table() {
        let user = Actor::User;
        let bot = Actor::assistant("claude-ai", None);
        assert_eq!(requirement(&user, None, false), Requirement::Token);
        assert_eq!(
            requirement(&bot, Some(McpHost::Gui), false),
            Requirement::GuiConfirmation
        );
        assert!(matches!(
            requirement(&bot, Some(McpHost::Headless), false),
            Requirement::Refused(_)
        ));
        assert_eq!(
            requirement(&bot, Some(McpHost::Headless), true),
            Requirement::Token
        );
        assert!(matches!(
            requirement(&bot, None, true),
            Requirement::Refused(_)
        ));
    }
}
