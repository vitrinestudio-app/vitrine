//! App Store Connect API client. See ARCHITECTURE.md §10.
//!
//! Shared calls: [`Client::find_app`] → [`Client::editable_version`] →
//! [`Client::localizations`] / [`Client::create_localization`]. Screenshots
//! and creative assets go through the App Asset Library ([`library`]:
//! asset library, reference data, images, placements, ordering requests);
//! the deprecated screenshot-set resources (`appScreenshotSets`,
//! `appScreenshots`) are no longer used (decision 0034).

pub mod library;
mod model;

use std::fmt;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use bytes::Bytes;
use reqwest::{Method, StatusCode, Url};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use tokio::sync::Mutex;
use tokio::task::JoinSet;
use tokio::time::Instant;
use zeroize::Zeroizing;

pub use library::{
    AssetLibrary, LibraryImage, LibraryUpload, Placement, RefData, RefDataFilter, RefGroup,
    RefSpec, is_asset_has_placements,
};
pub use model::{
    App, AppRef, AppStoreVersion, AppSummary, AppsOverview, HttpHeader, Localization, Platform,
    UploadOperation,
};
use model::{AppAttrs, Document, ErrorResponse, LocalizationAttrs, Resource, VersionAttrs};

use crate::error::{ApiErrorDetail, Error, Result};
use crate::http::{ClientOptions, Http, Reply, Retry, join_segments, parse_base_url, status_error};
use crate::progress::{Progress, ProgressFn, UploadStep};

/// Production API host.
pub const DEFAULT_BASE_URL: &str = "https://api.appstoreconnect.apple.com";
/// JWT audience required by App Store Connect.
pub const AUDIENCE: &str = "appstoreconnect-v1";
/// Apple rejects tokens living longer than 20 minutes (for these resources).
pub const MAX_TOKEN_LIFETIME: Duration = Duration::from_mins(20);

/// `appVersionState` values in which screenshots can be edited.
///
/// Apple publishes no explicit list; these are the states in which the
/// version page in App Store Connect is editable (ADR 0017).
pub const EDITABLE_STATES: &[&str] = &[
    "PREPARE_FOR_SUBMISSION",
    "DEVELOPER_REJECTED",
    "REJECTED",
    "METADATA_REJECTED",
    "INVALID_BINARY",
];

/// Credentials and endpoint. The private key is held in memory only (the
/// caller loads it from the keychain) and wiped on drop.
#[derive(Clone)]
pub struct Config {
    /// Issuer id (team keys). `None` for an individual key (`sub: "user"`).
    pub issuer_id: Option<String>,
    /// Key id (`kid`).
    pub key_id: String,
    /// Contents of the `.p8` file (PKCS#8 PEM, EC P-256).
    pub private_key_pem: Zeroizing<String>,
    /// API base URL (overridable for tests).
    pub base_url: String,
    /// Token lifetime (clamped to [`MAX_TOKEN_LIFETIME`]).
    pub token_lifetime: Duration,
}

impl Config {
    /// Team key configuration against the production API.
    pub fn team(
        issuer_id: impl Into<String>,
        key_id: impl Into<String>,
        private_key_pem: impl Into<String>,
    ) -> Self {
        Self {
            issuer_id: Some(issuer_id.into()),
            key_id: key_id.into(),
            private_key_pem: Zeroizing::new(private_key_pem.into()),
            base_url: DEFAULT_BASE_URL.to_owned(),
            token_lifetime: Duration::from_mins(15),
        }
    }

    /// Individual key configuration (no issuer id).
    pub fn individual(key_id: impl Into<String>, private_key_pem: impl Into<String>) -> Self {
        Self {
            issuer_id: None,
            ..Self::team(String::new(), key_id, private_key_pem)
        }
    }

    /// Replaces the base URL (tests, proxies).
    #[must_use]
    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = base_url.into();
        self
    }
}

impl fmt::Debug for Config {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Config")
            .field("issuer_id", &self.issuer_id)
            .field("key_id", &self.key_id)
            .field("private_key_pem", &"<redacted>")
            .field("base_url", &self.base_url)
            .field("token_lifetime", &self.token_lifetime)
            .finish()
    }
}

#[derive(Serialize)]
struct Claims<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    iss: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sub: Option<&'a str>,
    iat: u64,
    exp: u64,
    aud: &'a str,
}

