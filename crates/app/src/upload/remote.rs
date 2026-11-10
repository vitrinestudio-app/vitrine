//! Store access for plan (remote check) and executor: clients built from
//! the project's account reference + the keychain secret, the Apple upload
//! flow abstraction and the Google edit helpers.
//!
//! **Apple flows.** [`AppleUploadFlow`] hides how a set of screenshots gets
//! into App Store Connect. Two implementations: [`ScreenshotSetFlow`]
//! (`appScreenshotSets` / `appScreenshots`, deprecated in ASC API 4.5.1;
//! ADR 0017) and [`super::library::AssetLibraryFlow`] (images, placements
//! and ordering requests; ADR 0019). [`open_apple`] picks the store's flow
//! from `store.apple.upload_flow` (`auto` = asset library, set API when the
//! library is unavailable) and [`route`] the flow of each set (`auto` also
//! falls back per set when the ref data has no group for it; Duo and
//! creative assets only go through the library).

use std::sync::Mutex;
use std::time::{Duration, Instant};

use arrangia_core::model::{ApplePlatform, AppleUploadFlowSetting, Project, StoreKind};
use arrangia_publish::apple::{self, AppRef, AppStoreVersion, DeliveryState, Platform};
use arrangia_publish::credentials::Secret;
use arrangia_publish::{ClientOptions, Error, PollPolicy, ProgressFn, Result, google};
use serde::{Deserialize, Serialize};

use super::library::AssetLibraryFlow;
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
    /// App Asset Library: images + placements + ordering requests (4.5.1).
    AssetLibrary,
}

impl AppleFlowKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ScreenshotSets => "screenshot_sets",
            Self::AssetLibrary => "asset_library",
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
    /// The App Asset Library (library id, ref data), when the run uses it.
    pub library: Option<super::library::LibraryContext>,
}

/// What the store holds for one set (remote check).
#[derive(Debug, Clone, Default)]
pub(crate) struct RemoteSet {
    /// Generic steps the executor will take (`create_localization`, …).
    pub steps: Vec<String>,
    /// Images in the store now.
    pub count: usize,
    /// The store holds exactly the planned images in this order (Apple:
    /// each `COMPLETE` or still processing).
    pub same: bool,
    /// Of those, Apple is still processing this many (`UPLOAD_COMPLETE`).
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
    fn progress(&self, index: usize, p: &arrangia_publish::Progress);
    /// Apple: the file is committed; Apple processes it now.
    fn file_uploaded(&self, _index: usize, _store_id: &str) {}
    /// Apple: the store already holds this file (same checksum), it is kept.
    fn file_kept(&self, _index: usize, _store_id: &str, _processing: bool) {}
    /// Apple: the delivery state of a file changed (log transitions only).
    fn file_state(&self, _index: usize, _state: &str, _elapsed: Duration) {}
    /// Apple: one polling round; `pending` = (position, state) still processing.
    fn processing(&self, _pending: &[(usize, String)], _elapsed: Duration) {}
    /// A file is in the store (Apple: `COMPLETE`).
    fn file_done(&self, index: usize, store_id: &str);
    /// Apple: processing failed (`FAILED`, with Apple's errors).
    fn file_failed(&self, _index: usize, _store_id: &str, _message: &str) {}
    /// A step of the flow (`delete_existing`, `set_order`, …).
    fn step(&self, step: &str);
}

/// How [`AppleUploadFlow::replace`] treats processing and existing images.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ReplaceOptions {
    /// Keep screenshots the store already holds with the same checksum
    /// (`COMPLETE` or still processing) instead of uploading them again.
    pub reuse: bool,
    /// How long to wait for Apple's processing of the whole set; images
    /// still processing afterwards are not failures.
    pub budget: Duration,
    /// Poll intervals (the timeout is not used; `budget` is).
    pub poll: PollPolicy,
}

/// What [`AppleUploadFlow::replace`] achieved.
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

/// How screenshots get into App Store Connect. Executor and plan only call
/// these three methods; a new flow is a new implementation.
pub(crate) trait AppleUploadFlow {
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

    /// Makes the set hold `files` in this order (keeping what the store
    /// already has with `reuse`), waits up to `opts.budget` for Apple's
    /// processing and verifies the order.
    #[allow(clippy::too_many_arguments)]
    async fn replace(
        &self,
        client: &apple::Client,
        target: &AppleTarget,
        locale: &str,
        display_type: &str,
        files: &[UploadFile],
        opts: ReplaceOptions,
        hooks: &dyn SetHooks,
    ) -> Result<ReplaceOutcome>;
}

