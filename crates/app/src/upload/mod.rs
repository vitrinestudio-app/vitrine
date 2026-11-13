//! Store upload: **plan → confirm → execute** (Phase 5.2/5.5, ADR 0018).
//!
//! ```text
//! plan     build the dry-run plan: render (incremental, `.cache/upload/images`),
//!          hash, compare with the upload log (and the store with `remote`),
//!          store-rule checks, preconditions, totals, confirm token
//! log      `<project>/.cache/uploads.sqlite`: plans (tokens), runs, sets, files, events
//! remote   store clients from the account + keychain secret, shared Apple
//!          flow types and the Google edit helpers
//! library  the App Asset Library flow (the only Apple flow, ADR 0019)
//! exec     the executor: Apple per set, Google one edit; progress, cancel,
//!          resume (completed sets are `unchanged` in the next plan), rate limits
//! readiness  setup checklist (accounts → project mapping → target slots)
//!          with the place to fix each step; shown before the plan
//! policy   who may execute: the user directly; MCP only with a GUI
//!          confirmation click (GUI-hosted) or an opt-in setting (headless)
//! ```
//!
//! The plan is the only input of the executor: `upload_execute` takes the
//! confirm token, loads the stored plan from the log, re-computes the local
//! fingerprint (project revision, store mapping, selection, image hashes)
//! and refuses when anything changed or the token expired.

pub mod exec;
pub mod library;
pub mod log;
pub mod plan;
pub mod policy;
pub mod readiness;
pub mod remote;

use std::sync::{Arc, Mutex, PoisonError, RwLock};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use vitrine_core::model::StoreKind;
use vitrine_publish::{ClientOptions, PollPolicy, RetryPolicy};

pub use exec::{RunHandle, RunReport, RunState, SetRun, SetRunStatus, StoreFailure};
pub use plan::PlanArgs;

/// Events of the upload.
pub const UPLOAD_PROGRESS: &str = "upload-progress";
pub const UPLOAD_FINISHED: &str = "upload-finished";
pub const UPLOAD_CONFIRM_REQUESTED: &str = "upload-confirm-requested";
pub const UPLOAD_CONFIRM_RESOLVED: &str = "upload-confirm-resolved";

/// Version of the plan JSON.
pub const PLAN_VERSION: u32 = 1;

// ----- plan ---------------------------------------------------------------------------

/// The dry-run plan (serde; shown by CLI, GUI and MCP).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadPlan {
    pub version: u32,
    /// Pass this to `upload_execute` (valid until `expires_at`, only for
    /// the project state the plan was made from).
    pub confirm_token: String,
    /// Unix seconds.
    pub created_at: u64,
    pub expires_at: u64,
    /// Project folder.
    pub project: String,
    pub revision: u64,
    pub selection: Selection,
    pub stores: Vec<StorePlan>,
    /// Problems not tied to a store (e.g. the project is not mapped).
    pub problems: Vec<Problem>,
    /// Setup checklist (what to do where, in order; see [`readiness`]).
    #[serde(default)]
    pub readiness: readiness::Readiness,
    pub totals: Totals,
    /// A blocking problem exists: the plan cannot be executed.
    pub blocked: bool,
}

impl UploadPlan {
    /// All sets of all stores.
    pub fn sets(&self) -> impl Iterator<Item = (&StorePlan, &PlanSet)> {
        self.stores
            .iter()
            .flat_map(|s| s.sets.iter().map(move |set| (s, set)))
    }

    /// One line per store for confirmations.
    pub fn summary(&self) -> String {
        let t = &self.totals;
        format!(
            "{} set(s) to upload ({} image(s), {}), {} unchanged, {} skipped{}",
            t.upload_sets,
            t.images,
            human_bytes(t.bytes),
            t.unchanged_sets,
            t.skipped_sets,
            if self.blocked { ", BLOCKED" } else { "" }
        )
    }
}

/// What the plan was made for.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Selection {
    /// Stores (empty = every mapped store).
    pub stores: Vec<StoreKind>,
    /// Targets (empty = every target with a store slot).
    pub targets: Vec<String>,
    /// Languages (empty = each target's languages).
    pub langs: Vec<String>,
    /// The store state was read (`--remote`).
    pub remote: bool,
    /// Upload even sets that look unchanged.
    pub force: bool,
}

/// Plan of one store.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorePlan {
    pub store: StoreKind,
    /// Account id (`settings.yaml`), if referenced.
    pub account: Option<String>,
    /// App identity: Apple app id / bundle id, Google package name.
    pub app: Option<String>,
    /// How the store is written: Apple `asset_library`, Google `edit`.
    /// (Plans logged before 2026-11-13 may say `screenshot_sets` and carry
    /// `flow_setting`/`flow_note`; those fields are ignored when read.)
    pub flow: String,
    /// What the remote check found (`None` = not checked).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remote: Option<RemoteInfo>,
    pub sets: Vec<PlanSet>,
    pub problems: Vec<Problem>,
}