struct CachedToken {
    token: Zeroizing<String>,
    refresh_at: Instant,
}

/// App Store Connect client.
pub struct Client {
    config: Config,
    base: Url,
    http: Http,
    token: Mutex<Option<CachedToken>>,
}

impl fmt::Debug for Client {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Client")
            .field("config", &self.config)
            .field("http", &self.http)
            .finish_non_exhaustive()
    }
}

impl Client {
    /// Creates a client. Fails early if the key cannot be parsed.
    pub fn new(config: Config, options: ClientOptions) -> Result<Self> {
        let base = parse_base_url(&config.base_url)?;
        if config.key_id.trim().is_empty() {
            return Err(Error::config("App Store Connect key id is empty"));
        }
        if config
            .issuer_id
            .as_deref()
            .is_some_and(|s| s.trim().is_empty())
        {
            return Err(Error::config("App Store Connect issuer id is empty"));
        }
        jsonwebtoken::EncodingKey::from_ec_pem(config.private_key_pem.as_bytes()).map_err(
            |_| {
                Error::config(
                    "the App Store Connect key is not a valid PKCS#8 EC (.p8) private key",
                )
            },
        )?;
        Ok(Self {
            config,
            base,
            http: Http::new(options)?,
            token: Mutex::new(None),
        })
    }

    /// The options in use (e.g. to reach the cancellation token).
    pub fn options(&self) -> &ClientOptions {
        &self.http.opts
    }

    // ---- auth --------------------------------------------------------------

