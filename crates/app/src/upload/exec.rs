//! The executor (Phase 5.5): runs a confirmed plan on its own thread.
//!
//! - **Apple**, per set: the flow ([`super::remote::AppleUploadFlow`])
//!   replaces the whole set (create localization/set if needed → delete
//!   the old screenshots → upload in order → set the order → verify).
//! - **Google**, one edit for all languages and image types: `deleteall`
//!   per language + type → upload in order → commit; on any failure or
//!   cancel the edit is deleted (nothing changes in Play).
//! - **Progress**: `upload-progress` events (step, set, file, bytes) and the
//!   [`RunState`] behind `upload_status`; `upload-finished` with the report.
//! - **Cancel**: [`RunHandle::cancel`] aborts in-flight requests; the
//!   current Apple set stays half-done (the next run replaces it), Google's
//!   edit is deleted.
//! - **Resume**: every set is logged with its image hashes; a set is
//!   `done` only when the store holds it (Apple: verified order, Google:
//!   committed). The next plan marks done sets `unchanged`, so a re-run
//!   uploads only what is missing.
//! - **Rate limits**: a `RateLimited` error pauses the set (status
//!   `paused`, `Retry-After` or one minute, at most
//!   [`super::UploadConfig::max_rate_limit_pause`]) and retries it
//!   [`super::UploadConfig::rate_limit_retries`] times.
//! - **Apple processing** (ADR 0018, "Processing"): all images of a set are
//!   reserved/uploaded/committed first, then polled together for up to
//!   [`super::UploadConfig::processing_budget`]. Images Apple is still
//!   processing afterwards are no failure: the set ends
//!   `uploaded_processing` (file status `processing`), the next plan with
//!   the remote check confirms them; `FAILED` fails the set with Apple's
//!   errors. Images the store already holds (same checksum) are kept.
//! - **Store-level failures** (no editable App Store version, app not
//!   reachable) are reported once per store ([`RunState::store_failures`]);
//!   its sets are `skipped` (no editable version) or `failed` with a short
//!   pointer, never the full message per set.
//! - **Watchdog** ([`spawn`]): writes the run's heartbeat to the log, and a
//!   run (not paused) without progress for
//!   [`super::UploadConfig::stall_timeout`] is cancelled and ends `failed`;
//!   if the executor does not return within `stall_grace` (or panics) the
//!   run is ended without it. Runs of a closed/crashed app are
//!   `interrupted` when the log is opened next ([`super::log`]).
//! - **Log**: runs, sets, files, redacted HTTP events (with the time they
//!   happened; Apple's poll requests only as state transitions) →
//!   `uploads.sqlite`.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, PoisonError};
use std::time::{Duration, Instant};

use arrangia_core::model::{AppleUploadFlowSetting, StoreKind};
use arrangia_publish::apple::{AppRef, Platform};
use arrangia_publish::credentials::Secret;
use arrangia_publish::{
    CancellationToken, ClientOptions, Error, HttpEvent, HttpEventKind, Progress, UploadStep, google,
};
use serde::Serialize;
use serde_json::json;

use super::library::AssetLibraryFlow;
use super::log::UploadLog;
use super::remote::{
    self, AppleUploadFlow as _, ReplaceOptions, Route, ScreenshotSetFlow, SetHooks, UploadFile,
};
use super::{
    PlanSet, StorePlan, UPLOAD_FINISHED, UPLOAD_PROGRESS, UploadConfig, UploadPlan, now_secs,
    sha256_hex,
};
use crate::events::EventSink;
use crate::stores::StoreAccount;

/// Status of a run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    Running,
    /// Waiting for a store rate limit.
    Paused,
    /// Every set is in the store.
    Completed,
    /// Some sets failed; the others are in the store.
    Partial,
    Failed,
    Cancelled,
}

/// A store that could not be used at all (reported once, not per set).
#[derive(Debug, Clone, Serialize)]
pub struct StoreFailure {
    pub store: StoreKind,
    /// `no_editable_version`, `app_not_reachable`, `credentials`.
    pub code: String,
    /// Short, human message (the details are in the log).
    pub message: String,
    /// Values for the localized message (GUI `upload.problem.<code>`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vars: Option<serde_json::Value>,
    /// Page that fixes it (`open_url`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub link: Option<String>,
}

/// Status of one set in a run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SetRunStatus {
    Pending,
    Running,
    Paused,
    Done,
    /// In the store, but Apple was still processing some images when the
    /// processing budget ran out (not a failure; re-check later).
    UploadedProcessing,
    Failed,
    Cancelled,
    /// `unchanged` / `skip` in the plan.
    Skipped,
}

