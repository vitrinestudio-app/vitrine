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

use serde::{Deserialize, Serialize};
use vitrine_core::model::StoreKind;

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

// ----- App Asset Library reference data (read-only, ADR 0019) -------------------------

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UploadRefdataArgs {
    /// Apple account id (default: the open project's Apple account, else
    /// the only Apple account of the app settings).
    #[serde(default)]
    pub account: Option<String>,
    /// App: numeric Apple ID or bundle id (default: the open project's
    /// app). Adds the app's asset library id (proves library access).
    #[serde(default)]
    pub app: Option<String>,
}

/// The app part of [`RefdataReport`].
#[derive(Debug, Clone, Serialize)]
pub struct RefdataApp {
    pub id: String,
    pub name: String,
    pub bundle_id: String,
    /// `GET /v1/apps/{id}/assetLibrary` answered: the app can use the
    /// asset library.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub library_id: Option<String>,
    /// Why the library could not be read (then `auto` uses the set API).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub library_error: Option<String>,
}

/// How one Vitrine Studio slot resolves against the real ref data.
#[derive(Debug, Clone, Serialize)]
pub struct RefdataMapping {
    /// Display type (`APP_IPHONE_67`) or creative placement (`header`).
    pub slot: String,
    pub placement_types: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_class: Option<String>,
    pub platforms: Vec<String>,
    /// Matching groups with their image sizes (`IPHONE_DUO_OUTER_PROFILE
    /// 1398x2034`), per placement type.
    pub groups: Vec<String>,
    /// Only the asset library can upload it.
    pub asset_library_only: bool,
}

/// `upload_refdata`: Apple's catalog plus how Vitrine Studio's mapping resolves.
#[derive(Debug, Clone, Serialize)]
pub struct RefdataReport {
    pub account: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app: Option<RefdataApp>,
    /// `GET /v1/appAssetLibraryRefData` as Apple sent it (camelCase).
    pub ref_data: vitrine_publish::apple::RefData,
    pub mapping: Vec<RefdataMapping>,
    /// Date `data/asset-library.yaml` was checked.
    pub mapping_checked: String,
}

/// Reads App Store Connect's asset library reference data with an Apple
/// account (read-only GETs: ref data, optionally the app and its library)
/// and resolves Vitrine Studio's slot mapping against it. For checking the
/// runtime group ids with a real key (`vitrine upload refdata`, GUI "Show
/// App Store asset types").
pub fn upload_refdata(app: &AppState, args: UploadRefdataArgs) -> CmdResult<RefdataReport> {
    let project = app.session().ok().map(|s| s.snapshot());
    let account_id = match args.account {
        Some(a) => a,
        None => {
            if let Some(a) = project
                .as_ref()
                .and_then(|p| p.store.account(StoreKind::Apple))
            {
                a.to_owned()
            } else {
                let apple: Vec<_> = app
                    .settings_store()
                    .load()?
                    .store_accounts
                    .into_iter()
                    .filter(|a| a.kind == StoreKind::Apple)
                    .collect();
                match apple.as_slice() {
                    [one] => one.id.clone(),
                    [] => {
                        return Err(CommandError::not_found(
                            "no App Store Connect account is set up (App settings → Stores)",
                        ));
                    }
                    _ => {
                        return Err(CommandError::invalid(
                            "several App Store Connect accounts: choose one (`account`)",
                        ));
                    }
                }
            }
        }
    };
    let app_ref = match args.app {
        Some(a) if !a.is_empty() && a.bytes().all(|b| b.is_ascii_digit()) => {
            Some(vitrine_publish::apple::AppRef::Id(a))
        }
        Some(a) if !a.is_empty() => Some(vitrine_publish::apple::AppRef::BundleId(a)),
        _ => project.as_ref().and_then(remote::apple_app_ref),
    };
    let (account, secret) = super::account_with_secret(app, &account_id, StoreKind::Apple)?;
    let config = app.upload().config();
    let options = vitrine_publish::ClientOptions {
        request_timeout: Duration::from_secs(60),
        ..super::interactive_options()
    };
    let (ref_data, app_info) = remote::block_on(async {
        let client = remote::apple_client(&config, &account, &secret, options)?;
        let ref_data = client
            .ref_data(&vitrine_publish::apple::RefDataFilter::default())
            .await?;
        let app_info = match &app_ref {
            None => None,
            Some(r) => {
                let found = client.find_app(r).await?;
                let (library_id, library_error) = match client.asset_library(&found.id).await {
                    Ok(l) => (Some(l.id), None),
                    Err(e) => (None, Some(remote::store_message(StoreKind::Apple, &e))),
                };
                Some(RefdataApp {
                    id: found.id,
                    name: found.name,
                    bundle_id: found.bundle_id,
                    library_id,
                    library_error,
                })
            }
        };
        Ok::<_, vitrine_publish::Error>((ref_data, app_info))
    })?
    .map_err(|e| super::store_failure(StoreKind::Apple, &e))?;
    Ok(RefdataReport {
        account: account.id,
        app: app_info,
        mapping: refdata_mapping(&ref_data),
        ref_data,
        mapping_checked: vitrine_core::catalog::asset_library().checked.clone(),
    })
}

/// Vitrine Studio's slots resolved against `rd` (all candidate groups).
fn refdata_mapping(rd: &vitrine_publish::apple::RefData) -> Vec<RefdataMapping> {
    use vitrine_core::model::{AppleSlot, CreativePlacement};
    let lib = vitrine_core::catalog::asset_library();
    let mut slots: Vec<(String, AppleSlot)> = lib
        .screenshots
        .keys()
        .map(|k| (k.clone(), AppleSlot::screenshots(k.clone())))
        .collect();
    for p in CreativePlacement::ALL {
        slots.push((p.as_str().to_owned(), AppleSlot::creative(p)));
    }
    slots
        .into_iter()
        .filter_map(|(name, slot)| {
            let sp = lib.slot(&slot)?;
            let groups = sp
                .placement_types
                .iter()
                .flat_map(|t| {
                    rd.groups_for(t, sp.display_class.as_deref(), &sp.platforms)
                        .into_iter()
                        .map(move |g| {
                            let sizes: Vec<String> = rd
                                .image_specs_for(t, &g.placement_profile_group_id)
                                .iter()
                                .map(|s| s.size_label())
                                .collect();
                            format!("{t} → {} {}", g.placement_profile_group_id, sizes.join(" "))
                                .trim_end()
                                .to_owned()
                        })
                })
                .collect();
            Some(RefdataMapping {
                slot: name,
                placement_types: sp.placement_types,
                display_class: sp.display_class,
                platforms: sp.platforms,
                groups,
                asset_library_only: sp.asset_library_only,
            })
        })
        .collect()
}