    fn sign(&self) -> Result<(Zeroizing<String>, Duration)> {
        let lifetime = self.config.token_lifetime.min(MAX_TOKEN_LIFETIME);
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| Error::config("system clock before 1970"))?
            .as_secs();
        let claims = Claims {
            iss: self.config.issuer_id.as_deref(),
            sub: self.config.issuer_id.is_none().then_some("user"),
            iat: now,
            exp: now + lifetime.as_secs(),
            aud: AUDIENCE,
        };
        let mut header = jsonwebtoken::Header::new(jsonwebtoken::Algorithm::ES256);
        header.kid = Some(self.config.key_id.clone());
        header.typ = Some("JWT".into());
        let key = jsonwebtoken::EncodingKey::from_ec_pem(self.config.private_key_pem.as_bytes())
            .map_err(|_| Error::config("invalid App Store Connect key"))?;
        let token = jsonwebtoken::encode(&header, &claims, &key)
            .map_err(|_| Error::config("signing the App Store Connect token failed"))?;
        Ok((Zeroizing::new(token), lifetime))
    }

    async fn bearer(&self, force_new: bool) -> Result<Zeroizing<String>> {
        let mut guard = self.token.lock().await;
        if !force_new
            && let Some(cached) = guard.as_ref()
            && Instant::now() < cached.refresh_at
        {
            return Ok(cached.token.clone());
        }
        let (token, lifetime) = self.sign()?;
        // Renew one minute before expiry (clock skew, long requests).
        let margin = Duration::from_secs(60).min(lifetime / 2);
        *guard = Some(CachedToken {
            token: token.clone(),
            refresh_at: Instant::now() + lifetime - margin,
        });
        Ok(token)
    }

    // ---- request helpers ---------------------------------------------------

    fn url(&self, segments: &[&str]) -> Result<Url> {
        join_segments(&self.base, segments)
    }

    /// Only follow `links.next` URLs on our own API host (the bearer token
    /// must never go elsewhere).
    fn check_own_url(&self, url: &Url) -> Result<()> {
        if url.origin() == self.base.origin() && url.path().starts_with(self.base.path()) {
            Ok(())
        } else {
            Err(Error::Transport {
                status: None,
                message: "pagination link points to a foreign host".into(),
            })
        }
    }

    /// Authenticated API call; on HTTP 401 the token is re-signed once.
    async fn api(
        &self,
        method: Method,
        url: Url,
        body: Option<Value>,
        retry: Retry,
    ) -> Result<Reply> {
        let mut force_new = false;
        loop {
            let token = self.bearer(force_new).await?;
            let build = || {
                let mut rb = self
                    .http
                    .client
                    .request(method.clone(), url.clone())
                    .bearer_auth(token.as_str())
                    .header(reqwest::header::ACCEPT, "application/json");
                if let Some(body) = &body {
                    rb = rb.json(body);
                }
                rb
            };
            match self.http.send(retry, &build, map_error).await {
                Err(e) if e.is_unauthorized() && !force_new => force_new = true,
                other => return other,
            }
        }
    }

    async fn get_doc<T: DeserializeOwned>(&self, url: Url) -> Result<Document<T>> {
        let reply = self.api(Method::GET, url, None, Retry::Idempotent).await?;
        parse_json(&reply)
    }

    /// GET a collection and follow `links.next`.
    async fn get_all<A: DeserializeOwned>(&self, mut url: Url) -> Result<Vec<Resource<A>>> {
        let mut out = Vec::new();
        loop {
            let doc: Document<Vec<Resource<A>>> = self.get_doc(url).await?;
            out.extend(doc.data);
            match doc.links.and_then(|l| l.next) {
                Some(next) => {
                    let next = Url::parse(&next).map_err(|_| Error::Transport {
                        status: None,
                        message: "invalid pagination link".into(),
                    })?;
                    self.check_own_url(&next)?;
                    url = next;
                }
                None => return Ok(out),
            }
        }
    }

    // ---- resources ---------------------------------------------------------

    /// Cheap read-only call for a connection test: `GET /v1/apps?limit=1`.
    /// Proves the key is accepted and returns how many apps it can see.
    pub async fn apps_overview(&self) -> Result<AppsOverview> {
        let mut url = self.url(&["v1", "apps"])?;
        url.query_pairs_mut()
            .append_pair("fields[apps]", "bundleId,name")
            .append_pair("limit", "1");
        let doc: Document<Vec<Resource<AppAttrs>>> = self.get_doc(url).await?;
        let total = doc
            .meta
            .and_then(|m| m.paging)
            .and_then(|p| p.total)
            .or_else(|| doc.data.is_empty().then_some(0));
        Ok(AppsOverview {
            total,
            first: doc.data.into_iter().next().map(App::from),
        })
    }

    /// Every app the key can see (read-only; follows `links.next`), sorted
    /// by name: id (= Apple ID), name, bundle id, SKU, primary locale. Used
    /// by the app picker in the project settings.
    pub async fn list_apps(&self) -> Result<Vec<AppSummary>> {
        let mut url = self.url(&["v1", "apps"])?;
        url.query_pairs_mut()
            .append_pair("fields[apps]", "name,bundleId,sku,primaryLocale")
            .append_pair("sort", "name")
            .append_pair("limit", "200");
        let mut apps: Vec<AppSummary> = self
            .get_all::<AppAttrs>(url)
            .await?
            .into_iter()
            .map(AppSummary::from)
            .collect();
        apps.sort_by(|a, b| {
            a.name
                .to_lowercase()
                .cmp(&b.name.to_lowercase())
                .then_with(|| a.bundle_id.cmp(&b.bundle_id))
        });
        Ok(apps)
    }

    /// Finds the app by bundle id or Apple id.
    pub async fn find_app(&self, app: &AppRef) -> Result<App> {
        match app {
            AppRef::Id(id) => {
                let doc: Document<Resource<AppAttrs>> =
                    self.get_doc(self.url(&["v1", "apps", id])?).await?;
                Ok(doc.data.into())
            }
            AppRef::BundleId(bundle_id) => {
                let mut url = self.url(&["v1", "apps"])?;
                url.query_pairs_mut()
                    .append_pair("filter[bundleId]", bundle_id)
                    .append_pair("limit", "200");
                let apps = self.get_all::<AppAttrs>(url).await?;
                apps.into_iter()
                    .map(App::from)
                    .find(|a| a.bundle_id == *bundle_id)
                    .ok_or_else(|| Error::NotFound {
                        message: format!("no app with bundle id {bundle_id} visible to this key"),
                    })
            }
        }
    }

    /// All App Store versions of the app on `platform`.
    pub async fn versions(&self, app_id: &str, platform: Platform) -> Result<Vec<AppStoreVersion>> {
        let mut url = self.url(&["v1", "apps", app_id, "appStoreVersions"])?;
        url.query_pairs_mut()
            .append_pair("filter[platform]", platform.as_str())
            .append_pair("limit", "200");
        Ok(self
            .get_all::<VersionAttrs>(url)
            .await?
            .into_iter()
            .map(AppStoreVersion::from)
            .collect())
    }

    /// The version whose screenshots can be edited (see [`EDITABLE_STATES`]).
    /// Fails with [`Error::Conflict`] if there is none.
    pub async fn editable_version(
        &self,
        app_id: &str,
        platform: Platform,
    ) -> Result<AppStoreVersion> {
        let versions = self.versions(app_id, platform).await?;
        if let Some(v) = versions.iter().find(|v| {
            v.state
                .as_deref()
                .is_some_and(|s| EDITABLE_STATES.contains(&s))
        }) {
            return Ok(v.clone());
        }
        Err(no_editable_version_error(platform, &versions))
    }

    /// Localizations of a version.
    pub async fn localizations(&self, version_id: &str) -> Result<Vec<Localization>> {
        let mut url = self.url(&[
            "v1",
            "appStoreVersions",
            version_id,
            "appStoreVersionLocalizations",
        ])?;
        url.query_pairs_mut().append_pair("limit", "200");
        Ok(self
            .get_all::<LocalizationAttrs>(url)
            .await?
            .into_iter()
            .map(Localization::from)
            .collect())
    }

    /// Creates the version localization for `locale` (`POST
    /// /v1/appStoreVersionLocalizations`, 201). Apple also expects the same
    /// locale in the app info localizations before submission; that is
    /// left to the user (App Store Connect shows it).
    pub async fn create_localization(
        &self,
        version_id: &str,
        locale: &str,
    ) -> Result<Localization> {
        let body = json!({
            "data": {
                "type": "appStoreVersionLocalizations",
                "attributes": { "locale": locale },
                "relationships": {
                    "appStoreVersion": { "data": { "type": "appStoreVersions", "id": version_id } }
                }
            }
        });
        let reply = self
            .api(
                Method::POST,
                self.url(&["v1", "appStoreVersionLocalizations"])?,
                Some(body),
                Retry::NotIdempotent,
            )
            .await?;
        let doc: Document<Resource<LocalizationAttrs>> = parse_json(&reply)?;
        Ok(doc.data.into())
    }

    // ---- upload ------------------------------------------------------------

    /// Sends every part of a reservation to Apple's pre-signed URLs
    /// (concurrently and in any order, as Apple allows), reporting progress
    /// per part. Shared by screenshots and asset library images.
    async fn transfer_parts(
        &self,
        ops: &[UploadOperation],
        bytes: &Bytes,
        p: &mut Progress,
        progress: ProgressFn<'_>,
    ) -> Result<()> {
        let total = bytes.len() as u64;
        if ops.is_empty() {
            return Err(Error::Validation {
                message: "reservation returned no upload operations".into(),
                details: Vec::new(),
            });
        }
        for op in ops {
            let end = op.offset.checked_add(op.length);
            if end.is_none_or(|end| end > total) {
                return Err(Error::Validation {
                    message: format!(
                        "upload operation out of range (offset {}, length {}, file {total} bytes)",
                        op.offset, op.length
                    ),
                    details: Vec::new(),
                });
            }
        }

        p.step = UploadStep::Transfer;
        progress(p);
        let concurrency = self.http.opts.part_concurrency.max(1);
        let mut pending = ops.iter().cloned();
        let mut running = JoinSet::new();
        let mut done = 0usize;
        let mut first_error = None;
        loop {
            while first_error.is_none() && running.len() < concurrency {
                let Some(op) = pending.next() else { break };
                let http = self.http.clone();
                let part = bytes.slice(op.offset as usize..(op.offset + op.length) as usize);
                running
                    .spawn(async move { upload_part(&http, &op, part).await.map(|()| op.length) });
            }
            let Some(joined) = running.join_next().await else {
                break;
            };
            match joined {
                Ok(Ok(len)) => {
                    done += 1;
                    p.bytes_sent += len;
                    p.parts = Some((done, ops.len()));
                    progress(p);
                }
                Ok(Err(e)) => {
                    first_error.get_or_insert(e);
                }
                Err(join) => {
                    first_error.get_or_insert(Error::Transport {
                        status: None,
                        message: format!("upload task failed: {join}"),
                    });
                }
            }
        }
        if let Some(e) = first_error {
            return Err(e);
        }
        self.http.check_cancelled()
    }
}

