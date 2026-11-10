//! The upload plan (dry run, Phase 5.2).
//!
//! 1. Sets: every target with a store slot × its languages (filtered),
//!    each language mapped to the store locale (project map → defaults).
//! 2. Render: the images of those targets go to `.cache/upload/images`
//!    (the incremental export, so a second plan renders nothing), in store
//!    order; SHA-256 (+ MD5 for Apple's checksum) per image.
//! 3. Compare with the last successful upload in the log → `unchanged` /
//!    `replace` / `new`; with `remote` also with the store (Apple:
//!    `sourceFileChecksum` of the set's screenshots, Google: `sha256` of the
//!    listing images).
//! 4. Checks: validation errors of the target × language, store-mapping
//!    warnings (`store_locale_duplicate`, `store_slot_duplicate`), file
//!    sizes against the catalog rules and Google's 15 MB transport limit,
//!    account and app identity, preconditions (remote: editable version,
//!    package reachable). Blocking problems block the whole plan.
//! 5. Confirm token: random, stored with the fingerprint (revision, store
//!    mapping, selection, sets and hashes) and an expiry in the upload log.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use arrangia_core::Project;
use arrangia_core::catalog::{self, Store as CatalogStore};
use arrangia_core::model::{ApplePlatform, Lang, StoreKind, Target};
use arrangia_core::validate::{FindingCode, Severity, ValidationReport};
use arrangia_publish::google as gp;
use arrangia_render::export::{ExportOptions, ExportReport, FileKind};
use serde::{Deserialize, Serialize};

use super::log::UploadLog;
use super::remote::{self, AppleUploadFlow as _};
use super::{
    PLAN_VERSION, PlanImage, PlanSet, Problem, RemoteInfo, Selection, SetAction, StorePlan, Totals,
    UploadPlan, now_secs, sha256_hex,
};
use crate::commands::AppState;
use crate::error::{CmdResult, CommandError};
use crate::session::Session;

/// Render folder of the plan images (below the project).
pub const IMAGES_DIR: &str = ".cache/upload/images";

/// Arguments of `upload_plan`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanArgs {
    /// Only this store (default: every mapped store).
    #[serde(default)]
    pub store: Option<StoreKind>,
    /// Only these targets.
    #[serde(default)]
    pub targets: Vec<String>,
    /// Only these languages.
    #[serde(default)]
    pub langs: Vec<String>,
    /// Also read the store state (network; needs the account's key).
    #[serde(default)]
    pub remote: bool,
    /// Upload sets even when they look unchanged.
    #[serde(default)]
    pub force: bool,
}

impl PlanArgs {
    pub fn selection(&self) -> Selection {
        Selection {
            stores: self.store.into_iter().collect(),
            targets: self.targets.clone(),
            langs: self.langs.clone(),
            remote: self.remote,
            force: self.force,
        }
    }

    pub fn from_selection(s: &Selection) -> Self {
        Self {
            store: s.stores.first().copied(),
            targets: s.targets.clone(),
            langs: s.langs.clone(),
            remote: s.remote,
            force: s.force,
        }
    }
}

/// Folder of the plan images of the project in `dir`.
pub fn images_dir(dir: &Path) -> PathBuf {
    dir.join(".cache").join("upload").join("images")
}

/// Slot value of `target` for `store`.
fn slot_of(target: &Target, store: StoreKind) -> Option<&str> {
    let slot = target.store.as_ref()?;
    match store {
        StoreKind::Apple => slot.apple.as_ref().map(|a| a.display_type.as_str()),
        StoreKind::Google => slot.google.as_ref().map(|g| g.image_type.as_str()),
    }
}

fn langs_of(project: &Project, target: &Target) -> Vec<String> {
    target
        .langs
        .as_ref()
        .map_or_else(|| project.languages.available.clone(), Clone::clone)
        .iter()
        .map(|l: &Lang| l.as_str().to_owned())
        .collect()
}

/// App identity used in the log keys.
fn app_key(project: &Project, store: StoreKind) -> Option<String> {
    match store {
        StoreKind::Apple => {
            let a = project.store.apple.as_ref()?;
            a.app_id.clone().or_else(|| a.bundle_id.clone())
        }
        StoreKind::Google => project.store.google.as_ref()?.package_name.clone(),
    }
}