/// The set-based flow (ADR 0017): localization → set → delete old → upload
/// (reserve/parts/commit/poll) → order → verify.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct ScreenshotSetFlow;

impl ScreenshotSetFlow {
    pub(crate) async fn localization_id(
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
            library: None,
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
                ..RemoteSet::default()
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
                ..RemoteSet::default()
            });
        };
        let shots = client.list_screenshots(&set.id).await?;
        let usable = |s: &apple::AppScreenshot| {
            matches!(
                s.delivery_state,
                Some(DeliveryState::Complete | DeliveryState::UploadComplete)
            )
        };
        let same = shots.len() == images.len()
            && shots.iter().zip(images).all(|(s, i)| {
                s.source_file_checksum
                    .as_deref()
                    .is_some_and(|c| c.eq_ignore_ascii_case(&i.md5))
                    && usable(s)
            });
        let processing = if same {
            shots
                .iter()
                .filter(|s| s.delivery_state == Some(DeliveryState::UploadComplete))
                .count()
        } else {
            0
        };
        let reusable = match_existing(&shots, images.iter().map(|i| i.md5.as_str()))
            .iter()
            .filter(|m| m.is_some())
            .count();
        if !shots.is_empty() {
            steps.push("delete_existing");
        }
        steps.extend(["upload", "set_order"]);
        Ok(RemoteSet {
            steps: steps.into_iter().map(str::to_owned).collect(),
            count: shots.len(),
            same,
            processing,
            reusable,
        })
    }

    #[allow(clippy::too_many_lines, clippy::too_many_arguments)] // one linear flow
    async fn replace(
        &self,
        client: &apple::Client,
        target: &AppleTarget,
        locale: &str,
        display_type: &str,
        files: &[UploadFile],
        opts: ReplaceOptions,
        hooks: &dyn SetHooks,
    ) -> Result<ReplaceOutcome> {
        let loc = Self::localization_id(client, target, locale, true)
            .await?
            .unwrap_or_default();
        let set = client.ensure_set(&loc, display_type).await?;
        // 1. Keep what the store already holds (same checksum, not failed),
        //    delete everything else (old images, leftovers of an aborted run).
        let old = client.list_screenshots(&set.id).await?;
        let kept = if opts.reuse {
            match_existing(&old, files.iter().map(|f| f.image.md5.as_str()))
        } else {
            vec![None; files.len()]
        };
        let keep_ids: Vec<&str> = kept.iter().flatten().map(|&k| old[k].id.as_str()).collect();
        let delete: Vec<&apple::AppScreenshot> = old
            .iter()
            .filter(|s| !keep_ids.contains(&s.id.as_str()))
            .collect();
        if !delete.is_empty() {
            hooks.step("delete_existing");
        }
        for s in delete {
            client.delete_screenshot(&s.id).await?;
        }
        // 2. Reserve/upload/commit every missing file; nobody waits for
        //    Apple's processing here.
        let mut ids: Vec<Option<String>> = vec![None; files.len()];
        let mut pending: Vec<(usize, String, Option<String>)> = Vec::new();
        let mut outcome = ReplaceOutcome::default();
        for (i, f) in files.iter().enumerate() {
            if let Some(k) = kept[i] {
                let s = &old[k];
                let processing = s.delivery_state != Some(DeliveryState::Complete);
                hooks.file_kept(i, &s.id, processing);
                if processing {
                    pending.push((i, s.id.clone(), None));
                }
                ids[i] = Some(s.id.clone());
                outcome.kept += 1;
                continue;
            }
            hooks.file_started(i);
            let progress = |p: &arrangia_publish::Progress| hooks.progress(i, p);
            let progress: ProgressFn<'_> = &progress;
            let shot = client
                .upload_screenshot_committed(&set.id, &f.image.file_name, f.bytes.clone(), progress)
                .await?;
            hooks.file_uploaded(i, &shot.id);
            ids[i] = Some(shot.id.clone());
            pending.push((i, shot.id, None));
        }
        // 3. Poll all of them together, up to the budget.
        let started = Instant::now();
        let mut interval = opts.poll.initial_interval;
        let mut failures: Vec<(usize, String, Vec<arrangia_publish::ApiErrorDetail>)> = Vec::new();
        let cancel = client.options().cancel.clone();
        while !pending.is_empty() {
            let mut still = Vec::new();
            for (i, id, last) in pending {
                let shot = client.screenshot(&id).await?;
                let state = shot
                    .delivery_state
                    .as_ref()
                    .map_or("UNKNOWN", DeliveryState::as_str)
                    .to_owned();
                if last.as_deref() != Some(state.as_str()) {
                    hooks.file_state(i, &state, started.elapsed());
                }
                match shot.delivery_state {
                    Some(DeliveryState::Complete) => hooks.file_done(i, &id),
                    Some(DeliveryState::Failed) => {
                        let msg = if shot.delivery_errors.is_empty() {
                            "Apple reported FAILED without details".to_owned()
                        } else {
                            shot.delivery_errors
                                .iter()
                                .map(ToString::to_string)
                                .collect::<Vec<_>>()
                                .join("; ")
                        };
                        hooks.file_failed(i, &id, &msg);
                        // Apple recommends deleting failed assets.
                        let _ = client.delete_screenshot(&id).await;
                        ids[i] = None;
                        failures.push((i, id, shot.delivery_errors));
                    }
                    _ => still.push((i, id, Some(state))),
                }
            }
            pending = still;
            let elapsed = started.elapsed();
            let states: Vec<(usize, String)> = pending
                .iter()
                .map(|(i, _, s)| (*i, s.clone().unwrap_or_default()))
                .collect();
            hooks.processing(&states, elapsed);
            if pending.is_empty() || elapsed >= opts.budget {
                break;
            }
            let wait = interval.min(opts.budget.saturating_sub(elapsed));
            tokio::select! {
                () = cancel.cancelled() => return Err(Error::Cancelled),
                () = tokio::time::sleep(wait) => {}
            }
            interval = (interval * 2).min(opts.poll.max_interval.max(opts.poll.initial_interval));
        }
        outcome.processing = pending.len();
        // 4. Order (also while some are processing; reservations are appended
        //    in upload order anyway) and verify.
        let order: Vec<String> = ids.iter().flatten().cloned().collect();
        hooks.step("set_order");
        if let Err(e) = client.reorder(&set.id, &order).await {
            if outcome.processing == 0 || matches!(e, Error::Cancelled) {
                return Err(e);
            }
            outcome.note = Some(format!(
                "the order could not be set while Apple is processing ({e}); check the store state later"
            ));
        }
        hooks.step("verify");
        let now: Vec<String> = client
            .list_screenshots(&set.id)
            .await?
            .into_iter()
            .map(|s| s.id)
            .collect();
        if now != order {
            if outcome.processing == 0 {
                return Err(Error::Conflict {
                    message: format!(
                        "the set holds {} screenshot(s) in another order than uploaded ({} expected)",
                        now.len(),
                        order.len()
                    ),
                    details: Vec::new(),
                });
            }
            outcome.note.get_or_insert_with(|| {
                "the order is not confirmed yet while Apple is processing; check the store state later"
                    .to_owned()
            });
        }
        if let Some((_, id, _)) = failures.first() {
            let details = failures.iter().flat_map(|(.., d)| d.clone()).collect();
            let names: Vec<&str> = failures
                .iter()
                .map(|(i, ..)| files[*i].image.file_name.as_str())
                .collect();
            return Err(Error::Processing {
                asset_id: format!("{} ({id})", names.join(", ")),
                details,
            });
        }
        outcome.ids = order;
        Ok(outcome)
    }
}