/// One set of a run.
#[derive(Debug, Clone, Serialize)]
pub struct SetRun {
    pub key: String,
    pub store: StoreKind,
    pub target: String,
    pub lang: String,
    pub locale: Option<String>,
    pub slot: String,
    pub status: SetRunStatus,
    /// Images in the store (Apple: `COMPLETE`; kept ones included).
    pub files_done: usize,
    pub files_total: usize,
    /// Apple: images committed and still processing.
    #[serde(default)]
    pub files_processing: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Something to know that is not an error (processing, kept images).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// Snapshot of a run (`upload_status`, `upload-finished`).
#[derive(Debug, Clone, Serialize)]
pub struct RunState {
    pub run_id: i64,
    pub status: RunStatus,
    /// What happens right now / why it ended.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    /// Unix seconds when a rate-limit pause ends.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub paused_until: Option<u64>,
    pub started_at: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finished_at: Option<u64>,
    pub sets: Vec<SetRun>,
    pub images_done: usize,
    pub images_total: usize,
    /// Apple: images uploaded and still being processed.
    #[serde(default)]
    pub images_processing: usize,
    pub bytes_done: u64,
    pub bytes_total: u64,
    /// Stores that could not be used at all (once per store).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub store_failures: Vec<StoreFailure>,
}

/// The final state.
pub type RunReport = RunState;

impl RunState {
    pub fn is_finished(&self) -> bool {
        !matches!(self.status, RunStatus::Running | RunStatus::Paused)
    }
}

/// A run: state, cancel token, completion.
#[derive(Debug)]
pub struct RunHandle {
    pub token: String,
    cancel: CancellationToken,
    state: Mutex<RunState>,
    finished: Condvar,
    /// Last progress (watchdog).
    activity: Mutex<Instant>,
    /// The watchdog stopped the run (no progress).
    stalled: AtomicBool,
    /// The final report is written (by the executor or the watchdog).
    finalized: AtomicBool,
}

impl RunHandle {
    pub(crate) fn new(run_id: i64, token: &str, plan: &UploadPlan) -> Self {
        let sets: Vec<SetRun> = plan
            .sets()
            .map(|(sp, s)| SetRun {
                key: s.key.clone(),
                store: sp.store,
                target: s.target.clone(),
                lang: s.lang.clone(),
                locale: s.locale.clone(),
                slot: s.slot.clone(),
                status: if s.action.uploads() {
                    SetRunStatus::Pending
                } else {
                    SetRunStatus::Skipped
                },
                files_done: 0,
                files_total: if s.action.uploads() {
                    s.images.len()
                } else {
                    0
                },
                files_processing: 0,
                error: None,
                note: None,
            })
            .collect();
        Self {
            token: token.to_owned(),
            cancel: CancellationToken::new(),
            state: Mutex::new(RunState {
                run_id,
                status: RunStatus::Running,
                message: None,
                paused_until: None,
                started_at: now_secs(),
                finished_at: None,
                sets,
                images_done: 0,
                images_total: plan.totals.images,
                images_processing: 0,
                bytes_done: 0,
                bytes_total: plan.totals.bytes,
                store_failures: Vec::new(),
            }),
            finished: Condvar::new(),
            activity: Mutex::new(Instant::now()),
            stalled: AtomicBool::new(false),
            finalized: AtomicBool::new(false),
        }
    }