/// The catalog's maximum file size for a store slot.
fn max_file_size(store: StoreKind, slot: &str) -> Option<u64> {
    let cat = catalog::catalog();
    cat.targets
        .presets
        .iter()
        .filter(|p| {
            p.slot.as_ref().is_some_and(|s| match store {
                StoreKind::Apple => s.apple.as_ref().is_some_and(|a| a.display_type == slot),
                StoreKind::Google => s.google.as_ref().is_some_and(|g| g.image_type == slot),
            })
        })
        .find_map(|p| p.rules.as_deref())
        .and_then(|r| cat.store_rules(r))
        .filter(|r| {
            matches!(
                (r.store, store),
                (CatalogStore::Apple, StoreKind::Apple) | (CatalogStore::Google, StoreKind::Google)
            )
        })
        .and_then(|r| r.max_file_size)
}

/// Apple display types of macOS versions.
fn is_mac_display_type(slot: &str) -> bool {
    slot == "APP_DESKTOP"
}

/// The local part of a plan, before the token.
pub(crate) struct LocalPlan {
    pub plan: UploadPlan,
    pub fingerprint: String,
}

#[derive(Serialize)]
struct FingerprintInput<'a> {
    project: &'a str,
    revision: u64,
    store: &'a arrangia_core::model::StoreSettings,
    selection: &'a Selection,
    sets: Vec<FingerprintSet<'a>>,
}

/// `(key, target, lang, locale, image hashes)`.
type FingerprintSet<'a> = (&'a str, &'a str, &'a str, Option<&'a str>, Vec<&'a str>);

fn fingerprint(plan: &UploadPlan, project: &Project) -> String {
    let input = FingerprintInput {
        project: &plan.project,
        revision: plan.revision,
        store: &project.store,
        selection: &Selection {
            remote: false,
            ..plan.selection.clone()
        },
        sets: plan
            .sets()
            .map(|(_, s)| {
                (
                    s.key.as_str(),
                    s.target.as_str(),
                    s.lang.as_str(),
                    s.locale.as_deref(),
                    s.images.iter().map(|i| i.sha256.as_str()).collect(),
                )
            })
            .collect(),
    };
    sha256_hex(serde_json::to_string(&input).unwrap_or_default().as_bytes())
}

/// Validation findings that block a set (target × language).
fn validation_problems(report: &ValidationReport, target: &str, lang: &str) -> Vec<Problem> {
    let mut out = Vec::new();
    for f in &report.findings {
        let t_ok = f.target.as_ref().is_none_or(|t| t.as_str() == target);
        let l_ok = f.lang.as_ref().is_none_or(|l| l.as_str() == lang);
        if !(t_ok && l_ok) {
            continue;
        }
        let store_warning = matches!(
            f.code,
            FindingCode::StoreLocaleDuplicate
                | FindingCode::StoreSlotDuplicate
                | FindingCode::StoreLocaleUnknown
        );
        if f.target.is_none() && !store_warning {
            continue;
        }
        if f.level == Severity::Error || store_warning {
            out.push(Problem::blocking(f.code.as_str(), f.message.clone()));
        } else if f.level == Severity::Warn && f.target.is_some() {
            out.push(Problem::warn(f.code.as_str(), f.message.clone()));
        }
    }
    out
}

