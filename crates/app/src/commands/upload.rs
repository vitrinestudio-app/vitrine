//! Store upload commands (Phase 5.2/5.5, ADR 0018; logic in
//! [`crate::upload`]):
//!
//! - `upload_readiness {store?}` → the setup checklist (no rendering, no
//!   network): accounts, project mapping, target slots, each with `fix`;
//! - `upload_plan {store?, targets?, langs?, remote?, force?}` → the plan
//!   with its confirm token (dry run, nothing is uploaded);
//! - `upload_execute {token, wait?, changes_not_sent_for_review?}` → starts
//!   the run (or waits for it); assistants additionally need a GUI
//!   confirmation ([`crate::upload::policy`]);
//! - `upload_status`, `upload_cancel`, `upload_log {limit?, events?}`;
//! - `upload_confirm {id, approved}`: the GUI's answer to an MCP request.

use std::sync::Arc;
use std::time::Duration;

use arrangia_core::model::StoreKind;
use serde::{Deserialize, Serialize};

use super::{AppState, NoArgs};
use crate::error::{CmdResult, CommandError, ErrorCode};
use crate::upload::exec::{self, AppleJob, GoogleJob, Job, RunHandle, RunState};
use crate::upload::log::{RunRecord, UploadLog};
pub use crate::upload::plan::PlanArgs;
use crate::upload::plan::{self as plan_mod};
use crate::upload::policy::{self, ConfirmRequest};
use crate::upload::{UploadPlan, remote};

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UploadReadinessArgs {
    /// Only this store (the dialog's tab); default: both.
    #[serde(default)]
    pub store: Option<StoreKind>,
}

/// What is missing before an upload, in order, with the place to fix it.
pub fn upload_readiness(
    app: &AppState,
    args: UploadReadinessArgs,
) -> CmdResult<crate::upload::readiness::Readiness> {
    let session = app.session()?;
    let accounts = app.settings_store().load()?.store_accounts;
    Ok(session.with_project(|p| crate::upload::readiness::readiness(p, &accounts, args.store)))
}