    pub fn state(&self) -> RunState {
        self.lock().clone()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, RunState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub fn is_running(&self) -> bool {
        !self.lock().is_finished()
    }

    /// Asks the run to stop (in-flight requests are aborted).
    pub fn cancel(&self) {
        self.cancel.cancel();
    }

    /// Waits until the run ended (or `timeout`); returns the state.
    pub fn wait(&self, timeout: Option<Duration>) -> RunState {
        let mut g = self.lock();
        let deadline = timeout.map(|t| Instant::now() + t);
        while !g.is_finished() {
            match deadline {
                None => {
                    g = self
                        .finished
                        .wait(g)
                        .unwrap_or_else(PoisonError::into_inner);
                }
                Some(d) => {
                    let now = Instant::now();
                    if now >= d {
                        break;
                    }
                    g = self
                        .finished
                        .wait_timeout(g, d - now)
                        .unwrap_or_else(PoisonError::into_inner)
                        .0;
                }
            }
        }
        g.clone()
    }

    fn set_mut<R>(&self, key: &str, f: impl FnOnce(&mut SetRun) -> R) -> Option<R> {
        self.lock().sets.iter_mut().find(|s| s.key == key).map(f)
    }

    /// Records progress (watchdog).
    fn touch(&self) {
        *self.activity.lock().unwrap_or_else(PoisonError::into_inner) = Instant::now();
    }

    fn idle_for(&self) -> Duration {
        self.activity
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .elapsed()
    }
}

/// Everything a run needs (built by `upload_execute`).
pub(crate) struct Job {
    pub dir: PathBuf,
    pub plan: UploadPlan,
    pub config: UploadConfig,
    pub apple: Option<AppleJob>,
    pub google: Option<GoogleJob>,
    pub events: Arc<dyn EventSink>,
    pub handle: Arc<RunHandle>,
    pub changes_not_sent_for_review: bool,
}

pub(crate) struct AppleJob {
    pub account: StoreAccount,
    pub secret: Secret,
    pub app: AppRef,
    pub platform: Platform,
    /// `store.apple.upload_flow` (ADR 0019).
    pub flow: AppleUploadFlowSetting,
}

pub(crate) struct GoogleJob {
    pub secret: Secret,
    pub package: String,
}

/// Redacted HTTP events with the time they happened, buffered until the
/// runner flushes them (on every progress step).
type EventBuffer = Arc<Mutex<Vec<(u64, String, String)>>>;

struct Runner<'a> {
    job: &'a Job,
    log: Mutex<UploadLog>,
    run_id: i64,
    http: EventBuffer,
}

impl Runner<'_> {
    fn log(&self) -> std::sync::MutexGuard<'_, UploadLog> {
        self.log.lock().unwrap_or_else(PoisonError::into_inner)
    }

    #[allow(clippy::needless_pass_by_value)] // call sites build the value inline
    fn emit(&self, step: &str, set: Option<&SetRun>, file: Option<serde_json::Value>) {
        self.job.handle.touch();
        self.flush_http();
        let s = self.job.handle.state();
        self.job.events.emit(
            UPLOAD_PROGRESS,
            json!({
                "run_id": self.run_id,
                "status": s.status,
                "step": step,
                "set": set,
                "file": file,
                "images_done": s.images_done,
                "images_total": s.images_total,
                "images_processing": s.images_processing,
                "bytes_done": s.bytes_done,
                "bytes_total": s.bytes_total,
                "paused_until": s.paused_until,
            }),
        );
    }

    fn note(&self, level: &str, message: &str) {
        let _ = self.log().event(self.run_id, level, message, now_secs());
    }

    fn flush_http(&self) {
        let events: Vec<_> =
            std::mem::take(&mut *self.http.lock().unwrap_or_else(PoisonError::into_inner));
        if events.is_empty() {
            return;
        }
        let log = self.log();
        for (at, level, msg) in events {
            let _ = log.event(self.run_id, &level, &msg, at);
        }
    }

    fn set_status(&self, key: &str, status: SetRunStatus, error: Option<String>) -> Option<SetRun> {
        self.job.handle.set_mut(key, |s| {
            s.status = status;
            s.error = error;
            s.clone()
        })
    }

    fn options(&self) -> ClientOptions {
        let buf = self.http.clone();
        ClientOptions {
            cancel: self.job.handle.cancel.clone(),
            observer: Some(Arc::new(move |e: &HttpEvent| {
                let (level, msg) = match &e.kind {
                    HttpEventKind::Request => return,
                    // Apple's processing polls: only state transitions are
                    // logged (by the hooks), not every request.
                    HttpEventKind::Response { status, .. }
                        if *status < 400 && e.method == "GET" && is_screenshot_poll(&e.url) =>
                    {
                        return;
                    }
                    HttpEventKind::Response { status, elapsed } => (
                        if *status >= 400 { "warn" } else { "http" },
                        format!(
                            "{} {} → {status} ({} ms)",
                            e.method,
                            e.url,
                            elapsed.as_millis()
                        ),
                    ),
                    HttpEventKind::NetworkError { message } => (
                        "warn",
                        format!("{} {} → network error: {message}", e.method, e.url),
                    ),
                    HttpEventKind::RetryScheduled { delay, reason } => (
                        "warn",
                        format!(
                            "{} {} → retry in {} ms ({reason})",
                            e.method,
                            e.url,
                            delay.as_millis()
                        ),
                    ),
                };
                buf.lock().unwrap_or_else(PoisonError::into_inner).push((
                    now_secs(),
                    level.to_owned(),
                    msg,
                ));
            })),
            ..self.job.config.options.clone()
        }
    }

    /// Reads the plan's images and checks their hashes.
    fn files(&self, set: &PlanSet) -> Result<Vec<UploadFile>, String> {
        let root = super::plan::images_dir(&self.job.dir);
        set.images
            .iter()
            .map(|img| {
                let path = root.join(&img.file);
                let bytes = std::fs::read(&path)
                    .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
                if sha256_hex(&bytes) != img.sha256 {
                    return Err(format!(
                        "{} changed since the plan – make a new plan",
                        img.file_name
                    ));
                }
                Ok(UploadFile {
                    image: img.clone(),
                    bytes,
                })
            })
            .collect()
    }

    fn start_set(&self, sp: &StorePlan, set: &PlanSet) -> i64 {
        let hashes: Vec<String> = set.images.iter().map(|i| i.sha256.clone()).collect();
        self.log()
            .start_set(
                self.run_id,
                sp.store.as_str(),
                sp.app.as_deref().unwrap_or_default(),
                set.locale.as_deref().unwrap_or_default(),
                &set.slot,
                &set.target,
                &set.lang,
                &hashes,
                now_secs(),
            )
            .unwrap_or(-1)
    }

    fn finish_set(&self, row: i64, key: &str, status: SetRunStatus, error: Option<String>) {
        let s = match status {
            SetRunStatus::Done => "done",
            SetRunStatus::UploadedProcessing => "uploaded_processing",
            SetRunStatus::Cancelled => "cancelled",
            SetRunStatus::Skipped => "skipped",
            _ => "failed",
        };
        let _ = self.log().finish_set(row, s, error.as_deref(), now_secs());
        self.flush_http();
        let snapshot = self.set_status(key, status, error);
        self.emit("set_finished", snapshot.as_ref(), None);
    }

    /// Waits out a rate limit; `false` = give up (too long, too often, cancelled).
    async fn pause(&self, key: &str, attempt: u32, retry_after: Option<Duration>) -> bool {
        let cfg = &self.job.config;
        let wait = retry_after.unwrap_or(cfg.default_rate_limit_pause);
        if attempt >= cfg.rate_limit_retries || wait > cfg.max_rate_limit_pause {
            return false;
        }
        let until = now_secs() + wait.as_secs();
        {
            let mut s = self.job.handle.lock();
            s.status = RunStatus::Paused;
            s.paused_until = Some(until);
            s.message = Some(format!(
                "rate limit reached – waiting {} s before retrying",
                wait.as_secs()
            ));
        }
        let snap = self.set_status(key, SetRunStatus::Paused, None);
        self.note(
            "warn",
            &format!("{key}: rate limited, pausing {} s", wait.as_secs()),
        );
        self.emit("paused", snap.as_ref(), None);
        let cancelled = tokio::time::timeout(wait, self.job.handle.cancel.cancelled())
            .await
            .is_ok();
        {
            let mut s = self.job.handle.lock();
            s.status = RunStatus::Running;
            s.paused_until = None;
            s.message = None;
        }
        let snap = self.set_status(key, SetRunStatus::Running, None);
        self.emit("resumed", snap.as_ref(), None);
        !cancelled
    }

    // ---- Apple ---------------------------------------------------------------------

    /// The store cannot be used at all: one report, its sets are `skipped`
    /// (`skip_reason`) or `failed` with a short pointer.
    fn store_failed(
        &self,
        sp: &StorePlan,
        sets: &[&PlanSet],
        failure: StoreFailure,
        skip_reason: Option<&str>,
    ) {
        self.note(
            "error",
            &format!("{}: {} ({})", sp.store, failure.message, failure.code),
        );
        let short = format!("not uploaded: {}", failure.message);
        self.job.handle.lock().store_failures.push(failure);
        for s in sets {
            if let Some(reason) = skip_reason {
                let snap = self.set_status(&s.key, SetRunStatus::Skipped, Some(reason.to_owned()));
                self.emit("set_finished", snap.as_ref(), None);
            } else {
                let row = self.start_set(sp, s);
                self.finish_set(row, &s.key, SetRunStatus::Failed, Some(short.clone()));
            }
        }
    }

    #[allow(clippy::too_many_lines)] // one linear flow
    async fn apple(&self, sp: &StorePlan, job: &AppleJob) {
        let sets: Vec<&PlanSet> = sp.sets.iter().filter(|s| s.action.uploads()).collect();
        if sets.is_empty() {
            return;
        }
        let failure = |code: &str, e: &Error| StoreFailure {
            store: StoreKind::Apple,
            code: code.to_owned(),
            message: remote::store_message(StoreKind::Apple, e),
            vars: None,
            link: None,
        };
        let client =
            match remote::apple_client(&self.job.config, &job.account, &job.secret, self.options())
            {
                Ok(c) => c,
                Err(e) => return self.store_failed(sp, &sets, failure("credentials", &e), None),
            };
        self.emit("prepare", None, Some(json!({ "store": "apple" })));
        let session = match remote::open_apple(&client, &job.app, job.platform, job.flow).await {
            Ok(s) => s,
            Err(Error::Cancelled) => return self.cancel_rest(sp),
            Err(e) => {
                if let Some(p) = super::plan::no_editable_version_problem(&e, sp.app.as_deref()) {
                    // The full version list goes to the log only.
                    let versions: Vec<String> = e
                        .details()
                        .iter()
                        .skip(1)
                        .map(|d| format!("{} {}", d.title, d.detail))
                        .collect();
                    self.note("info", &format!("apple: versions: {}", versions.join(", ")));
                    let f = StoreFailure {
                        store: StoreKind::Apple,
                        code: p.code,
                        message: p.message,
                        vars: p.vars,
                        link: p.link,
                    };
                    return self.store_failed(sp, &sets, f, Some("no editable App Store version"));
                }
                let code = if e.kind() == arrangia_publish::ErrorKind::Auth {
                    "credentials"
                } else {
                    "app_not_reachable"
                };
                return self.store_failed(sp, &sets, failure(code, &e), None);
            }
        };
        if let Some(why) = &session.fallback {
            self.note("warn", &format!("apple: {why}"));
        }
        self.note(
            "info",
            &format!("apple: upload flow {}", session.kind.as_str()),
        );
        let target = &session.target;
        let opts = ReplaceOptions {
            reuse: !self.job.plan.selection.force,
            budget: self.job.config.processing_budget,
            poll: self.job.config.options.poll,
        };
        for set in sets {
            if self.job.handle.cancel.is_cancelled() {
                return self.cancel_rest(sp);
            }
            let row = self.start_set(sp, set);
            let files = match self.files(set) {
                Ok(f) => f,
                Err(e) => {
                    self.finish_set(row, &set.key, SetRunStatus::Failed, Some(e));
                    continue;
                }
            };
            let route = match remote::route(&session, &set.slot, &set.images) {
                Ok(r) => r,
                Err(why) => {
                    self.finish_set(row, &set.key, SetRunStatus::Failed, Some(why));
                    continue;
                }
            };
            if let Route::Sets { note: Some(n) } = &route {
                self.note("info", &format!("{}: {n}", set.key));
            }
            let snap = self.set_status(&set.key, SetRunStatus::Running, None);
            self.emit("set_started", snap.as_ref(), None);
            let locale = set.locale.clone().unwrap_or_default();
            let mut attempt = 0;
            let result = loop {
                let hooks = Hooks::new(self, set, row);
                let replaced = match &route {
                    Route::Library(_) => {
                        AssetLibraryFlow
                            .replace(&client, target, &locale, &set.slot, &files, opts, &hooks)
                            .await
                    }
                    Route::Sets { .. } => {
                        ScreenshotSetFlow
                            .replace(&client, target, &locale, &set.slot, &files, opts, &hooks)
                            .await
                    }
                };
                match replaced {
                    Err(Error::RateLimited { retry_after })
                        if self.pause(&set.key, attempt, retry_after).await =>
                    {
                        attempt += 1;
                        self.reset_files(&set.key);
                    }
                    other => break other,
                }
            };
            match result {
                Ok(out) => {
                    let mut notes = Vec::new();
                    if out.kept > 0 {
                        notes.push(format!(
                            "{} image(s) were already in the store and kept",
                            out.kept
                        ));
                    }
                    if let Some(n) = &out.note {
                        notes.push(n.clone());
                    }
                    let status = if out.processing > 0 {
                        notes.insert(
                            0,
                            format!(
                                "Apple is still processing {} image(s) – check the store state later (plan with remote check)",
                                out.processing
                            ),
                        );
                        SetRunStatus::UploadedProcessing
                    } else {
                        SetRunStatus::Done
                    };
                    if !notes.is_empty() {
                        let note = notes.join("; ");
                        self.note("info", &format!("{}: {note}", set.key));
                        self.job.handle.set_mut(&set.key, |s| s.note = Some(note));
                    }
                    self.finish_set(row, &set.key, status, None);
                }
                Err(Error::Cancelled) => {
                    self.finish_set(
                        row,
                        &set.key,
                        SetRunStatus::Cancelled,
                        Some(
                            "cancelled – the set may be incomplete; the next run completes it"
                                .into(),
                        ),
                    );
                    return self.cancel_rest(sp);
                }
                Err(e) => {
                    let msg = remote::store_message(StoreKind::Apple, &e);
                    let fatal = e.kind() == arrangia_publish::ErrorKind::Auth;
                    self.finish_set(row, &set.key, SetRunStatus::Failed, Some(msg.clone()));
                    if fatal {
                        return self.fail_rest(sp, &msg);
                    }
                }
            }
        }
    }

    fn reset_files(&self, key: &str) {
        self.job.handle.set_mut(key, |s| {
            s.files_done = 0;
            s.files_processing = 0;
        });
    }

    /// Marks the store's pending sets cancelled.
    fn cancel_rest(&self, sp: &StorePlan) {
        self.mark_pending(sp, SetRunStatus::Cancelled, "cancelled before it started");
    }

    fn fail_rest(&self, sp: &StorePlan, msg: &str) {
        self.mark_pending(sp, SetRunStatus::Failed, msg);
    }

    fn mark_pending(&self, sp: &StorePlan, status: SetRunStatus, msg: &str) {
        let keys: Vec<String> = sp.sets.iter().map(|s| s.key.clone()).collect();
        let mut g = self.job.handle.lock();
        for s in g.sets.iter_mut().filter(|s| keys.contains(&s.key)) {
            if s.status == SetRunStatus::Pending {
                s.status = status;
                s.error = Some(msg.to_owned());
            }
        }
    }

    // ---- Google --------------------------------------------------------------------

    #[allow(clippy::too_many_lines)]
    async fn google(&self, sp: &StorePlan, job: &GoogleJob) {
        let sets: Vec<&PlanSet> = sp.sets.iter().filter(|s| s.action.uploads()).collect();
        if sets.is_empty() {
            return;
        }
        let rows: Vec<i64> = sets.iter().map(|s| self.start_set(sp, s)).collect();
        let fail_all = |msg: &str, status: SetRunStatus| {
            for (s, row) in sets.iter().zip(&rows) {
                self.finish_set(*row, &s.key, status, Some(msg.to_owned()));
            }
        };
        let client = match remote::google_client(
            &self.job.config,
            &job.secret,
            &job.package,
            self.options(),
        ) {
            Ok(c) => c,
            Err(e) => {
                return fail_all(
                    &remote::store_message(StoreKind::Google, &e),
                    SetRunStatus::Failed,
                );
            }
        };
        // Cleanup must work after a cancel, so it gets its own token.
        let cleanup = remote::google_client(
            &self.job.config,
            &job.secret,
            &job.package,
            ClientOptions {
                cancel: CancellationToken::new(),
                ..self.options()
            },
        )
        .ok();
        self.emit("prepare", None, Some(json!({ "store": "google" })));
        let edit = match client.insert_edit().await {
            Ok(e) => e,
            Err(Error::Cancelled) => {
                return fail_all("cancelled before it started", SetRunStatus::Cancelled);
            }
            Err(e) => {
                return fail_all(
                    &remote::store_message(StoreKind::Google, &e),
                    SetRunStatus::Failed,
                );
            }
        };
        let abort = async |msg: &str, status: SetRunStatus| {
            if let Some(c) = &cleanup {
                let _ = c.delete_edit(&edit.id).await;
            }
            self.note("warn", &format!("google: edit discarded ({msg})"));
            fail_all(msg, status);
        };
        for (set, row) in sets.iter().zip(&rows) {
            if self.job.handle.cancel.is_cancelled() {
                return abort(
                    "cancelled – nothing was changed in Google Play",
                    SetRunStatus::Cancelled,
                )
                .await;
            }
            let files = match self.files(set) {
                Ok(f) => f,
                Err(e) => return abort(&e, SetRunStatus::Failed).await,
            };
            let Some(kind) = google::ImageType::parse(&set.slot) else {
                return abort(
                    &format!("unknown image type `{}`", set.slot),
                    SetRunStatus::Failed,
                )
                .await;
            };
            let snap = self.set_status(&set.key, SetRunStatus::Running, None);
            self.emit("set_started", snap.as_ref(), None);
            let locale = set.locale.clone().unwrap_or_default();
            let mut attempt = 0;
            let result = loop {
                let hooks = Hooks::new(self, set, *row);
                match self
                    .google_set(&client, &edit.id, &locale, kind, &files, &hooks)
                    .await
                {
                    Err(Error::RateLimited { retry_after })
                        if self.pause(&set.key, attempt, retry_after).await =>
                    {
                        attempt += 1;
                        self.reset_files(&set.key);
                    }
                    other => break other,
                }
            };
            match result {
                Ok(()) => {
                    let snap = self.job.handle.set_mut(&set.key, |s| s.clone());
                    self.emit("set_uploaded", snap.as_ref(), None);
                }
                Err(Error::Cancelled) => {
                    return abort(
                        "cancelled – nothing was changed in Google Play",
                        SetRunStatus::Cancelled,
                    )
                    .await;
                }
                Err(e) => {
                    let msg = format!(
                        "{} – the edit was discarded, nothing was changed in Google Play",
                        remote::store_message(StoreKind::Google, &e)
                    );
                    return abort(&msg, SetRunStatus::Failed).await;
                }
            }
        }
        self.emit("commit", None, Some(json!({ "store": "google" })));
        match client
            .commit(&edit.id, self.job.changes_not_sent_for_review)
            .await
        {
            Ok(_) => {
                for (s, row) in sets.iter().zip(&rows) {
                    self.finish_set(*row, &s.key, SetRunStatus::Done, None);
                }
            }
            Err(Error::Cancelled) => {
                abort(
                    "cancelled before the commit – nothing was changed in Google Play",
                    SetRunStatus::Cancelled,
                )
                .await;
            }
            Err(e) => {
                let mut msg = format!(
                    "commit failed: {}",
                    remote::store_message(StoreKind::Google, &e)
                );
                if msg.contains("changesNotSentForReview") {
                    msg.push_str(
                        " (this app needs `changes not sent for review`: run again with that option)",
                    );
                }
                abort(&msg, SetRunStatus::Failed).await;
            }
        }
    }

    async fn google_set(
        &self,
        client: &google::Client,
        edit: &str,
        locale: &str,
        kind: google::ImageType,
        files: &[UploadFile],
        hooks: &Hooks<'_>,
    ) -> Result<(), Error> {
        hooks.step("delete_existing");
        client.delete_all(edit, locale, kind).await?;
        for (i, f) in files.iter().enumerate() {
            hooks.file_started(i);
            let progress = |p: &Progress| hooks.progress(i, p);
            let mime = if std::path::Path::new(&f.image.file)
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("jpg"))
            {
                "image/jpeg"
            } else {
                "image/png"
            };
            let img = client
                .upload_image(edit, locale, kind, f.bytes.clone(), mime, &progress)
                .await?;
            hooks.file_done(i, &img.id);
        }
        Ok(())
    }
}