/// For each wanted checksum (in order) the index of a store screenshot
/// with that checksum that is `COMPLETE` or still processing; each store
/// screenshot is used once.
pub(crate) fn match_existing<'a>(
    shots: &[apple::AppScreenshot],
    wanted: impl Iterator<Item = &'a str>,
) -> Vec<Option<usize>> {
    let mut used = vec![false; shots.len()];
    wanted
        .map(|md5| {
            let k = shots.iter().enumerate().position(|(k, s)| {
                !used[k]
                    && matches!(
                        s.delivery_state,
                        Some(DeliveryState::Complete | DeliveryState::UploadComplete)
                    )
                    && s.source_file_checksum
                        .as_deref()
                        .is_some_and(|c| c.eq_ignore_ascii_case(md5))
            })?;
            used[k] = true;
            Some(k)
        })
        .collect()
}

// ----- flow selection (ADR 0019) -------------------------------------------------------

/// The project's `store.apple.upload_flow`.
pub(crate) fn flow_setting(project: &Project) -> AppleUploadFlowSetting {
    project
        .store
        .apple
        .as_ref()
        .map(|a| a.upload_flow)
        .unwrap_or_default()
}

/// The flow a store starts with (before the store is asked): `auto` and
/// `asset_library` → the asset library.
pub(crate) fn preferred_flow(setting: AppleUploadFlowSetting) -> AppleFlowKind {
    match setting {
        AppleUploadFlowSetting::ScreenshotSets => AppleFlowKind::ScreenshotSets,
        AppleUploadFlowSetting::Auto | AppleUploadFlowSetting::AssetLibrary => {
            AppleFlowKind::AssetLibrary
        }
    }
}

