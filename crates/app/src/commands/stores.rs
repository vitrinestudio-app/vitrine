//! Store accounts (app-wide, secrets in the keychain) and the project's
//! store mapping (`store:` in `project.yaml`), Phase 5.1 / ADR 0016.
//!
//! - `store_accounts_list`, `store_account_import {kind, path, label,
//!   issuer_id?, key_id?}`, `store_account_remove {id}`,
//!   `store_account_test {id}`;
//! - `store_apps_list {account, refresh?}`: the Apple apps a key can see
//!   (app picker, cached for the app session), `store_package_check
//!   {account, package_name}`: does Play know the package and may the
//!   service account edit it (temporary edit, deleted again);
//! - `project_store_get`, `project_store_patch` (one `set_store` op);
//! - `store_config_get`: read-only overview for MCP (no secrets).
//!
//! Key files are read here, validated by `arrangia_publish::credentials`
//! and go straight to the keychain; their content never reaches a result,
//! an error message or the settings file.

use std::collections::BTreeMap;
use std::path::PathBuf;

use arrangia_core::catalog::{self, LocaleSource, StoreLocale};
use arrangia_core::model::{AppleStore, GoogleStore, Project, StoreKind, StoreSettings};
use arrangia_core::ops::Op;
use arrangia_publish::credentials::{self, Backend, CredentialError, Secret};
use serde::{Deserialize, Serialize};

use super::edit::double_option;
use super::{AppState, NoArgs};
use crate::error::{CmdResult, CommandError, ErrorCode};
use crate::session::ApplyResult;
use crate::stores::{AppleAccount, ConnectionInfo, GoogleAccount, StoreAccount, new_account_id};

pub(crate) fn keychain_error(e: CredentialError) -> CommandError {
    match e {
        CredentialError::InvalidKey(m) => CommandError::invalid(m),
        CredentialError::Missing(_) => CommandError::not_found(e.to_string()),
        CredentialError::Keychain(_) => CommandError::new(ErrorCode::Keychain, e.to_string()),
    }
}

// ----- accounts -----------------------------------------------------------------------

/// Where secrets are kept (shown in the settings).
#[derive(Debug, Clone, Serialize)]
pub struct KeychainInfo {
    pub service: String,
    pub backend: Option<Backend>,
    /// Why the keychain is unavailable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct StoreAccountsList {
    pub accounts: Vec<StoreAccount>,
    pub keychain: KeychainInfo,
}

fn keychain_info(app: &AppState) -> KeychainInfo {
    match app.secret_store() {
        Ok(s) => KeychainInfo {
            service: s.service().to_owned(),
            backend: Some(s.backend()),
            error: None,
        },
        Err(e) => KeychainInfo {
            service: credentials::DEFAULT_SERVICE.to_owned(),
            backend: None,
            error: Some(e.message),
        },
    }
}