/// Whether a redacted URL is Apple's processing poll (`GET
/// …/v1/appScreenshots/<id>` or `…/v1/appAssetLibraryImages/<id>`).
fn is_screenshot_poll(url: &str) -> bool {
    let path = url.split_once("://").map_or(url, |(_, rest)| rest);
    let segs: Vec<&str> = path.split('/').collect();
    segs.len() >= 3
        && segs[segs.len() - 3] == "v1"
        && matches!(
            segs[segs.len() - 2],
            "appScreenshots" | "appAssetLibraryImages"
        )
        && !segs[segs.len() - 1].is_empty()
}

fn mmss(d: Duration) -> String {
    let s = d.as_secs();
    format!("{}:{:02}", s / 60, s % 60)
}

/// [`SetHooks`] of one set: state, events, file log.
struct Hooks<'a> {
    runner: &'a Runner<'a>,
    set: &'a PlanSet,
    row: i64,
    /// Position → file row in the log.
    files: Mutex<HashMap<usize, i64>>,
    /// Positions whose bytes are counted.
    counted: Mutex<HashSet<usize>>,
    /// Positions counted as processing.
    processing: Mutex<HashSet<usize>>,
}

impl<'a> Hooks<'a> {
    fn new(runner: &'a Runner<'a>, set: &'a PlanSet, row: i64) -> Self {
        Self {
            runner,
            set,
            row,
            files: Mutex::new(HashMap::new()),
            counted: Mutex::new(HashSet::new()),
            processing: Mutex::new(HashSet::new()),
        }
    }