/// Builds the local plan (no network): sets, rendered images, hashes,
/// log comparison and checks.
#[allow(clippy::too_many_lines)]
pub(crate) fn build_local(
    app: &AppState,
    session: &Session,
    args: &PlanArgs,
) -> CmdResult<LocalPlan> {
    let project = session.snapshot();
    for t in &args.targets {
        if project.target(t).is_none() {
            return Err(CommandError::not_found(format!("unknown target `{t}`")));
        }
    }
    for l in &args.langs {
        if !project.languages.is_available(l) {
            return Err(CommandError::not_found(format!(
                "`{l}` is not a project language"
            )));
        }
    }
    let dir = session.dir().to_path_buf();
    let accounts = app.settings_store().load()?.store_accounts;
    let selection = args.selection();
    let mut plan = UploadPlan {
        version: PLAN_VERSION,
        confirm_token: String::new(),
        created_at: now_secs(),
        expires_at: 0,
        project: dir.display().to_string(),
        revision: session.revision(),
        selection,
        stores: Vec::new(),
        problems: Vec::new(),
        totals: Totals::default(),
        blocked: false,
    };
    let stores: Vec<StoreKind> = StoreKind::ALL
        .into_iter()
        .filter(|s| args.store.is_none_or(|x| x == *s))
        .filter(|s| project.store.has(*s))
        .collect();
    if stores.is_empty() {
        plan.problems.push(Problem::blocking(
            "no_store",
            match args.store {
                Some(s) => format!("the project is not mapped to {s} (Project settings → Stores)"),
                None => "the project is not mapped to a store (Project settings → Stores)".into(),
            },
        ));
    }
    let wanted =
        |t: &Target| args.targets.is_empty() || args.targets.iter().any(|x| x == t.id.as_str());
    let apple_platform = project
        .store
        .apple
        .as_ref()
        .map(|a| a.platform)
        .unwrap_or_default();
    // Targets to render: a slot of a selected store (Apple: of the
    // project's platform; the others are listed as `skip`).
    let renders = |t: &Target, s: StoreKind| {
        slot_of(t, s).is_some_and(|slot| {
            s != StoreKind::Apple
                || is_mac_display_type(slot) == (apple_platform == ApplePlatform::Macos)
        })
    };
    let slotted: Vec<&Target> = project
        .targets
        .iter()
        .filter(|t| wanted(t) && stores.iter().any(|s| renders(t, *s)))
        .collect();

    // Validation (export mode) of the slotted targets.
    let report = if slotted.is_empty() {
        ValidationReport::default()
    } else {
        crate::commands::validate(
            app,
            crate::commands::ValidateArgs {
                targets: Some(slotted.iter().map(|t| t.id.to_string()).collect()),
                langs: (!args.langs.is_empty()).then(|| args.langs.clone()),
                preview: false,
            },
        )?
        .report
    };

    // Render (incremental) into the plan folder.
    let export = if slotted.is_empty() {
        ExportReport::default()
    } else {
        arrangia_render::export::export(
            &project,
            &session.render_context(),
            &ExportOptions {
                out_dir: Some(images_dir(&dir)),
                targets: slotted.iter().map(|t| t.id.to_string()).collect(),
                langs: args.langs.clone(),
                gallery: false,
                incremental: true,
                ..ExportOptions::default()
            },
        )
    };
    let out_dir = images_dir(&dir);
    let log = UploadLog::open(&dir)?;

    for store in stores {
        let mut sp = StorePlan {
            store,
            account: project.store.account(store).map(str::to_owned),
            app: app_key(&project, store),
            flow: match store {
                StoreKind::Apple => remote::apple_flow().kind().as_str().to_owned(),
                StoreKind::Google => "edit".to_owned(),
            },
            remote: None,
            sets: Vec::new(),
            problems: Vec::new(),
        };
        match &sp.account {
            None => sp.problems.push(Problem::blocking(
                "no_account",
                format!("no {store} account selected (Project settings → Stores)"),
            )),
            Some(id) => match accounts.iter().find(|a| &a.id == id) {
                None => sp.problems.push(Problem::blocking(
                    "unknown_account",
                    format!(
                        "store account `{id}` is not set up on this machine (App settings → Stores)"
                    ),
                )),
                Some(a) if a.kind != store => sp.problems.push(Problem::blocking(
                    "wrong_account",
                    format!("store account `{id}` is a {} account", a.kind),
                )),
                Some(_) => {}
            },
        }
        if sp.app.is_none() {
            sp.problems.push(Problem::blocking(
                "no_app",
                match store {
                    StoreKind::Apple => "no Apple app id or bundle id (Project settings → Stores)",
                    StoreKind::Google => "no Google Play package name (Project settings → Stores)",
                },
            ));
        }
        let platform = apple_platform;
        let list = catalog::store_locales().store(store);
        let explicit = project.store.locales.get(store);
        let mut seen: BTreeMap<String, String> = BTreeMap::new();
        for target in project.targets.iter().filter(|t| wanted(t)) {
            let Some(slot) = slot_of(target, store) else {
                continue;
            };
            for lang in langs_of(&project, target) {
                if !args.langs.is_empty() && !args.langs.contains(&lang) {
                    continue;
                }
                let locale = list.resolve(explicit, &lang).map(|(c, _)| c.to_owned());
                let mut set = PlanSet {
                    key: format!("{store}:{}:{slot}", locale.as_deref().unwrap_or("-")),
                    target: target.id.to_string(),
                    lang: lang.clone(),
                    locale: locale.clone(),
                    slot: slot.to_owned(),
                    action: SetAction::New,
                    reason: None,
                    steps: Vec::new(),
                    remote_count: None,
                    images: Vec::new(),
                    bytes: 0,
                    problems: Vec::new(),
                };
                if locale.is_none() {
                    set.action = SetAction::Skip;
                    set.reason = Some(format!(
                        "language `{lang}` has no {store} store locale (store.locales.{store})"
                    ));
                    sp.sets.push(set);
                    continue;
                }
                if store == StoreKind::Apple {
                    let mac = is_mac_display_type(slot);
                    if mac != (platform == ApplePlatform::Macos) {
                        set.action = SetAction::Skip;
                        set.reason = Some(format!(
                            "`{slot}` belongs to the {} version; the project uploads to {}",
                            if mac { "macOS" } else { "iOS" },
                            if platform == ApplePlatform::Macos {
                                "macOS"
                            } else {
                                "iOS"
                            }
                        ));
                        sp.sets.push(set);
                        continue;
                    }
                }
                if let Some(other) = seen.insert(set.key.clone(), format!("{}/{lang}", target.id)) {
                    set.problems.push(Problem::blocking(
                        "duplicate_set",
                        format!(
                            "{other} fills the same store set ({}, {slot})",
                            locale.as_deref().unwrap_or_default()
                        ),
                    ));
                }
                set.problems
                    .extend(validation_problems(&report, target.id.as_str(), &lang));
                for e in &export.errors {
                    if e.starts_with(&format!("{}/{lang}:", target.id)) {
                        set.problems
                            .push(Problem::blocking("render_failed", e.clone()));
                    }
                }
                // Images in store order.
                let mut files: Vec<_> = export
                    .files
                    .iter()
                    .filter(|f| {
                        f.target == target.id.as_str()
                            && f.lang == lang
                            && f.kind != FileKind::Gallery
                    })
                    .collect();
                files.sort_by_key(|f| f.image.unwrap_or(0));
                let limit = max_file_size(store, slot);
                for (n, f) in files.iter().enumerate() {
                    let path = out_dir.join(&f.path);
                    let bytes = std::fs::read(&path)
                        .map_err(|e| CommandError::io(format!("{}: {e}", path.display())))?;
                    let size = bytes.len() as u64;
                    let file_name = format!(
                        "{}-{lang}-{}",
                        target.id,
                        f.path.rsplit('/').next().unwrap_or(&f.path)
                    );
                    if let Some(max) = limit
                        && size > max
                    {
                        set.problems.push(Problem::blocking(
                            "file_too_large",
                            format!(
                                "{file_name}: {} – the store accepts at most {}",
                                super::human_bytes(size),
                                super::human_bytes(max)
                            ),
                        ));
                    }
                    if store == StoreKind::Google && size > gp::MAX_IMAGE_BYTES as u64 {
                        set.problems.push(Problem::blocking(
                            "file_too_large",
                            format!("{file_name}: larger than Google Play's 15 MB upload limit"),
                        ));
                    }
                    set.bytes += size;
                    set.images.push(PlanImage {
                        index: u32::try_from(n + 1).unwrap_or(u32::MAX),
                        file: f.path.clone(),
                        file_name,
                        width: f.width,
                        height: f.height,
                        bytes: size,
                        sha256: sha256_hex(&bytes),
                        md5: arrangia_publish::apple::md5_hex(&bytes),
                    });
                }
                if set.images.is_empty() && !set.problems.iter().any(|p| p.blocking) {
                    set.problems.push(Problem::blocking(
                        "no_images",
                        "no images were rendered for this set",
                    ));
                }
                set.steps = default_steps(store);
                if set.problems.iter().any(|p| p.blocking) {
                    set.action = SetAction::Blocked;
                } else if let (Some(app), Some(loc)) = (&sp.app, &set.locale) {
                    let hashes: Vec<String> = set.images.iter().map(|i| i.sha256.clone()).collect();
                    match log.last_success(store.as_str(), app, loc, slot)? {
                        Some((h, at)) if h == hashes && !args.force => {
                            set.action = SetAction::Unchanged;
                            set.reason =
                                Some(format!("same images as the upload at {}", fmt_time(at)));
                        }
                        Some(_) => set.action = SetAction::Replace,
                        None => set.action = SetAction::New,
                    }
                }
                sp.sets.push(set);
            }
        }
        if sp.sets.is_empty() {
            sp.problems.push(Problem::warn(
                "no_sets",
                format!("no target has a {store} store slot (nothing to upload)"),
            ));
        }
        plan.stores.push(sp);
    }
    finish_totals(&mut plan);
    let fingerprint = fingerprint(&plan, &project);
    Ok(LocalPlan { plan, fingerprint })
}

