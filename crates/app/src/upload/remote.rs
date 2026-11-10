//! Store access for plan (remote check) and executor: clients built from
//! the project's account reference + the keychain secret, the Apple upload
//! flow abstraction and the Google edit helpers.
//!
//! **Apple flows.** [`AppleUploadFlow`] hides how a set of screenshots gets
//! into App Store Connect. Today there is one implementation,
//! [`ScreenshotSetFlow`] (`appScreenshotSets` / `appScreenshots`, deprecated
//! in ASC API 4.5.1 but still documented; ADR 0017). The App Asset Library
//! flow (assets + placements + ordering requests) can be added as a second
//! implementation of the same trait plus a variant of [`AppleFlowKind`];
//! plan, executor and UI only see the flow's name (`StorePlan.flow`).

use std::sync::Mutex;

use arrangia_core::model::{ApplePlatform, Project, StoreKind};
use arrangia_publish::apple::{self, AppRef, AppStoreVersion, DeliveryState, Platform};
use arrangia_publish::credentials::Secret;
use arrangia_publish::{ClientOptions, Error, ProgressFn, Result, google};
use serde::{Deserialize, Serialize};

use super::{PlanImage, UploadConfig};
use crate::commands::AppState;
use crate::error::{CmdResult, CommandError};
use crate::stores::StoreAccount;

/// The account of `store` referenced by the project, with its secret.
pub(crate) fn credentials(
    app: &AppState,
    project: &Project,
    store: StoreKind,
) -> CmdResult<(StoreAccount, Secret)> {
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

// ----- Apple flows --------------------------------------------------------------------

/// The Apple upload flows (only the name reaches the plan).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppleFlowKind {
    /// `appScreenshotSets` + `appScreenshots` (ASC API ≤ 4.5.0 resources).
    #[default]
    ScreenshotSets,
}

impl AppleFlowKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ScreenshotSets => "screenshot_sets",
        }
    }
}

/// The app + editable version a run writes to.
#[derive(Debug)]
pub(crate) struct AppleTarget {
    pub app: apple::App,
    pub version: AppStoreVersion,
    /// Version localizations (filled lazily, created ones added).
    pub localizations: Mutex<Option<Vec<apple::Localization>>>,
}

/// What the store holds for one set (remote check).
#[derive(Debug, Clone, Default)]
pub(crate) struct RemoteSet {
    /// Generic steps the executor will take (`create_localization`, …).
    pub steps: Vec<String>,
    /// Images in the store now.
    pub count: usize,
    /// The store holds exactly the planned images in this order.
    pub same: bool,
}

/// One image to upload, with its bytes.
#[derive(Debug, Clone)]
pub(crate) struct UploadFile {
    pub image: PlanImage,
    pub bytes: Vec<u8>,
}

/// Callbacks of one set upload.
pub(crate) trait SetHooks: Sync {
    /// A file starts (0-based position).
    fn file_started(&self, index: usize);
    /// Progress of the current file.
    fn progress(&self, index: usize, p: &arrangia_publish::Progress);
    /// A file is in the store.
    fn file_done(&self, index: usize, store_id: &str);
    /// A step of the flow (`delete_existing`, `set_order`, …).
    fn step(&self, step: &str);
}

/// How screenshots get into App Store Connect. Executor and plan only call
/// these three methods; a new flow is a new implementation.
pub(crate) trait AppleUploadFlow {
    fn kind(&self) -> AppleFlowKind;

    /// Finds the app and its editable version (precondition of every run).
    async fn open(
        &self,
        client: &apple::Client,
        app: &AppRef,
        platform: Platform,
    ) -> Result<AppleTarget>;

    /// Reads the store state of one set (plan, `remote`).
    async fn inspect(
        &self,
        client: &apple::Client,
        target: &AppleTarget,
        locale: &str,
        display_type: &str,
        images: &[PlanImage],
    ) -> Result<RemoteSet>;

    /// Replaces the set with `files` in this order and verifies the result;
    /// returns the store ids in order.
    async fn replace(
        &self,
        client: &apple::Client,
        target: &AppleTarget,
        locale: &str,
        display_type: &str,
        files: &[UploadFile],
        hooks: &dyn SetHooks,
    ) -> Result<Vec<String>>;
}

/// The set-based flow (ADR 0017): localization → set → delete old → upload
/// (reserve/parts/commit/poll) → order → verify.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct ScreenshotSetFlow;