/// The accounts (no secrets; the keychain is not read).
pub fn store_accounts_list(app: &AppState, _: NoArgs) -> CmdResult<StoreAccountsList> {
    Ok(StoreAccountsList {
        accounts: app.settings_store().load()?.store_accounts,
        keychain: keychain_info(app),
    })
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StoreAccountImportArgs {
    pub kind: StoreKind,
    /// The key file: Apple `AuthKey_<KEYID>.p8`, Google service account `.json`.
    pub path: PathBuf,
    /// Display name (default: key id / client email).
    #[serde(default)]
    pub label: Option<String>,
    /// Apple: issuer id (App Store Connect → Users and Access → Integrations).
    #[serde(default)]
    pub issuer_id: Option<String>,
    /// Apple: key id (default: from the file name `AuthKey_<KEYID>.p8`).
    #[serde(default)]
    pub key_id: Option<String>,
}

fn read_key_file(path: &std::path::Path) -> CmdResult<String> {
    let meta = std::fs::metadata(path)
        .map_err(|e| CommandError::not_found(format!("{}: {e}", path.display())))?;
    if !meta.is_file() {
        return Err(CommandError::invalid(format!(
            "{} is not a file",
            path.display()
        )));
    }
    if meta.len() > credentials::MAX_KEY_FILE {
        return Err(CommandError::invalid(format!(
            "{} is too large for a key file",
            path.display()
        )));
    }
    let bytes =
        std::fs::read(path).map_err(|e| CommandError::io(format!("{}: {e}", path.display())))?;
    String::from_utf8(bytes)
        .map_err(|_| CommandError::invalid(format!("{} is not a text file", path.display())))
}

/// Validates the key file, stores the secret in the keychain and adds the
/// account to the settings.
pub fn store_account_import(
    app: &AppState,
    args: StoreAccountImportArgs,
) -> CmdResult<StoreAccount> {
    let text = read_key_file(&args.path)?;
    let file_name = args
        .path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let (apple, google, default_label) = match args.kind {
        StoreKind::Apple => {
            credentials::validate_apple_key(&text).map_err(keychain_error)?;
            let key_id = args
                .key_id
                .map(|k| k.trim().to_owned())
                .filter(|k| !k.is_empty())
                .or_else(|| credentials::apple_key_id_from_file_name(&file_name))
                .ok_or_else(|| {
                    CommandError::invalid(
                        "key id missing: enter it or keep Apple's file name AuthKey_<KEYID>.p8",
                    )
                })?;
            let issuer_id = args
                .issuer_id
                .map(|i| i.trim().to_ascii_lowercase())
                .filter(|i| !i.is_empty())
                .ok_or_else(|| {
                    CommandError::invalid(
                        "issuer id missing (App Store Connect → Users and Access → Integrations)",
                    )
                })?;
            let label = format!("Apple {key_id}");
            (Some(AppleAccount { issuer_id, key_id }), None, label)
        }
        StoreKind::Google => {
            let info = credentials::validate_google_key(&text).map_err(keychain_error)?;
            let label = info.client_email.clone();
            let google = GoogleAccount {
                client_email: info.client_email,
                project_id: info.project_id,
            };
            (None, Some(google), label)
        }
    };
    let label = args
        .label
        .map(|l| l.trim().to_owned())
        .filter(|l| !l.is_empty())
        .unwrap_or(default_label);
    let settings_store = app.settings_store();
    let mut settings = settings_store.load()?;
    let account = StoreAccount {
        id: new_account_id(args.kind, &label, &settings.store_accounts),
        kind: args.kind,
        label,
        apple,
        google,
    };
    account.check().map_err(CommandError::invalid)?;
    let secrets = app.secret_store()?;
    secrets
        .set(&account.id, account.kind, &Secret::new(text))
        .map_err(keychain_error)?;
    settings.store_accounts.push(account.clone());
    if let Err(e) = settings_store.save(&settings) {
        // Do not leave an orphaned keychain entry behind.
        let _ = secrets.delete(&account.id, account.kind);
        return Err(e);
    }
    Ok(account)
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StoreAccountIdArgs {
    pub id: String,
}

fn find_account(app: &AppState, id: &str) -> CmdResult<StoreAccount> {
    app.settings_store()
        .load()?
        .store_accounts
        .into_iter()
        .find(|a| a.id == id)
        .ok_or_else(|| CommandError::not_found(format!("unknown store account `{id}`")))
}

#[derive(Debug, Clone, Serialize)]
pub struct StoreAccountRemoved {
    pub id: String,
}

/// Deletes the keychain entry and removes the account from the settings
/// (projects keep their reference; validation of the upload reports it).
pub fn store_account_remove(
    app: &AppState,
    args: StoreAccountIdArgs,
) -> CmdResult<StoreAccountRemoved> {
    let account = find_account(app, &args.id)?;
    app.secret_store()?
        .delete(&account.id, account.kind)
        .map_err(keychain_error)?;
    let store = app.settings_store();
    let mut settings = store.load()?;
    settings.store_accounts.retain(|a| a.id != account.id);
    store.save(&settings)?;
    app.cache_store_apps(&account.id, None);
    Ok(StoreAccountRemoved { id: account.id })
}

#[derive(Debug, Clone, Serialize)]
pub struct StoreAccountTest {
    pub id: String,
    pub ok: bool,
    #[serde(flatten)]
    pub info: ConnectionInfo,
}

/// Reads the secret from the keychain, checks it again and asks the store
/// who we are (read-only, see `crate::store_test`). Without an installed
/// tester (CLI, headless MCP) the answer is `not_implemented` (details:
/// `secret: "ok"`). A failed test is an `io` error with `details.reason`
/// (`key_rejected`, `no_permission`, `offline`, …).
pub fn store_account_test(app: &AppState, args: StoreAccountIdArgs) -> CmdResult<StoreAccountTest> {
    let account = find_account(app, &args.id)?;
    let secret = app
        .secret_store()?
        .get(&account.id, account.kind)
        .map_err(keychain_error)?;
    match account.kind {
        StoreKind::Apple => credentials::validate_apple_key(secret.expose()),
        StoreKind::Google => credentials::validate_google_key(secret.expose()).map(|_| ()),
    }
    .map_err(|e| {
        CommandError::invalid(format!("the key stored in the keychain is not valid: {e}"))
    })?;
    let Some(tester) = app.connection_tester() else {
        return Err(CommandError::new(
            ErrorCode::NotImplemented,
            "the key is in the keychain and valid; the connection test is only available \
             in the app",
        )
        .with_details(serde_json::json!({ "secret": "ok" })));
    };
    match tester.test(&account, &secret) {
        Ok(info) => Ok(StoreAccountTest {
            id: account.id,
            ok: true,
            info,
        }),
        Err(e) => Err(CommandError::new(ErrorCode::Io, e.message)
            .with_details(serde_json::json!({ "reason": e.reason }))),
    }
}

// ----- app picker (Apple) and package check (Google) ---------------------------------

/// Client options of the interactive store calls (short timeout, one retry;
/// like the connection test).
fn interactive_options() -> arrangia_publish::ClientOptions {
    crate::store_test::StoreClientTester::default().options
}

fn store_failure(store: StoreKind, e: &arrangia_publish::Error) -> CommandError {
    let e = crate::store_test::map_error(store, e);
    CommandError::new(ErrorCode::Io, e.message)
        .with_details(serde_json::json!({ "reason": e.reason }))
}

/// An account of `kind` with its secret from the keychain.
fn account_with_secret(
    app: &AppState,
    id: &str,
    kind: StoreKind,
) -> CmdResult<(StoreAccount, Secret)> {
    let account = find_account(app, id)?;
    if account.kind != kind {
        return Err(CommandError::invalid(format!(
            "store account `{id}` is a {} account, not {kind}",
            account.kind
        )));
    }
    let secret = app
        .secret_store()?
        .get(&account.id, account.kind)
        .map_err(keychain_error)?;
    Ok((account, secret))
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StoreAppsListArgs {
    /// Apple account id.
    pub account: String,
    /// Ask App Store Connect again instead of using this session's list.
    #[serde(default)]
    pub refresh: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct StoreAppsList {
    pub account: String,
    /// Sorted by name; `id` is the numeric Apple ID.
    pub apps: Vec<arrangia_publish::apple::AppSummary>,
    /// Answered from this session's cache (no network).
    pub cached: bool,
}

/// The apps an Apple key can see (read-only `GET /v1/apps`, all pages).
/// Cached per account for the app session; `refresh` asks again. Errors
/// like the connection test (`io` with `details.reason`).
pub fn store_apps_list(app: &AppState, args: StoreAppsListArgs) -> CmdResult<StoreAppsList> {
    if !args.refresh
        && let Some(apps) = app.cached_store_apps(&args.account)
    {
        return Ok(StoreAppsList {
            account: args.account,
            apps,
            cached: true,
        });
    }
    let (account, secret) = account_with_secret(app, &args.account, StoreKind::Apple)?;
    let config = app.upload().config();
    let apps = crate::upload::remote::block_on(async {
        let client =
            crate::upload::remote::apple_client(&config, &account, &secret, interactive_options())?;
        client.list_apps().await
    })?
    .map_err(|e| store_failure(StoreKind::Apple, &e))?;
    app.cache_store_apps(&account.id, Some(apps.clone()));
    Ok(StoreAppsList {
        account: account.id,
        apps,
        cached: false,
    })
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StorePackageCheckArgs {
    /// Google account id.
    pub account: String,
    pub package_name: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct StorePackageCheck {
    pub package_name: String,
    /// Title of the default listing, if Play returned one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_language: Option<String>,
}

/// `com.example.app`: two or more segments of letters, digits and `_`,
/// each starting with a letter.
pub fn is_valid_package_name(name: &str) -> bool {
    let parts: Vec<&str> = name.split('.').collect();
    parts.len() >= 2
        && parts.iter().all(|p| {
            p.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
                && p.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        })
}

/// Checks a Play package with the account's service account. Google has no
/// "list my apps" call and no edit-free read covered by the store-listing
/// permission, so this opens a temporary edit, reads the app details and
/// deletes the edit again (never committed – nothing changes in Play).
/// The GUI calls it only on the user's "Check" click.
pub fn store_package_check(
    app: &AppState,
    args: StorePackageCheckArgs,
) -> CmdResult<StorePackageCheck> {
    let package = args.package_name.trim().to_owned();
    if !is_valid_package_name(&package) {
        return Err(CommandError::invalid(format!(
            "`{package}` is not a package name (like com.example.app)"
        )));
    }
    let (_, secret) = account_with_secret(app, &args.account, StoreKind::Google)?;
    let config = app.upload().config();
    let info = crate::upload::remote::block_on(async {
        let client = crate::upload::remote::google_client(
            &config,
            &secret,
            &package,
            interactive_options(),
        )?;
        client.check_package().await
    })?
    .map_err(|e| store_failure(StoreKind::Google, &e))?;
    Ok(StorePackageCheck {
        package_name: package,
        title: info.title,
        default_language: info.default_language,
    })
}

// ----- project mapping ----------------------------------------------------------------

/// Effective store locale of one project language.
#[derive(Debug, Clone, Serialize)]
pub struct EffectiveLocale {
    pub lang: String,
    /// `None` = no store locale (not uploaded).
    pub locale: Option<String>,
    pub source: Option<LocaleSource>,
    /// The built-in default (shown next to an explicit entry).
    pub default: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct StoreLocaleLists {
    pub apple: &'static [StoreLocale],
    pub google: &'static [StoreLocale],
    /// Date the lists were verified.
    pub checked: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct EffectiveLocales {
    pub apple: Vec<EffectiveLocale>,
    pub google: Vec<EffectiveLocale>,
}

/// An account reference that does not resolve.
#[derive(Debug, Clone, Serialize)]
pub struct MissingAccount {
    pub store: StoreKind,
    pub account: String,
    /// `unknown` = no such account, `wrong_kind` = an account of the other store.
    pub problem: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProjectStoreInfo {
    pub store: StoreSettings,
    pub effective: EffectiveLocales,
    /// All store locale codes (`project_store_get` only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub locales: Option<StoreLocaleLists>,
    /// Account choices (no secrets).
    pub accounts: Vec<StoreAccount>,
    pub missing_accounts: Vec<MissingAccount>,
}

fn effective(project: &Project, store: StoreKind) -> Vec<EffectiveLocale> {
    let list = catalog::store_locales().store(store);
    let explicit = project.store.locales.get(store);
    project
        .languages
        .available
        .iter()
        .map(|l| {
            let lang = l.as_str();
            let resolved = list.resolve(explicit, lang);
            EffectiveLocale {
                lang: lang.to_owned(),
                locale: resolved.map(|(c, _)| c.to_owned()),
                source: resolved.map(|(_, s)| s),
                default: list.default_for(lang).map(|(c, _)| c.to_owned()),
            }
        })
        .collect()
}

fn project_store_info(
    project: &Project,
    accounts: Vec<StoreAccount>,
    with_lists: bool,
) -> ProjectStoreInfo {
    let lists = catalog::store_locales();
    let mut missing = Vec::new();
    for kind in StoreKind::ALL {
        if let Some(id) = project.store.account(kind) {
            let problem = match accounts.iter().find(|a| a.id == id) {
                None => Some("unknown"),
                Some(a) if a.kind != kind => Some("wrong_kind"),
                Some(_) => None,
            };
            if let Some(problem) = problem {
                missing.push(MissingAccount {
                    store: kind,
                    account: id.to_owned(),
                    problem,
                });
            }
        }
    }
    ProjectStoreInfo {
        store: project.store.clone(),
        effective: EffectiveLocales {
            apple: effective(project, StoreKind::Apple),
            google: effective(project, StoreKind::Google),
        },
        locales: with_lists.then(|| StoreLocaleLists {
            apple: &lists.apple.locales,
            google: &lists.google.locales,
            checked: &lists.checked,
        }),
        accounts,
        missing_accounts: missing,
    }
}

/// The project's store mapping with effective locales and account choices.
pub fn project_store_get(app: &AppState, _: NoArgs) -> CmdResult<ProjectStoreInfo> {
    let session = app.session()?;
    let accounts = app.settings_store().load()?.store_accounts;
    Ok(session.with_project(|p| project_store_info(p, accounts, true)))
}

/// Patch of the store mapping: a present store object replaces that
/// store's settings (`null` removes the store); `locales.<store>` replaces
/// that map (empty values are dropped = back to the default).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(clippy::option_option)]
pub struct ProjectStorePatchArgs {
    #[serde(default, deserialize_with = "double_option")]
    pub apple: Option<Option<AppleStore>>,
    #[serde(default, deserialize_with = "double_option")]
    pub google: Option<Option<GoogleStore>>,
    #[serde(default)]
    pub locales: Option<LocalesPatch>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LocalesPatch {
    #[serde(default)]
    pub apple: Option<BTreeMap<String, String>>,
    #[serde(default)]
    pub google: Option<BTreeMap<String, String>>,
}

fn clean_map(map: BTreeMap<String, String>) -> BTreeMap<String, String> {
    map.into_iter()
        .map(|(k, v)| (k.trim().to_owned(), v.trim().to_owned()))
        .filter(|(k, v)| !k.is_empty() && !v.is_empty())
        .collect()
}

/// Empty strings in the store objects mean "not set".
fn clean_apple(mut a: AppleStore) -> AppleStore {
    for f in [&mut a.account, &mut a.app_id, &mut a.bundle_id] {
        if f.as_deref().is_some_and(|v| v.trim().is_empty()) {
            *f = None;
        } else if let Some(v) = f {
            *v = v.trim().to_owned();
        }
    }
    a
}

fn clean_google(mut g: GoogleStore) -> GoogleStore {
    for f in [&mut g.account, &mut g.package_name] {
        if f.as_deref().is_some_and(|v| v.trim().is_empty()) {
            *f = None;
        } else if let Some(v) = f {
            *v = v.trim().to_owned();
        }
    }
    g
}

pub fn project_store_patch(app: &AppState, args: ProjectStorePatchArgs) -> CmdResult<ApplyResult> {
    let session = app.session()?;
    let mut store = session.with_project(|p| p.store.clone());
    if let Some(a) = args.apple {
        store.apple = a.map(clean_apple);
    }
    if let Some(g) = args.google {
        store.google = g.map(clean_google);
    }
    if let Some(l) = args.locales {
        if let Some(m) = l.apple {
            store.locales.apple = clean_map(m);
        }
        if let Some(m) = l.google {
            store.locales.google = clean_map(m);
        }
    }
    if session.with_project(|p| p.store == store) {
        return Ok(session.unchanged());
    }
    session.apply_ops(
        vec![Op::SetStore { store }],
        Some("Edit store settings".into()),
    )
}

// ----- read-only overview (MCP) -------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct StoreConfig {
    /// Accounts without secrets.
    pub accounts: Vec<StoreAccount>,
    pub keychain: KeychainInfo,
    /// The open project's mapping (`None` without a project).
    pub project: Option<ProjectStoreInfo>,
}

/// Accounts plus the open project's mapping; never any secret.
pub fn store_config_get(app: &AppState, _: NoArgs) -> CmdResult<StoreConfig> {
    let accounts = app.settings_store().load()?.store_accounts;
    let project = app
        .session()
        .ok()
        .map(|s| s.with_project(|p| project_store_info(p, accounts.clone(), false)));
    Ok(StoreConfig {
        accounts,
        keychain: keychain_info(app),
        project,
    })
}
