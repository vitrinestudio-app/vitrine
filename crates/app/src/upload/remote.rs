//! Store access for plan (remote check) and executor: clients built from
//! the project's account reference + the keychain secret, the types the
//! Apple flow shares with plan and executor, and the Google edit helpers.
//!
//! **Apple.** Screenshots and creative assets go through the App Asset
//! Library only ([`super::library::AssetLibraryFlow`]; ARCHITECTURE.md
//! §10). The deprecated screenshot-set API (`appScreenshotSets`) was
//! removed by decision 0035.

use std::sync::Mutex;
use std::time::Duration;

use vitrine_core::model::{ApplePlatform, Project, StoreKind};
use vitrine_publish::apple::{self, AppRef, AppStoreVersion, Platform};
use vitrine_publish::credentials::Secret;
use vitrine_publish::{ClientOptions, Error, PollPolicy, Result, google};

use super::{PlanImage, UploadConfig};
use crate::commands::AppState;
use crate::error::{CmdResult, CommandError};
use crate::stores::StoreAccount;

/// The account of `store` referenced by the project, with its secret;
/// environment credentials (CLI in CI) win and skip the keychain.
pub(crate) fn credentials(
    app: &AppState,
    project: &Project,
    store: StoreKind,
) -> CmdResult<(StoreAccount, Secret)> {
    if let Some(env) = app.env_account(store) {
        return Ok(env);
    }
    let id = project.store.account(store).ok_or_else(|| {
        CommandError::invalid(format!(
            "the project has no {store} account (Project settings → Stores)"
        ))
    })?;
    let account = app
        .settings_store()
        .load()?
        .store_accounts
        .into_iter()
        .find(|a| a.id == id && a.kind == store)
        .ok_or_else(|| {
            CommandError::not_found(format!(
                "store account `{id}` is not set up on this machine (App settings → Stores)"
            ))
        })?;
    let secret = app
        .secret_store()?
        .get(&account.id, account.kind)
        .map_err(crate::commands::keychain_error)?;
    Ok((account, secret))
}

pub(crate) fn apple_platform(p: ApplePlatform) -> Platform {
    match p {
        ApplePlatform::Ios => Platform::Ios,
        ApplePlatform::Macos => Platform::MacOs,
    }
}

/// How the project names the Apple app (app id wins over bundle id).
pub(crate) fn apple_app_ref(project: &Project) -> Option<AppRef> {
    let a = project.store.apple.as_ref()?;
    a.app_id
        .clone()
        .map(AppRef::Id)
        .or_else(|| a.bundle_id.clone().map(AppRef::BundleId))
}

pub(crate) fn apple_client(
    config: &UploadConfig,
    account: &StoreAccount,
    secret: &Secret,
    options: ClientOptions,
) -> Result<apple::Client> {
    let facts = account.apple.as_ref().ok_or_else(|| Error::Config {
        message: "the account has no issuer id / key id".into(),
    })?;
    let mut c = apple::Config::team(
        facts.issuer_id.clone(),
        facts.key_id.clone(),
        secret.expose(),
    );
    if let Some(url) = &config.apple_base_url {
        c = c.with_base_url(url.clone());
    }
    apple::Client::new(c, options)
}

pub(crate) fn google_client(
    config: &UploadConfig,
    secret: &Secret,
    package: &str,
    options: ClientOptions,
) -> Result<google::Client> {
    let mut c = google::Config::new(secret.expose(), package);
    if config.google_base_url.is_some() || config.google_token_url.is_some() {
        let base = config
            .google_base_url
            .clone()
            .unwrap_or_else(|| c.base_url.clone());
        let token = config
            .google_token_url
            .clone()
            .unwrap_or_else(|| c.token_url.clone());
        c = c.with_endpoints(base, token);
    }
    google::Client::new(c, options)
}

/// Runs `fut` on an own thread with a current-thread runtime (callers may
/// already be inside a runtime, where `block_on` would panic).
pub(crate) fn block_on<T: Send>(fut: impl Future<Output = T> + Send) -> CmdResult<T> {
    std::thread::scope(|scope| {
        scope
            .spawn(|| {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .map_err(|e| {
                        CommandError::io(format!("cannot start the network runtime: {e}"))
                    })?;
                Ok(rt.block_on(fut))
            })
            .join()
            .unwrap_or_else(|_| Err(CommandError::internal("the store task crashed")))
    })
}