impl ScreenshotSetFlow {
    async fn localization_id(
        client: &apple::Client,
        target: &AppleTarget,
        locale: &str,
        create: bool,
    ) -> Result<Option<String>> {
        let known = target
            .localizations
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        let list = if let Some(l) = known {
            l
        } else {
            let l = client.localizations(&target.version.id).await?;
            *target
                .localizations
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(l.clone());
            l
        };
        if let Some(l) = list.iter().find(|l| l.locale.eq_ignore_ascii_case(locale)) {
            return Ok(Some(l.id.clone()));
        }
        if !create {
            return Ok(None);
        }
        let created = client
            .create_localization(&target.version.id, locale)
            .await?;
        let id = created.id.clone();
        target
            .localizations
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get_or_insert_with(Vec::new)
            .push(created);
        Ok(Some(id))
    }
}

impl AppleUploadFlow for ScreenshotSetFlow {
    fn kind(&self) -> AppleFlowKind {
        AppleFlowKind::ScreenshotSets
    }

    async fn open(
        &self,
        client: &apple::Client,
        app: &AppRef,
        platform: Platform,
    ) -> Result<AppleTarget> {
        let found = client.find_app(app).await?;
        let version = client.editable_version(&found.id, platform).await?;
        Ok(AppleTarget {
            app: found,
            version,
            localizations: Mutex::new(None),
        })
    }

    async fn inspect(
        &self,
        client: &apple::Client,
        target: &AppleTarget,
        locale: &str,
        display_type: &str,
        images: &[PlanImage],
    ) -> Result<RemoteSet> {
        let mut steps = Vec::new();
        let Some(loc) = Self::localization_id(client, target, locale, false).await? else {
            steps.extend(["create_localization", "create_set", "upload", "set_order"]);
            return Ok(RemoteSet {
                steps: steps.into_iter().map(str::to_owned).collect(),
                count: 0,
                same: false,
            });
        };
        let set = client
            .screenshot_sets(&loc)
            .await?
            .into_iter()
            .find(|s| s.display_type == display_type);
        let Some(set) = set else {
            steps.extend(["create_set", "upload", "set_order"]);
            return Ok(RemoteSet {
                steps: steps.into_iter().map(str::to_owned).collect(),
                count: 0,
                same: false,
            });
        };
        let shots = client.list_screenshots(&set.id).await?;
        let same = shots.len() == images.len()
            && shots.iter().zip(images).all(|(s, i)| {
                s.source_file_checksum
                    .as_deref()
                    .is_some_and(|c| c.eq_ignore_ascii_case(&i.md5))
                    && s.delivery_state == Some(DeliveryState::Complete)
            });
        if !shots.is_empty() {
            steps.push("delete_existing");
        }
        steps.extend(["upload", "set_order"]);
        Ok(RemoteSet {
            steps: steps.into_iter().map(str::to_owned).collect(),
            count: shots.len(),
            same,
        })
    }

    async fn replace(
        &self,
        client: &apple::Client,
        target: &AppleTarget,
        locale: &str,
        display_type: &str,
        files: &[UploadFile],
        hooks: &dyn SetHooks,
    ) -> Result<Vec<String>> {
        let loc = Self::localization_id(client, target, locale, true)
            .await?
            .unwrap_or_default();
        let set = client.ensure_set(&loc, display_type).await?;
        // Replace the whole set: also removes leftovers of an aborted run.
        let old = client.list_screenshots(&set.id).await?;
        if !old.is_empty() {
            hooks.step("delete_existing");
        }
        for s in &old {
            client.delete_screenshot(&s.id).await?;
        }
        let mut ids = Vec::with_capacity(files.len());
        for (i, f) in files.iter().enumerate() {
            hooks.file_started(i);
            let progress = |p: &arrangia_publish::Progress| hooks.progress(i, p);
            let progress: ProgressFn<'_> = &progress;
            let shot = client
                .upload_screenshot(&set.id, &f.image.file_name, f.bytes.clone(), progress)
                .await?;
            hooks.file_done(i, &shot.id);
            ids.push(shot.id);
        }
        hooks.step("set_order");
        client.reorder(&set.id, &ids).await?;
        hooks.step("verify");
        let now: Vec<String> = client
            .list_screenshots(&set.id)
            .await?
            .into_iter()
            .map(|s| s.id)
            .collect();
        if now != ids {
            return Err(Error::Conflict {
                message: format!(
                    "the set holds {} screenshot(s) in another order than uploaded ({} expected)",
                    now.len(),
                    ids.len()
                ),
                details: Vec::new(),
            });
        }
        Ok(ids)
    }
}

/// The flow in use (one today; the user's decision on the App Asset
/// Library adds a second, ADR 0017/0018).
pub(crate) fn apple_flow() -> ScreenshotSetFlow {
    ScreenshotSetFlow
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
        Error::Transport { status: None, .. } => {
            format!("cannot reach {name} – offline or blocked by a proxy/firewall?")
        }
        _ => format!("{name}: {e}"),
    }
}