/// Facts from the store (remote check).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RemoteInfo {
    /// The check ran without errors.
    pub ok: bool,
    /// App name as the store reports it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_name: Option<String>,
    /// Apple: editable version (`1.2.0 PREPARE_FOR_SUBMISSION`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// What happens to one store set (target × language).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SetAction {
    /// Nothing known in the store: upload the images.
    New,
    /// Replace the images in the store.
    Replace,
    /// Same images as the last upload / the store: skipped.
    Unchanged,
    /// Not uploaded (no store locale, other platform …): see `reason`.
    Skip,
    /// A blocking problem: the plan cannot run.
    Blocked,
}

impl SetAction {
    pub fn uploads(self) -> bool {
        matches!(self, Self::New | Self::Replace)
    }
}

/// One store set: a target's images for one language.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanSet {
    /// `<store>:<locale>:<slot>` – what the log and resume key on.
    pub key: String,
    pub target: String,
    pub lang: String,
    pub locale: Option<String>,
    /// Display type / image type (Apple creative assets: the placement
    /// type, e.g. `PRODUCT_PAGE_HEADER_ASSET`).
    pub slot: String,
    /// Apple asset library: placement type → group (after the remote
    /// check; locally the display class), e.g. `APP_SCREENSHOT →
    /// IPHONE_DYNAMIC_ISLAND_LARGE_PROFILE`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub placement: Option<String>,
    pub action: SetAction,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// Generic steps (`create_localization`, `create_set`,
    /// `delete_existing`, `upload`, `set_order`).
    pub steps: Vec<String>,
    /// Images in the store now (remote check only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remote_count: Option<usize>,
    /// Of the planned images, Apple is still processing this many
    /// (remote check, or the last run ended while Apple was processing).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub processing: Option<usize>,
    /// Planned images the store already holds; the run keeps them instead
    /// of uploading them again (remote check, Apple).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reusable: Option<usize>,
    pub images: Vec<PlanImage>,
    pub bytes: u64,
    pub problems: Vec<Problem>,
}

/// One image in store order.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanImage {
    /// 1-based position in the set.
    pub index: u32,
    /// Path below `.cache/upload/images` (`/`-separated).
    pub file: String,
    /// File name sent to the store.
    pub file_name: String,
    pub width: u32,
    pub height: u32,
    pub bytes: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProblemLevel {
    Info,
    Warn,
    Error,
}

/// A plan problem. `blocking` problems prevent execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Problem {
    pub level: ProblemLevel,
    pub code: String,
    pub message: String,
    pub blocking: bool,
    /// Values for a localized message (`upload.problem.<code>` in the GUI).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vars: Option<serde_json::Value>,
    /// A page that fixes it (opened with `open_url`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link: Option<String>,
}

impl Problem {
    pub fn blocking(code: &str, message: impl Into<String>) -> Self {
        Self {
            level: ProblemLevel::Error,
            code: code.to_owned(),
            message: message.into(),
            blocking: true,
            vars: None,
            link: None,
        }
    }

    pub fn warn(code: &str, message: impl Into<String>) -> Self {
        Self {
            level: ProblemLevel::Warn,
            code: code.to_owned(),
            message: message.into(),
            blocking: false,
            vars: None,
            link: None,
        }
    }

    /// Adds values for the localized message.
    #[must_use]
    pub fn with_vars(mut self, vars: serde_json::Value) -> Self {
        self.vars = Some(vars);
        self
    }

    /// Adds the page that fixes it.
    #[must_use]
    pub fn with_link(mut self, link: impl Into<String>) -> Self {
        self.link = Some(link.into());
        self
    }