    fn file_json(&self, index: usize, extra: serde_json::Value) -> serde_json::Value {
        let img = &self.set.images[index];
        let mut v = json!({"index": img.index, "file_name": img.file_name,
                           "bytes_total": img.bytes});
        if let (Some(o), serde_json::Value::Object(e)) = (v.as_object_mut(), extra) {
            o.extend(e);
        }
        v
    }

    fn snap(&self) -> Option<SetRun> {
        self.runner.job.handle.set_mut(&self.set.key, |s| s.clone())
    }

    /// Counts the bytes of a file once.
    fn count_bytes(&self, index: usize) {
        if self
            .counted
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(index)
        {
            self.runner.job.handle.lock().bytes_done += self.set.images[index].bytes;
        }
    }

    fn set_processing(&self, index: usize, on: bool) {
        let mut p = self
            .processing
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let changed = if on {
            p.insert(index)
        } else {
            p.remove(&index)
        };
        if changed {
            let delta = |v: &mut usize| {
                if on {
                    *v += 1;
                } else {
                    *v = v.saturating_sub(1);
                }
            };
            delta(&mut self.runner.job.handle.lock().images_processing);
            self.runner
                .job
                .handle
                .set_mut(&self.set.key, |s| delta(&mut s.files_processing));
        }
    }