/// An opened App Store Connect app with the flow the store uses.
#[derive(Debug)]
pub(crate) struct AppleSession {
    pub target: AppleTarget,
    pub kind: AppleFlowKind,
    pub setting: AppleUploadFlowSetting,
    /// Why `auto` fell back to the set-based API.
    pub fallback: Option<String>,
}

/// Opens the app + editable version and, unless the setting forbids it,
/// the App Asset Library. `auto` falls back to the set-based API when the
/// library is not available for this app/key (403/404/400).
pub(crate) async fn open_apple(
    client: &apple::Client,
    app: &AppRef,
    platform: Platform,
    setting: AppleUploadFlowSetting,
) -> Result<AppleSession> {
    let mut target = ScreenshotSetFlow.open(client, app, platform).await?;
    if setting == AppleUploadFlowSetting::ScreenshotSets {
        return Ok(AppleSession {
            target,
            kind: AppleFlowKind::ScreenshotSets,
            setting,
            fallback: None,
        });
    }
    match AssetLibraryFlow::attach(client, &target.app.id).await {
        Ok(ctx) => {
            target.library = Some(ctx);
            Ok(AppleSession {
                target,
                kind: AppleFlowKind::AssetLibrary,
                setting,
                fallback: None,
            })
        }
        Err(e) if setting == AppleUploadFlowSetting::Auto && super::library::unavailable(&e) => {
            Ok(AppleSession {
                target,
                kind: AppleFlowKind::ScreenshotSets,
                setting,
                fallback: Some(format!(
                    "the App Asset Library is not available for this app or key ({}) – using the screenshot set API",
                    store_message(StoreKind::Apple, &e)
                )),
            })
        }
        Err(e) => Err(e),
    }
}

/// How one set gets into the store.
#[derive(Debug, Clone)]
pub(crate) enum Route {
    /// The set-based API (`note`: why, when `auto` fell back for this set).
    Sets { note: Option<String> },
    /// The asset library with these placements.
    Library(Vec<super::library::PlacementRoute>),
}

/// The route of one set: the store's flow, per-set fallback of `auto` to
/// the set-based API when the ref data lacks a group, or a blocking reason.
pub(crate) fn route(
    session: &AppleSession,
    slot: &str,
    images: &[PlanImage],
) -> std::result::Result<Route, String> {
    let library_only = super::library::asset_library_only(slot);
    let Some(ctx) = session.target.library.as_ref() else {
        if library_only {
            return Err(needs_library_message(slot, session.setting));
        }
        return Ok(Route::Sets { note: None });
    };
    match super::library::resolve(
        &ctx.ref_data,
        slot,
        images.first().map(|i| (i.width, i.height)),
    ) {
        Ok(routes) => Ok(Route::Library(routes)),
        Err(why) if session.setting == AppleUploadFlowSetting::Auto && !library_only => {
            Ok(Route::Sets {
                note: Some(format!("{why}; uploaded with the screenshot set API")),
            })
        }
        Err(why) => Err(why),
    }
}

/// The blocking message for a slot only the asset library can upload.
pub(crate) fn needs_library_message(slot: &str, setting: AppleUploadFlowSetting) -> String {
    let what = super::library::slot_from_key(slot);
    let name = if what.is_creative() {
        format!("the {} asset", what.placement().as_str().replace('_', " "))
    } else {
        format!("`{slot}`")
    };
    if setting == AppleUploadFlowSetting::ScreenshotSets {
        format!(
            "{name} can only be uploaded through the App Asset Library; the project forces the screenshot set API (store.apple.upload_flow: screenshot_sets)"
        )
    } else {
        format!(
            "{name} can only be uploaded through the App Asset Library, which is not available for this app or key"
        )
    }
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