/// `code` of the first detail of the "no editable version" conflict.
pub const NO_EDITABLE_VERSION: &str = "NO_EDITABLE_VERSION";

/// Facts of a "no editable App Store version" conflict (see
/// [`no_editable_version`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoEditableVersion {
    /// Platform (`IOS`, `MAC_OS`).
    pub platform: String,
    /// Highest existing version and its raw state.
    pub latest: Option<(String, String)>,
    /// Suggested next version (`2.1.0` → `2.1.1`).
    pub suggestion: Option<String>,
}

/// Recognises the conflict [`Client::editable_version`] returns when no
/// version can be edited; the full version list is in the error details.
pub fn no_editable_version(e: &Error) -> Option<NoEditableVersion> {
    let Error::Conflict { details, .. } = e else {
        return None;
    };
    let first = details.first().filter(|d| d.code == NO_EDITABLE_VERSION)?;
    let latest = details
        .iter()
        .skip(1)
        .find(|d| d.title == first.detail)
        .map(|d| (d.title.clone(), d.detail.clone()));
    Some(NoEditableVersion {
        platform: first.title.clone(),
        suggestion: latest.as_ref().and_then(|(v, _)| next_version(v)),
        latest,
    })
}

/// Human name of an `appVersionState` (`READY_FOR_DISTRIBUTION` →
/// `Ready for Distribution`).
pub fn version_state_label(state: &str) -> String {
    state
        .split('_')
        .filter(|w| !w.is_empty())
        .enumerate()
        .map(|(i, w)| {
            let w = w.to_ascii_lowercase();
            if i > 0 && matches!(w.as_str(), "for" | "of" | "in" | "to" | "and" | "by") {
                w
            } else {
                let mut c = w.chars();
                c.next()
                    .map(|f| f.to_ascii_uppercase().to_string() + c.as_str())
                    .unwrap_or_default()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn version_key(v: &str) -> Vec<u64> {
    v.split('.')
        .map(|p| p.trim().parse().unwrap_or(0))
        .collect()
}

/// `2.1.0` → `2.1.1`, `3` → `3.0.1`.
fn next_version(v: &str) -> Option<String> {
    let mut parts = version_key(v);
    if parts.is_empty() || v.trim().is_empty() {
        return None;
    }
    while parts.len() < 3 {
        parts.push(0);
    }
    if let Some(last) = parts.last_mut() {
        *last += 1;
    }
    Some(
        parts
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("."),
    )
}

/// The short conflict (one line; the version list goes to `details`).
fn no_editable_version_error(platform: Platform, versions: &[AppStoreVersion]) -> Error {
    let latest = versions
        .iter()
        .max_by(|a, b| version_key(&a.version_string).cmp(&version_key(&b.version_string)));
    let mut details = vec![ApiErrorDetail {
        code: NO_EDITABLE_VERSION.to_owned(),
        title: platform.as_str().to_owned(),
        detail: latest.map(|v| v.version_string.clone()).unwrap_or_default(),
    }];
    details.extend(versions.iter().map(|v| ApiErrorDetail {
        code: "APP_STORE_VERSION".to_owned(),
        title: v.version_string.clone(),
        detail: v.state.clone().unwrap_or_default(),
    }));
    let message = match latest {
        Some(v) => format!(
            "No editable App Store version. Create a new version{} in App Store Connect, then plan again. Latest version: {} ({}).",
            next_version(&v.version_string)
                .map(|n| format!(" (e.g. {n})"))
                .unwrap_or_default(),
            v.version_string,
            version_state_label(v.state.as_deref().unwrap_or("unknown"))
        ),
        None => "No App Store version yet. Create a version in App Store Connect, then plan again."
            .to_owned(),
    };
    Error::Conflict { message, details }
}

/// Sends one upload part to the pre-signed URL (no JWT).
async fn upload_part(http: &Http, op: &UploadOperation, part: Bytes) -> Result<()> {
    let method = Method::from_bytes(op.method.as_bytes()).map_err(|_| Error::Validation {
        message: format!("unsupported upload method {}", op.method),
        details: Vec::new(),
    })?;
    let url = Url::parse(&op.url).map_err(|_| Error::Validation {
        message: "invalid upload URL in reservation".into(),
        details: Vec::new(),
    })?;
    let build = || {
        let mut rb = http.client.request(method.clone(), url.clone());
        for h in &op.request_headers {
            rb = rb.header(h.name.as_str(), h.value.as_str());
        }
        rb.body(part.clone())
    };
    http.send(Retry::Idempotent, &build, |status, _| {
        status_error(
            status,
            format!(
                "upload part rejected by the storage server (HTTP {})",
                status.as_u16()
            ),
        )
        .into_part_error()
    })
    .await?;
    Ok(())
}

impl Error {
    /// Storage-server rejections are not credential problems (the upload URL
    /// carries its own signature): report 401/403 as validation errors.
    fn into_part_error(self) -> Self {
        match self {
            Self::Auth { message, .. } => Self::Validation {
                message,
                details: Vec::new(),
            },
            other => other,
        }
    }
}

fn parse_json<T: DeserializeOwned>(reply: &Reply) -> Result<T> {
    serde_json::from_slice(&reply.body).map_err(|e| Error::Transport {
        status: Some(reply.status.as_u16()),
        message: format!("unexpected App Store Connect response: {e}"),
    })
}

/// Maps an App Store Connect error response.
fn map_error(status: StatusCode, body: &[u8]) -> Error {
    let details: Vec<ApiErrorDetail> = serde_json::from_slice::<ErrorResponse>(body)
        .map(|r| r.errors.into_iter().map(ApiErrorDetail::from).collect())
        .unwrap_or_default();
    let message = details.first().map_or_else(
        || format!("HTTP {}", status.as_u16()),
        |d| {
            if d.detail.is_empty() {
                d.title.clone()
            } else {
                d.detail.clone()
            }
        },
    );
    let is_state = details.iter().any(|d| d.code.starts_with("STATE_ERROR"));
    match status.as_u16() {
        409 if is_state => Error::Conflict { message, details },
        400 | 409 | 422 => Error::Validation { message, details },
        _ => status_error(status, message),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_errors_map_to_conflict() {
        let body = br#"{"errors":[{"status":"409","code":"STATE_ERROR.SCREENSHOT_UPLOADS_IN_PROGRESS","title":"t","detail":"d"}]}"#;
        assert!(matches!(
            map_error(StatusCode::CONFLICT, body),
            Error::Conflict { .. }
        ));
        let body = br#"{"errors":[{"status":"409","code":"ENTITY_ERROR.ATTRIBUTE.INVALID","title":"t","detail":"bad size"}]}"#;
        let e = map_error(StatusCode::CONFLICT, body);
        assert!(matches!(e, Error::Validation { ref message, .. } if message == "bad size"));
        assert_eq!(e.details()[0].code, "ENTITY_ERROR.ATTRIBUTE.INVALID");
    }

    #[test]
    fn version_helpers() {
        assert_eq!(
            version_state_label("READY_FOR_DISTRIBUTION"),
            "Ready for Distribution"
        );
        assert_eq!(
            version_state_label("PREPARE_FOR_SUBMISSION"),
            "Prepare for Submission"
        );
        assert_eq!(next_version("2.1.0").as_deref(), Some("2.1.1"));
        assert_eq!(next_version("3").as_deref(), Some("3.0.1"));
        assert_eq!(next_version(""), None);
        let v = |s: &str, st: &str| AppStoreVersion {
            id: s.into(),
            platform: "IOS".into(),
            version_string: s.into(),
            state: Some(st.into()),
        };
        let e = no_editable_version_error(
            Platform::Ios,
            &[
                v("2.0.10", "READY_FOR_DISTRIBUTION"),
                v("2.0.9", "READY_FOR_DISTRIBUTION"),
            ],
        );
        let info = no_editable_version(&e).unwrap();
        assert_eq!(info.latest.unwrap().0, "2.0.10");
        assert_eq!(info.suggestion.as_deref(), Some("2.0.11"));
        let none = no_editable_version_error(Platform::Ios, &[]);
        assert!(no_editable_version(&none).unwrap().latest.is_none());
    }

    #[test]
    fn config_debug_hides_key() {
        let c = Config::team("iss", "kid", "-----BEGIN PRIVATE KEY-----SECRET");
        assert!(!format!("{c:?}").contains("SECRET"));
    }
}