    fn log_file(&self, index: usize, store_id: &str, status: &str, error: Option<&str>) {
        let img = &self.set.images[index];
        let mut files = self.files.lock().unwrap_or_else(PoisonError::into_inner);
        let log = self.runner.log();
        if let Some(id) = files.get(&index) {
            let _ = log.update_file(*id, status, error);
        } else if let Ok(id) = log.file(
            self.row,
            img.index,
            &img.file_name,
            &img.sha256,
            img.bytes,
            Some(store_id),
            status,
            error,
        ) {
            files.insert(index, id);
        }
    }
}

impl SetHooks for Hooks<'_> {
    fn file_started(&self, index: usize) {
        self.runner.emit(
            "file_started",
            self.snap().as_ref(),
            Some(self.file_json(index, json!({"bytes_sent": 0}))),
        );
    }

    fn progress(&self, index: usize, p: &Progress) {
        let step = match p.step {
            UploadStep::Transfer if p.bytes_sent >= p.bytes_total && p.bytes_total > 0 => {
                "file_commit"
            }
            UploadStep::Transfer => "transfer",
            UploadStep::Reserve => "file_reserved",
            _ => return,
        };
        self.runner.emit(
            step,
            self.snap().as_ref(),
            Some(self.file_json(index, json!({"bytes_sent": p.bytes_sent}))),
        );
    }

    fn file_uploaded(&self, index: usize, store_id: &str) {
        self.count_bytes(index);
        self.set_processing(index, true);
        self.log_file(index, store_id, "processing", None);
        self.runner.emit(
            "file_uploaded",
            self.snap().as_ref(),
            Some(self.file_json(
                index,
                json!({"bytes_sent": self.set.images[index].bytes, "store_id": store_id}),
            )),
        );
    }

    fn file_kept(&self, index: usize, store_id: &str, processing: bool) {
        self.count_bytes(index);
        let img = &self.set.images[index];
        self.runner.note(
            "info",
            &format!(
                "{} #{} {}: already in the store ({store_id}), kept",
                self.set.key, img.index, img.file_name
            ),
        );
        if processing {
            self.set_processing(index, true);
            self.log_file(index, store_id, "processing", None);
        } else {
            self.file_done(index, store_id);
        }
    }

    fn file_state(&self, index: usize, state: &str, elapsed: Duration) {
        let img = &self.set.images[index];
        self.runner.note(
            "info",
            &format!(
                "{} #{} {}: {state} after {}",
                self.set.key,
                img.index,
                img.file_name,
                mmss(elapsed)
            ),
        );
    }

    fn processing(&self, pending: &[(usize, String)], elapsed: Duration) {
        if pending.is_empty() {
            return;
        }
        let (first, state) = &pending[0];
        self.runner.emit(
            "processing",
            self.snap().as_ref(),
            Some(self.file_json(
                *first,
                json!({"pending": pending.len(), "state": state,
                       "elapsed_secs": elapsed.as_secs()}),
            )),
        );
    }

    fn file_done(&self, index: usize, store_id: &str) {
        self.count_bytes(index);
        self.set_processing(index, false);
        self.log_file(index, store_id, "done", None);
        self.runner.job.handle.lock().images_done += 1;
        let snap = self.runner.job.handle.set_mut(&self.set.key, |s| {
            s.files_done += 1;
            s.clone()
        });
        let img = &self.set.images[index];
        self.runner.emit(
            "file_done",
            snap.as_ref(),
            Some(self.file_json(
                index,
                json!({"bytes_sent": img.bytes, "store_id": store_id}),
            )),
        );
    }

    fn file_failed(&self, index: usize, store_id: &str, message: &str) {
        self.set_processing(index, false);
        self.log_file(index, store_id, "failed", Some(message));
        let img = &self.set.images[index];
        self.runner.note(
            "error",
            &format!(
                "{} #{} {}: Apple could not process it: {message}",
                self.set.key, img.index, img.file_name
            ),
        );
        self.runner.emit(
            "file_failed",
            self.snap().as_ref(),
            Some(self.file_json(index, json!({"error": message}))),
        );
    }

    fn step(&self, step: &str) {
        self.runner.emit(step, self.snap().as_ref(), None);
    }
}