fn default_steps(store: StoreKind) -> Vec<String> {
    match store {
        StoreKind::Apple => vec!["delete_existing", "upload", "set_order"],
        StoreKind::Google => vec!["delete_existing", "upload"],
    }
    .into_iter()
    .map(str::to_owned)
    .collect()
}

/// `2026-11-10 14:03 UTC`.
pub(crate) fn fmt_time(secs: u64) -> String {
    let days = secs / 86_400;
    let rem = secs % 86_400;
    // Civil date from days since 1970-01-01 (Howard Hinnant's algorithm).
    let z = i64::try_from(days).unwrap_or(0) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!(
        "{y:04}-{m:02}-{d:02} {:02}:{:02} UTC",
        rem / 3600,
        (rem % 3600) / 60
    )
}

/// Recomputes totals and `blocked`.
pub(crate) fn finish_totals(plan: &mut UploadPlan) {
    let mut t = Totals::default();
    let mut blocked = plan.problems.iter().any(|p| p.blocking);
    for sp in &plan.stores {
        let store_blocked = sp.problems.iter().any(|p| p.blocking);
        blocked |= store_blocked;
        for s in &sp.sets {
            t.sets += 1;
            match s.action {
                SetAction::New | SetAction::Replace => {
                    t.upload_sets += 1;
                    t.images += s.images.len();
                    t.bytes += s.bytes;
                }
                SetAction::Unchanged => t.unchanged_sets += 1,
                SetAction::Skip => t.skipped_sets += 1,
                SetAction::Blocked => {
                    t.blocked_sets += 1;
                    blocked = true;
                }
            }
        }
    }
    plan.totals = t;
    plan.blocked = blocked;
}