// ----- Apple ---------------------------------------------------------------------------

/// The app + editable version a run writes to.
#[derive(Debug)]
pub(crate) struct AppleTarget {
    pub app: apple::App,
    pub version: AppStoreVersion,
    /// Version localizations (filled lazily, created ones added).
    pub localizations: Mutex<Option<Vec<apple::Localization>>>,
    /// The app's App Asset Library (library id, ref data).
    pub library: super::library::LibraryContext,
}

/// What the store holds for one set (remote check).
#[derive(Debug, Clone, Default)]
pub(crate) struct RemoteSet {
    /// Generic steps the executor will take (`create_localization`, …).
    pub steps: Vec<String>,
    /// Images in the store now.
    pub count: usize,
    /// The store holds exactly the planned images in this order (Apple:
    /// each processed or still processing).
    pub same: bool,
    /// Of those, Apple is still processing this many.
    pub processing: usize,
    /// Planned images the store already holds (kept by the next run).
    pub reusable: usize,
}

/// One image to upload, with its bytes.
#[derive(Debug, Clone)]
pub(crate) struct UploadFile {
    pub image: PlanImage,
    pub bytes: Vec<u8>,
}

/// Callbacks of one set upload (positions are 0-based).
pub(crate) trait SetHooks: Sync {
    /// A file starts (Apple: the reservation is requested).
    fn file_started(&self, index: usize);
    /// Progress of the current file (reserve/transfer/commit).
    fn progress(&self, index: usize, p: &vitrine_publish::Progress);
    /// Apple: the file is committed; Apple processes it now.
    fn file_uploaded(&self, _index: usize, _store_id: &str) {}
    /// Apple: the store already holds this file (same content marker), it is kept.
    fn file_kept(&self, _index: usize, _store_id: &str, _processing: bool) {}
    /// Apple: the state of a file changed (log transitions only).
    fn file_state(&self, _index: usize, _state: &str, _elapsed: Duration) {}
    /// Apple: one polling round; `pending` = (position, state) still processing.
    fn processing(&self, _pending: &[(usize, String)], _elapsed: Duration) {}
    /// A file is in the store (Apple: processed).
    fn file_done(&self, index: usize, store_id: &str);
    /// Apple: processing failed (`FAILED`, with Apple's errors).
    fn file_failed(&self, _index: usize, _store_id: &str, _message: &str) {}
    /// A step of the flow (`delete_placements`, `set_order`, …).
    fn step(&self, step: &str);
}

/// How [`AssetLibraryFlow::replace`](super::library::AssetLibraryFlow::replace)
/// treats processing and existing images.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ReplaceOptions {
    /// Reuse images the localization shows or the library holds with the
    /// same content marker (processed or still processing) instead of
    /// uploading them again.
    pub reuse: bool,
    /// How long to wait for Apple's processing of the whole set; images
    /// still processing afterwards are not failures.
    pub budget: Duration,
    /// Poll intervals (the timeout is not used; `budget` is).
    pub poll: PollPolicy,
}

/// What [`AssetLibraryFlow::replace`](super::library::AssetLibraryFlow::replace)
/// achieved.
#[derive(Debug, Clone, Default)]
pub(crate) struct ReplaceOutcome {
    /// Store ids in plan order.
    pub ids: Vec<String>,
    /// Images Apple was still processing when the budget ran out.
    pub processing: usize,
    /// Images kept from the store (not uploaded again).
    pub kept: usize,
    /// Something the user should know (e.g. the order could not be
    /// confirmed while Apple was processing).
    pub note: Option<String>,
}

/// User-facing message of a store error (no secrets in any of them).
pub(crate) fn store_message(store: StoreKind, e: &Error) -> String {
    let name = match store {
        StoreKind::Apple => "App Store Connect",
        StoreKind::Google => "Google Play",
    };
    match e {
        Error::Auth {
            status: Some(403), ..
        } => format!(
            "{name}: the key has no permission for this app (check its role / Play Console access)"
        ),
        Error::Auth { .. } => format!("{name} rejected the key ({e})"),
        Error::Conflict { message, .. } if apple::no_editable_version(e).is_some() => {
            format!("{name}: {message}")
        }
        Error::Transport { status: None, .. } => {
            format!("cannot reach {name} – offline or blocked by a proxy/firewall?")
        }
        _ => format!("{name}: {e}"),
    }
}