/// Starts the run on its own thread plus its watchdog (heartbeat, stall
/// detection, panic safety net).
pub(crate) fn spawn(job: Job) -> std::io::Result<()> {
    let run_id = job.handle.state().run_id;
    let handle = job.handle.clone();
    let events = job.events.clone();
    let dir = job.dir.clone();
    let config = job.config.clone();
    std::thread::Builder::new()
        .name(format!("upload-{run_id}"))
        .spawn(move || {
            let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run(&job)));
            if caught.is_err() {
                end_without_executor(
                    &job.dir,
                    &job.handle,
                    &*job.events,
                    "internal error: the upload task crashed – plan again to resume",
                );
            }
        })?;
    std::thread::Builder::new()
        .name(format!("upload-{run_id}-watchdog"))
        .spawn(move || watchdog(&dir, &handle, &*events, &config))?;
    Ok(())
}

/// Heartbeat + stall detection until the run is finished.
fn watchdog(
    dir: &std::path::Path,
    handle: &RunHandle,
    events: &dyn EventSink,
    config: &UploadConfig,
) {
    let run_id = handle.state().run_id;
    let log = UploadLog::open(dir).ok();
    let mut cancelled_at: Option<Instant> = None;
    loop {
        if handle.wait(Some(config.heartbeat_interval)).is_finished() {
            return;
        }
        if let Some(log) = &log {
            let _ = log.heartbeat(run_id, now_secs());
        }
        match cancelled_at {
            None => {
                let paused = handle.lock().status == RunStatus::Paused;
                if !paused && handle.idle_for() >= config.stall_timeout {
                    handle.stalled.store(true, Ordering::SeqCst);
                    if let Some(log) = &log {
                        let _ = log.event(
                            run_id,
                            "error",
                            &format!(
                                "watchdog: no progress for {} s – stopping the run",
                                config.stall_timeout.as_secs()
                            ),
                            now_secs(),
                        );
                    }
                    handle.cancel();
                    cancelled_at = Some(Instant::now());
                }
            }
            Some(at) if at.elapsed() >= config.stall_grace => {
                end_without_executor(dir, handle, events, &stall_message(config.stall_timeout));
                return;
            }
            Some(_) => {}
        }
    }
}

fn stall_message(timeout: Duration) -> String {
    format!(
        "the upload stopped: no progress for {} s (network or store not responding) – plan again to resume",
        timeout.as_secs()
    )
}