// ----- remote check -------------------------------------------------------------------

/// Reads the store state and refines actions/steps (Apple: editable
/// version, localizations, sets, checksums; Google: package reachable,
/// listing image hashes).
pub(crate) fn check_remote(
    app: &AppState,
    session: &Session,
    plan: &mut UploadPlan,
) -> CmdResult<()> {
    let project = session.snapshot();
    let config = app.upload().config();
    let force = plan.selection.force;
    for sp in &mut plan.stores {
        if sp.problems.iter().any(|p| p.blocking) {
            sp.remote = Some(RemoteInfo {
                ok: false,
                error: Some("not checked: fix the problems above first".into()),
                ..RemoteInfo::default()
            });
            continue;
        }
        let (account, secret) = match remote::credentials(app, &project, sp.store) {
            Ok(c) => c,
            Err(e) => {
                sp.problems
                    .push(Problem::blocking("credentials", e.message));
                continue;
            }
        };
        let info = match sp.store {
            StoreKind::Apple => {
                remote::block_on(check_apple(&config, &account, &secret, &project, sp, force))?
            }
            StoreKind::Google => remote::block_on(check_google(&config, &secret, sp, force))?,
        };
        sp.remote = Some(info);
    }
    finish_totals(plan);
    Ok(())
}

fn refine(set: &mut PlanSet, count: usize, same: bool, steps: Vec<String>, force: bool) {
    set.remote_count = Some(count);
    set.steps = steps;
    if !set.action.uploads() && set.action != SetAction::Unchanged {
        return;
    }
    set.action = if same && !force {
        set.reason = Some("the store already holds these images in this order".into());
        SetAction::Unchanged
    } else if count > 0 {
        set.reason = None;
        SetAction::Replace
    } else {
        set.reason = None;
        SetAction::New
    };
}