    pub fn info(code: &str, message: impl Into<String>) -> Self {
        Self {
            level: ProblemLevel::Info,
            code: code.to_owned(),
            message: message.into(),
            blocking: false,
            vars: None,
            link: None,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Totals {
    pub sets: usize,
    pub upload_sets: usize,
    pub unchanged_sets: usize,
    pub skipped_sets: usize,
    pub blocked_sets: usize,
    /// Images to upload.
    pub images: usize,
    /// Bytes to upload.
    pub bytes: u64,
}

pub(crate) fn human_bytes(b: u64) -> String {
    if b >= 1024 * 1024 {
        format!("{:.1} MB", b as f64 / (1024.0 * 1024.0))
    } else {
        format!("{} KB", b.div_ceil(1024))
    }
}

// ----- configuration and state --------------------------------------------------------

/// Env overrides of the store endpoints (dev bridge, CLI tests; loopback only).
pub use crate::store_test::{ENV_ASC_BASE_URL, ENV_PLAY_BASE_URL, ENV_PLAY_TOKEN_URL};

/// Endpoints, client options and timings of uploads (tests shorten them).
#[derive(Debug, Clone)]
pub struct UploadConfig {
    /// App Store Connect base URL (`None` = production).
    pub apple_base_url: Option<String>,
    /// Android Publisher base URL (`None` = production).
    pub google_base_url: Option<String>,
    /// Google OAuth token URL (`None` = production).
    pub google_token_url: Option<String>,
    /// Retries, polling, timeouts of the store clients.
    pub options: ClientOptions,
    /// How long a confirm token stays valid.
    pub token_ttl: Duration,
    /// Longest pause on a rate limit before the set fails.
    pub max_rate_limit_pause: Duration,
    /// Pause used when the store gives no `Retry-After`.
    pub default_rate_limit_pause: Duration,
    /// Pauses per set before it fails.
    pub rate_limit_retries: u32,
    /// How long an MCP upload waits for the GUI confirmation.
    pub confirm_timeout: Duration,
    /// Apple: how long a set waits for Apple's processing of its images
    /// (all polled together). Images still processing afterwards are not
    /// failures: the set ends `uploaded_processing`.
    pub processing_budget: Duration,
    /// Watchdog: a running (not paused) run without any progress for this
    /// long is cancelled and ends `failed`.
    pub stall_timeout: Duration,
    /// Watchdog: after cancelling a stalled run, how long to wait for the
    /// executor before the run is ended without it.
    pub stall_grace: Duration,
    /// Watchdog tick: heartbeat in the log, stall check.
    pub heartbeat_interval: Duration,
}

impl Default for UploadConfig {
    fn default() -> Self {
        Self {
            apple_base_url: None,
            google_base_url: None,
            google_token_url: None,
            options: ClientOptions {
                retry: RetryPolicy::default(),
                poll: PollPolicy::default(),
                ..ClientOptions::default()
            },
            token_ttl: Duration::from_mins(30),
            max_rate_limit_pause: Duration::from_mins(15),
            default_rate_limit_pause: Duration::from_mins(1),
            rate_limit_retries: 2,
            confirm_timeout: Duration::from_mins(5),
            processing_budget: Duration::from_mins(15),
            stall_timeout: Duration::from_mins(10),
            stall_grace: Duration::from_secs(30),
            heartbeat_interval: Duration::from_secs(30),
        }
    }
}

impl UploadConfig {
    /// Production endpoints, overridable through [`ENV_ASC_BASE_URL`],
    /// [`ENV_PLAY_BASE_URL`], [`ENV_PLAY_TOKEN_URL`] (loopback URLs only,
    /// like the connection test).
    pub fn from_env() -> Self {
        let var = |name: &str| {
            std::env::var(name)
                .ok()
                .filter(|u| crate::store_test::is_loopback_url(u))
        };
        Self {
            apple_base_url: var(ENV_ASC_BASE_URL),
            google_base_url: var(ENV_PLAY_BASE_URL),
            google_token_url: var(ENV_PLAY_TOKEN_URL),
            ..Self::default()
        }
    }
}

/// Upload state of the app (part of [`crate::AppState`]).
#[derive(Debug, Default)]
pub struct UploadState {
    config: RwLock<Option<UploadConfig>>,
    run: Mutex<Option<Arc<RunHandle>>>,
    pub(crate) confirm: policy::ConfirmBroker,
}

impl UploadState {
    /// The configuration (default: [`UploadConfig::from_env`]).
    pub fn config(&self) -> UploadConfig {
        if let Some(c) = &*self.config.read().unwrap_or_else(PoisonError::into_inner) {
            return c.clone();
        }
        let c = UploadConfig::from_env();
        *self.config.write().unwrap_or_else(PoisonError::into_inner) = Some(c.clone());
        c
    }

    /// Replaces the configuration (tests, dev bridge).
    pub fn set_config(&self, config: UploadConfig) {
        *self.config.write().unwrap_or_else(PoisonError::into_inner) = Some(config);
    }

    /// The running or last run.
    pub fn run(&self) -> Option<Arc<RunHandle>> {
        self.run
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// Registers a new run unless one is running.
    pub(crate) fn begin(&self, handle: Arc<RunHandle>) -> bool {
        let mut slot = self.run.lock().unwrap_or_else(PoisonError::into_inner);
        if slot.as_ref().is_some_and(|r| r.is_running()) {
            return false;
        }
        *slot = Some(handle);
        true
    }

    /// The broker for MCP confirmation requests.
    pub fn confirmations(&self) -> &policy::ConfirmBroker {
        &self.confirm
    }
}

/// Lowercase hex.
pub(crate) fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(s, "{b:02x}");
    }
    s
}

/// SHA-256 as lowercase hex.
pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    hex(&Sha256::digest(bytes))
}

/// Unix seconds now.
pub(crate) fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}
