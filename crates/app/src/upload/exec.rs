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
//! - **Log**: runs, sets, files, redacted HTTP events → `uploads.sqlite`.

use std::path::PathBuf;
use std::sync::{Arc, Condvar, Mutex, PoisonError};
use std::time::Duration;

use arrangia_core::model::StoreKind;
use arrangia_publish::apple::{AppRef, Platform};
use arrangia_publish::credentials::Secret;
use arrangia_publish::{
    CancellationToken, ClientOptions, Error, HttpEvent, HttpEventKind, Progress, UploadStep, google,
};
use serde::Serialize;
use serde_json::json;

use super::log::UploadLog;
use super::remote::{self, AppleUploadFlow as _, SetHooks, UploadFile};
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

/// Status of one set in a run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SetRunStatus {
    Pending,
    Running,
    Paused,
    Done,
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
    pub files_done: usize,
    pub files_total: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
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
    pub bytes_done: u64,
    pub bytes_total: u64,
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
                error: None,
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
                bytes_done: 0,
                bytes_total: plan.totals.bytes,
            }),
            finished: Condvar::new(),
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
        let deadline = timeout.map(|t| std::time::Instant::now() + t);
        while !g.is_finished() {
            match deadline {
                None => {
                    g = self
                        .finished
                        .wait(g)
                        .unwrap_or_else(PoisonError::into_inner);
                }
                Some(d) => {
                    let now = std::time::Instant::now();
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
}

pub(crate) struct GoogleJob {
    pub secret: Secret,
    pub package: String,
}

/// Redacted HTTP events, buffered until the log writer flushes them.
type EventBuffer = Arc<Mutex<Vec<(String, String)>>>;

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
        for (level, msg) in events {
            self.note(&level, &msg);
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
                buf.lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .push((level.to_owned(), msg));
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
            SetRunStatus::Cancelled => "cancelled",
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

    async fn apple(&self, sp: &StorePlan, job: &AppleJob) {
        let sets: Vec<&PlanSet> = sp.sets.iter().filter(|s| s.action.uploads()).collect();
        if sets.is_empty() {
            return;
        }
        let flow = remote::apple_flow();
        let fail_all = |msg: &str| {
            for s in &sets {
                let row = self.start_set(sp, s);
                self.finish_set(row, &s.key, SetRunStatus::Failed, Some(msg.to_owned()));
            }
        };
        let client =
            match remote::apple_client(&self.job.config, &job.account, &job.secret, self.options())
            {
                Ok(c) => c,
                Err(e) => return fail_all(&remote::store_message(StoreKind::Apple, &e)),
            };
        self.emit("prepare", None, Some(json!({ "store": "apple" })));
        let target = match flow.open(&client, &job.app, job.platform).await {
            Ok(t) => t,
            Err(Error::Cancelled) => return self.cancel_rest(sp),
            Err(e) => return fail_all(&remote::store_message(StoreKind::Apple, &e)),
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
            let snap = self.set_status(&set.key, SetRunStatus::Running, None);
            self.emit("set_started", snap.as_ref(), None);
            let locale = set.locale.clone().unwrap_or_default();
            let mut attempt = 0;
            let result = loop {
                let hooks = Hooks {
                    runner: self,
                    set,
                    row,
                };
                match flow
                    .replace(&client, &target, &locale, &set.slot, &files, &hooks)
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
                Ok(_) => self.finish_set(row, &set.key, SetRunStatus::Done, None),
                Err(Error::Cancelled) => {
                    self.finish_set(
                        row,
                        &set.key,
                        SetRunStatus::Cancelled,
                        Some(
                            "cancelled – the set may be incomplete; the next run replaces it"
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
        self.job.handle.set_mut(key, |s| s.files_done = 0);
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
                let hooks = Hooks {
                    runner: self,
                    set,
                    row: *row,
                };
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

/// [`SetHooks`] of one set: state, events, file log.
struct Hooks<'a> {
    runner: &'a Runner<'a>,
    set: &'a PlanSet,
    row: i64,
}

impl SetHooks for Hooks<'_> {
    fn file_started(&self, index: usize) {
        let img = &self.set.images[index];
        let snap = self.runner.job.handle.set_mut(&self.set.key, |s| s.clone());
        self.runner.emit(
            "file_started",
            snap.as_ref(),
            Some(json!({"index": img.index, "file_name": img.file_name, "bytes_sent": 0, "bytes_total": img.bytes})),
        );
    }

    fn progress(&self, index: usize, p: &Progress) {
        if p.step != UploadStep::Transfer {
            return;
        }
        let img = &self.set.images[index];
        let snap = self.runner.job.handle.set_mut(&self.set.key, |s| s.clone());
        self.runner.emit(
            "transfer",
            snap.as_ref(),
            Some(json!({"index": img.index, "file_name": img.file_name,
                        "bytes_sent": p.bytes_sent, "bytes_total": p.bytes_total})),
        );
    }

    fn file_done(&self, index: usize, store_id: &str) {
        let img = &self.set.images[index];
        let _ = self.runner.log().file(
            self.row,
            img.index,
            &img.file_name,
            &img.sha256,
            img.bytes,
            Some(store_id),
            "done",
            None,
        );
        {
            let mut s = self.runner.job.handle.lock();
            s.images_done += 1;
            s.bytes_done += img.bytes;
        }
        let snap = self.runner.job.handle.set_mut(&self.set.key, |s| {
            s.files_done += 1;
            s.clone()
        });
        self.runner.emit(
            "file_done",
            snap.as_ref(),
            Some(json!({"index": img.index, "file_name": img.file_name,
                        "bytes_sent": img.bytes, "bytes_total": img.bytes})),
        );
    }

    fn step(&self, step: &str) {
        let snap = self.runner.job.handle.set_mut(&self.set.key, |s| s.clone());
        self.runner.emit(step, snap.as_ref(), None);
    }
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
            .filter(|x| x.status == SetRunStatus::Done)
            .count();
        let cancelled = job.handle.cancel.is_cancelled()
            || uploading
                .iter()
                .any(|x| x.status == SetRunStatus::Cancelled);
        if let Err(e) = &outcome {
            s.message = Some(e.clone());
        }
        s.status = if outcome.is_err() {
            RunStatus::Failed
        } else if cancelled {
            RunStatus::Cancelled
        } else if done == total {
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
    let count = |st: SetRunStatus| s.sets.iter().filter(|x| x.status == st).count();
    format!(
        "{} set(s) uploaded, {} failed, {} cancelled, {} skipped; {} image(s)",
        count(SetRunStatus::Done),
        count(SetRunStatus::Failed),
        count(SetRunStatus::Cancelled),
        count(SetRunStatus::Skipped),
        s.images_done
    )
}