async fn check_apple(
    config: &super::UploadConfig,
    account: &crate::stores::StoreAccount,
    secret: &arrangia_publish::credentials::Secret,
    project: &Project,
    sp: &mut StorePlan,
    force: bool,
) -> RemoteInfo {
    let flow = remote::apple_flow();
    let fail = |sp: &mut StorePlan, code: &str, e: &arrangia_publish::Error| {
        let msg = remote::store_message(StoreKind::Apple, e);
        sp.problems.push(Problem::blocking(code, msg.clone()));
        RemoteInfo {
            ok: false,
            error: Some(msg),
            ..RemoteInfo::default()
        }
    };
    let client = match remote::apple_client(config, account, secret, config.options.clone()) {
        Ok(c) => c,
        Err(e) => return fail(sp, "credentials", &e),
    };
    let Some(app_ref) = remote::apple_app_ref(project) else {
        return RemoteInfo::default();
    };
    let platform = remote::apple_platform(
        project
            .store
            .apple
            .as_ref()
            .map(|a| a.platform)
            .unwrap_or_default(),
    );
    let target = match flow.open(&client, &app_ref, platform).await {
        Ok(t) => t,
        Err(e) => {
            let code = if e.kind() == arrangia_publish::ErrorKind::Conflict {
                "no_editable_version"
            } else {
                "app_not_reachable"
            };
            return fail(sp, code, &e);
        }
    };
    let mut ok = true;
    for set in &mut sp.sets {
        if !(set.action.uploads() || set.action == SetAction::Unchanged) {
            continue;
        }
        let Some(locale) = set.locale.clone() else {
            continue;
        };
        match flow
            .inspect(&client, &target, &locale, &set.slot, &set.images)
            .await
        {
            Ok(r) => refine(set, r.count, r.same, r.steps, force),
            Err(e) => {
                ok = false;
                set.problems.push(Problem::warn(
                    "remote_check_failed",
                    remote::store_message(StoreKind::Apple, &e),
                ));
            }
        }
    }
    RemoteInfo {
        ok,
        app_name: Some(target.app.name.clone()),
        version: Some(format!(
            "{} {}",
            target.version.version_string,
            target.version.state.as_deref().unwrap_or("?")
        )),
        error: None,
    }
}

async fn check_google(
    config: &super::UploadConfig,
    secret: &arrangia_publish::credentials::Secret,
    sp: &mut StorePlan,
    force: bool,
) -> RemoteInfo {
    let package = sp.app.clone().unwrap_or_default();
    let fail = |sp: &mut StorePlan, code: &str, e: &arrangia_publish::Error| {
        let msg = remote::store_message(StoreKind::Google, e);
        sp.problems.push(Problem::blocking(code, msg.clone()));
        RemoteInfo {
            ok: false,
            error: Some(msg),
            ..RemoteInfo::default()
        }
    };
    let client = match remote::google_client(config, secret, &package, config.options.clone()) {
        Ok(c) => c,
        Err(e) => return fail(sp, "credentials", &e),
    };
    // A read-only edit: opened, listed, deleted (never committed).
    let edit = match client.insert_edit().await {
        Ok(e) => e,
        Err(e) => return fail(sp, "app_not_reachable", &e),
    };
    let mut ok = true;
    for set in &mut sp.sets {
        if !(set.action.uploads() || set.action == SetAction::Unchanged) {
            continue;
        }
        let (Some(locale), Some(kind)) = (set.locale.clone(), gp::ImageType::parse(&set.slot))
        else {
            continue;
        };
        match client.list_images(&edit.id, &locale, kind).await {
            Ok(images) => {
                let same = images.len() == set.images.len()
                    && images.iter().zip(&set.images).all(|(r, l)| {
                        r.sha256
                            .as_deref()
                            .is_some_and(|h| h.eq_ignore_ascii_case(&l.sha256))
                    });
                let mut steps = Vec::new();
                if !images.is_empty() {
                    steps.push("delete_existing".to_owned());
                }
                steps.push("upload".to_owned());
                refine(set, images.len(), same, steps, force);
            }
            Err(e) => {
                ok = false;
                set.problems.push(Problem::warn(
                    "remote_check_failed",
                    remote::store_message(StoreKind::Google, &e),
                ));
            }
        }
    }
    let _ = client.delete_edit(&edit.id).await;
    RemoteInfo {
        ok,
        app_name: Some(package),
        version: None,
        error: None,
    }
}