/// Ends the run when the executor cannot (panic, hang): state, log, event.
fn end_without_executor(
    dir: &std::path::Path,
    handle: &RunHandle,
    events: &dyn EventSink,
    message: &str,
) {
    if handle.finalized.swap(true, Ordering::SeqCst) {
        return;
    }
    let report = {
        let mut s = handle.lock();
        for x in &mut s.sets {
            if matches!(
                x.status,
                SetRunStatus::Pending | SetRunStatus::Running | SetRunStatus::Paused
            ) {
                x.status = SetRunStatus::Failed;
                x.error.get_or_insert_with(|| message.to_owned());
            }
        }
        s.status = RunStatus::Failed;
        s.paused_until = None;
        s.message = Some(message.to_owned());
        s.finished_at = Some(now_secs());
        s.clone()
    };
    if let Ok(log) = UploadLog::open(dir) {
        let _ = log.event(report.run_id, "error", message, now_secs());
        let _ = log.finish_open_sets(report.run_id, "failed", message, now_secs());
        let _ = log.finish_run(
            report.run_id,
            "failed",
            now_secs(),
            &serde_json::to_string(&report).unwrap_or_default(),
        );
    }
    handle.finished.notify_all();
    events.emit(
        UPLOAD_FINISHED,
        serde_json::to_value(&report).unwrap_or_default(),
    );
}

/// Runs the job to the end (blocking; called on the run's thread).
pub(crate) fn run(job: &Job) -> RunReport {
    let run_id = job.handle.state().run_id;
    let outcome = (|| -> Result<(), String> {
        let log = UploadLog::open(&job.dir).map_err(|e| e.message)?;
        let runner = Runner {
            job,
            log: Mutex::new(log),
            run_id,
            http: Arc::new(Mutex::new(Vec::new())),
        };
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| format!("cannot start the network runtime: {e}"))?;
        rt.block_on(async {
            for sp in &job.plan.stores {
                match (sp.store, &job.apple, &job.google) {
                    (StoreKind::Apple, Some(a), _) => runner.apple(sp, a).await,
                    (StoreKind::Google, _, Some(g)) => runner.google(sp, g).await,
                    _ => {}
                }
            }
        });
        runner.flush_http();
        Ok(())
    })();
    if job.handle.finalized.swap(true, Ordering::SeqCst) {
        // The watchdog already ended the run.
        return job.handle.state();
    }
    let stalled = job.handle.stalled.load(Ordering::SeqCst);
    let report = {
        let mut s = job.handle.lock();
        let uploading: Vec<&SetRun> = s
            .sets
            .iter()
            .filter(|x| x.status != SetRunStatus::Skipped)
            .collect();
        let total = uploading.len();
        let done = uploading
            .iter()
            .filter(|x| {
                matches!(
                    x.status,
                    SetRunStatus::Done | SetRunStatus::UploadedProcessing
                )
            })
            .count();
        let cancelled = job.handle.cancel.is_cancelled()
            || uploading
                .iter()
                .any(|x| x.status == SetRunStatus::Cancelled);
        let store_failed = !s.store_failures.is_empty();
        if let Err(e) = &outcome {
            s.message = Some(e.clone());
        } else if stalled {
            s.message = Some(stall_message(job.config.stall_timeout));
        }
        s.status = if outcome.is_err() || stalled {
            RunStatus::Failed
        } else if cancelled {
            RunStatus::Cancelled
        } else if done == total && total > 0 && !store_failed {
            RunStatus::Completed
        } else if done > 0 {
            RunStatus::Partial
        } else {
            RunStatus::Failed
        };
        // Sets never reached (e.g. the store was not configured).
        for x in &mut s.sets {
            if matches!(x.status, SetRunStatus::Pending | SetRunStatus::Running) {
                x.status = SetRunStatus::Failed;
                x.error.get_or_insert_with(|| "not uploaded".into());
            }
        }
        s.paused_until = None;
        s.finished_at = Some(now_secs());
        if s.message.is_none() {
            s.message = Some(summary(&s));
        }
        s.clone()
    };
    if let Ok(log) = UploadLog::open(&job.dir) {
        let status = serde_json::to_value(report.status)
            .ok()
            .and_then(|v| v.as_str().map(str::to_owned))
            .unwrap_or_default();
        let _ = log.finish_run(
            run_id,
            &status,
            now_secs(),
            &serde_json::to_string(&report).unwrap_or_default(),
        );
    }
    job.handle.finished.notify_all();
    job.events.emit(
        UPLOAD_FINISHED,
        serde_json::to_value(&report).unwrap_or_default(),
    );
    report
}

fn summary(s: &RunState) -> String {
    use std::fmt::Write as _;
    let count = |st: SetRunStatus| s.sets.iter().filter(|x| x.status == st).count();
    let mut out = String::new();
    for f in &s.store_failures {
        out.push_str(&f.message);
        out.push(' ');
    }
    let _ = write!(
        out,
        "{} set(s) uploaded, {} failed, {} cancelled, {} skipped; {} image(s)",
        count(SetRunStatus::Done) + count(SetRunStatus::UploadedProcessing),
        count(SetRunStatus::Failed),
        count(SetRunStatus::Cancelled),
        count(SetRunStatus::Skipped),
        s.images_done
    );
    if s.images_processing > 0 {
        let _ = write!(
            out,
            ". Apple is still processing {} image(s) – check the store state later (\"Check store state\" / plan --remote); they are not uploaded again",
            s.images_processing
        );
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn poll_urls_are_recognised() {
        assert!(is_screenshot_poll(
            "https://api.appstoreconnect.apple.com/v1/appScreenshots/9d80-0a47"
        ));
        assert!(!is_screenshot_poll(
            "https://api.appstoreconnect.apple.com/v1/appScreenshots"
        ));
        assert!(!is_screenshot_poll(
            "https://api.appstoreconnect.apple.com/v1/appScreenshotSets/x/appScreenshots"
        ));
        assert_eq!(mmss(Duration::from_secs(151)), "2:31");
    }
}