/// Builds the plan (renders the images incrementally; `remote` reads the
/// store state with the project's account).
pub fn upload_plan(app: &AppState, args: PlanArgs) -> CmdResult<UploadPlan> {
    let session = app.session()?;
    plan_mod::make_plan(app, &session, &args)
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UploadExecuteArgs {
    /// `confirm_token` of the plan.
    pub token: String,
    /// Block until the run ends (CLI, tests); default: return right away
    /// and report through `upload-progress` / `upload_status`.
    #[serde(default)]
    pub wait: bool,
    /// Google: commit with `changesNotSentForReview=true` (needed for apps
    /// whose changes are not sent for review automatically).
    #[serde(default)]
    pub changes_not_sent_for_review: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct UploadExecuteResult {
    pub run: RunState,
}

/// Checks the token (known, unused, not expired, project unchanged, plan
/// not blocked) and returns its plan. Used by the MCP gate before asking
/// the user.
pub fn upload_verify(app: &AppState, token: &str) -> CmdResult<UploadPlan> {
    let session = app.session()?;
    plan_mod::verify_token(app, &session, token)
}

/// Starts (or runs, with `wait`) the upload of a confirmed plan.
pub fn upload_execute(app: &AppState, args: UploadExecuteArgs) -> CmdResult<UploadExecuteResult> {
    let session = app.session()?;
    policy::check_execute(app, &args.token)?;
    if app.upload().run().is_some_and(|r| r.is_running()) {
        return Err(CommandError::new(
            ErrorCode::Busy,
            "an upload is already running",
        ));
    }
    let plan = plan_mod::verify_token(app, &session, &args.token)?;
    let project = session.snapshot();
    let needs = |store: StoreKind| {
        plan.stores
            .iter()
            .any(|s| s.store == store && s.sets.iter().any(|x| x.action.uploads()))
    };
    let apple = if needs(StoreKind::Apple) {
        let (account, secret) = remote::credentials(app, &project, StoreKind::Apple)?;
        Some(AppleJob {
            account,
            secret,
            app: remote::apple_app_ref(&project)
                .ok_or_else(|| CommandError::invalid("no Apple app id or bundle id"))?,
            platform: remote::apple_platform(
                project
                    .store
                    .apple
                    .as_ref()
                    .map(|a| a.platform)
                    .unwrap_or_default(),
            ),
            flow: remote::flow_setting(&project),
        })
    } else {
        None
    };
    let google = if needs(StoreKind::Google) {
        let (_, secret) = remote::credentials(app, &project, StoreKind::Google)?;
        Some(GoogleJob {
            secret,
            package: project
                .store
                .google
                .as_ref()
                .and_then(|g| g.package_name.clone())
                .ok_or_else(|| CommandError::invalid("no Google Play package name"))?,
        })
    } else {
        None
    };
    let log = UploadLog::open(session.dir())?;
    if !log.use_plan(&args.token)? {
        return Err(CommandError::new(
            ErrorCode::NotConfirmed,
            "this confirm token was already used – make a new plan",
        )
        .with_details(serde_json::json!({ "reason": "used" })));
    }
    let run_id = log.start_run(&args.token, crate::upload::now_secs())?;
    drop(log);
    let handle = Arc::new(RunHandle::new(run_id, &args.token, &plan));
    if !app.upload().begin(handle.clone()) {
        return Err(CommandError::new(
            ErrorCode::Busy,
            "an upload is already running",
        ));
    }
    let job = Job {
        dir: session.dir().to_path_buf(),
        plan,
        config: app.upload().config(),
        apple,
        google,
        events: app.events().clone(),
        handle: handle.clone(),
        changes_not_sent_for_review: args.changes_not_sent_for_review,
    };
    exec::spawn(job)
        .map_err(|e| CommandError::internal(format!("cannot start the upload: {e}")))?;
    let run = if args.wait {
        handle.wait(None)
    } else {
        handle.state()
    };
    Ok(UploadExecuteResult { run })
}

#[derive(Debug, Clone, Serialize)]
pub struct UploadStatus {
    /// The running or last run of this app session.
    pub run: Option<RunState>,
    /// MCP upload requests waiting for the user's click.
    pub pending_confirmations: Vec<ConfirmRequest>,
}

pub fn upload_status(app: &AppState, _: NoArgs) -> CmdResult<UploadStatus> {
    Ok(UploadStatus {
        run: app.upload().run().map(|r| r.state()),
        pending_confirmations: app.upload().confirmations().pending(),
    })
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct UploadCancelResult {
    pub cancelled: bool,
}

/// Asks the running upload to stop.
pub fn upload_cancel(app: &AppState, _: NoArgs) -> CmdResult<UploadCancelResult> {
    let run = app.upload().run().filter(|r| r.is_running());
    if let Some(r) = &run {
        r.cancel();
    }
    Ok(UploadCancelResult {
        cancelled: run.is_some(),
    })
}

/// Waits for the running upload (tests, CLI); `None` = no run.
pub fn upload_wait(app: &AppState, timeout: Option<Duration>) -> Option<RunState> {
    app.upload().run().map(|r| r.wait(timeout))
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UploadLogArgs {
    /// Number of runs (default 10).
    #[serde(default)]
    pub limit: Option<usize>,
    /// Include the HTTP events (method, path, status – never secrets).
    #[serde(default)]
    pub events: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct UploadLogResult {
    /// `<project>/.cache/uploads.sqlite`.
    pub path: String,
    pub runs: Vec<RunRecord>,
}

pub fn upload_log(app: &AppState, args: UploadLogArgs) -> CmdResult<UploadLogResult> {
    let session = app.session()?;
    let log = UploadLog::open(session.dir())?;
    Ok(UploadLogResult {
        path: crate::upload::log::log_path(session.dir())
            .display()
            .to_string(),
        runs: log.runs(args.limit.unwrap_or(10).max(1), args.events)?,
    })
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UploadConfirmArgs {
    /// `id` of the `upload-confirm-requested` event.
    pub id: String,
    pub approved: bool,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct UploadConfirmResult {
    /// The request was still waiting.
    pub ok: bool,
}

/// The user's answer to an assistant's upload request (GUI dialog only).
pub fn upload_confirm(app: &AppState, args: UploadConfirmArgs) -> CmdResult<UploadConfirmResult> {
    if !crate::Actor::current().is_user() {
        return Err(CommandError::new(
            ErrorCode::Forbidden,
            "only the user can confirm an upload",
        ));
    }
    Ok(UploadConfirmResult {
        ok: app.upload().confirmations().answer(&args.id, args.approved),
    })
}