// ----- token --------------------------------------------------------------------------

/// Makes the full plan: local (+ remote), token, stored in the log.
pub(crate) fn make_plan(
    app: &AppState,
    session: &Session,
    args: &PlanArgs,
) -> CmdResult<UploadPlan> {
    let LocalPlan {
        mut plan,
        fingerprint,
    } = build_local(app, session, args)?;
    if args.remote && !plan.problems.iter().any(|p| p.blocking) {
        check_remote(app, session, &mut plan)?;
    }
    let ttl = app.upload().config().token_ttl;
    plan.expires_at = plan.created_at + ttl.as_secs();
    let mut nonce = [0u8; 16];
    getrandom::fill(&mut nonce).map_err(|e| CommandError::internal(format!("random: {e}")))?;
    let token =
        sha256_hex(format!("{fingerprint}|{}|{}", plan.expires_at, super::hex(&nonce)).as_bytes());
    plan.confirm_token = format!("upl_{}", &token[..32]);
    let json = serde_json::to_string(&plan).map_err(|e| CommandError::internal(e.to_string()))?;
    UploadLog::open(session.dir())?.save_plan(
        &plan.confirm_token,
        &fingerprint,
        plan.created_at,
        plan.expires_at,
        &json,
    )?;
    Ok(plan)
}

/// Why a token cannot be executed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TokenProblem {
    Unknown,
    Expired,
    Used,
    Changed,
    Blocked,
}

/// Checks `token` against the log and the current project state; returns
/// the stored plan.
pub(crate) fn verify_token(
    app: &AppState,
    session: &Session,
    token: &str,
) -> CmdResult<UploadPlan> {
    let fail = |reason: TokenProblem, message: &str| {
        CommandError::new(crate::error::ErrorCode::NotConfirmed, message)
            .with_details(serde_json::json!({ "reason": reason }))
    };
    let log = UploadLog::open(session.dir())?;
    let Some(stored) = log.plan(token)? else {
        return Err(fail(
            TokenProblem::Unknown,
            "unknown confirm token – make a plan first (upload_plan)",
        ));
    };
    if stored.used {
        return Err(fail(
            TokenProblem::Used,
            "this confirm token was already used – make a new plan",
        ));
    }
    if now_secs() > stored.expires_at {
        return Err(fail(
            TokenProblem::Expired,
            "the confirm token expired – make a new plan",
        ));
    }
    let plan: UploadPlan = serde_json::from_str(&stored.plan_json)
        .map_err(|e| CommandError::internal(format!("stored plan: {e}")))?;
    if plan.blocked {
        return Err(fail(
            TokenProblem::Blocked,
            "the plan has blocking problems – fix them and make a new plan",
        ));
    }
    let args = PlanArgs::from_selection(&plan.selection);
    let now = build_local(app, session, &args)?;
    if now.fingerprint != stored.fingerprint {
        return Err(fail(
            TokenProblem::Changed,
            "the project or its images changed since the plan – make a new plan",
        ));
    }
    Ok(plan)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn times_format() {
        assert_eq!(fmt_time(0), "1970-01-01 00:00 UTC");
        assert_eq!(fmt_time(1_794_312_000), "2026-11-10 12:00 UTC");
    }
}
