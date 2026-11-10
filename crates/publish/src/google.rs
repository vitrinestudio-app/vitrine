//! Google Play Android Publisher API v3 client (store listing images).
//! See ADR 0017.
//!
//! Flow: [`Client::insert_edit`] → per language and image type
//! [`Client::delete_all`] → [`Client::upload_image`] in display order →
//! [`Client::commit`]; on abort [`Client::delete_edit`].

use std::fmt;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use bytes::Bytes;
use reqwest::{Method, StatusCode, Url};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use tokio::time::Instant;
use zeroize::Zeroizing;

use crate::error::{ApiErrorDetail, Error, Result};
use crate::http::{ClientOptions, Http, Reply, Retry, join_segments, parse_base_url, status_error};
use crate::progress::{Progress, ProgressFn, UploadStep};

/// Production API host.
pub const DEFAULT_BASE_URL: &str = "https://androidpublisher.googleapis.com";
/// OAuth 2.0 token endpoint; also the required JWT `aud`.
pub const DEFAULT_TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
/// OAuth scope of the Android Publisher API.
pub const SCOPE: &str = "https://www.googleapis.com/auth/androidpublisher";
/// Maximum image upload size (`mediaUpload.maxSize` in the discovery
/// document: 15 MiB).
pub const MAX_IMAGE_BYTES: usize = 15 * 1024 * 1024;
/// Assertion lifetime (Google allows at most one hour).
const ASSERTION_LIFETIME: Duration = Duration::from_hours(1);

/// Store listing image type (`AppImageType`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ImageType {
    /// `phoneScreenshots`
    PhoneScreenshots,
    /// `sevenInchScreenshots`
    SevenInchScreenshots,
    /// `tenInchScreenshots`
    TenInchScreenshots,
    /// `tvScreenshots`
    TvScreenshots,
    /// `wearScreenshots`
    WearScreenshots,
    /// `icon`
    Icon,
    /// `featureGraphic`
    FeatureGraphic,
    /// `tvBanner`
    TvBanner,
}

impl ImageType {
    /// All types.
    pub const ALL: [Self; 8] = [
        Self::PhoneScreenshots,
        Self::SevenInchScreenshots,
        Self::TenInchScreenshots,
        Self::TvScreenshots,
        Self::WearScreenshots,
        Self::Icon,
        Self::FeatureGraphic,
        Self::TvBanner,
    ];

    /// API value.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::PhoneScreenshots => "phoneScreenshots",
            Self::SevenInchScreenshots => "sevenInchScreenshots",
            Self::TenInchScreenshots => "tenInchScreenshots",
            Self::TvScreenshots => "tvScreenshots",
            Self::WearScreenshots => "wearScreenshots",
            Self::Icon => "icon",
            Self::FeatureGraphic => "featureGraphic",
            Self::TvBanner => "tvBanner",
        }
    }

    /// Parses the API value (as used in `targets.yaml` slots).
    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|t| t.as_str() == s)
    }
}

/// An edit (`AppEdit`).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Edit {
    /// Edit id.
    pub id: String,
    /// Expiry (seconds since epoch, as string).
    #[serde(default)]
    pub expiry_time_seconds: Option<String>,
}

/// A listing image (`Image`).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Image {
    /// Image id.
    pub id: String,
    /// Preview URL.
    #[serde(default)]
    pub url: Option<String>,
    /// SHA-1 (hex).
    #[serde(default)]
    pub sha1: Option<String>,
    /// SHA-256 (hex).
    #[serde(default)]
    pub sha256: Option<String>,
}

/// Credentials and endpoints. The service account JSON is held in memory
/// only (the caller loads it from the keychain) and wiped on drop.
#[derive(Clone)]
pub struct Config {
    /// Service account key file contents (JSON).
    pub service_account_json: Zeroizing<String>,
    /// Package name (`com.example.app`).
    pub package_name: String,
    /// API base URL (overridable for tests).
    pub base_url: String,
    /// Token endpoint (overridable for tests). The `token_uri` from the JSON
    /// is deliberately ignored so a pasted file cannot redirect assertions.
    pub token_url: String,
}

impl Config {
    /// Configuration against the production endpoints.
    pub fn new(service_account_json: impl Into<String>, package_name: impl Into<String>) -> Self {
        Self {
            service_account_json: Zeroizing::new(service_account_json.into()),
            package_name: package_name.into(),
            base_url: DEFAULT_BASE_URL.to_owned(),
            token_url: DEFAULT_TOKEN_URL.to_owned(),
        }
    }

    /// Replaces both endpoints (tests).
    #[must_use]
    pub fn with_endpoints(
        mut self,
        base_url: impl Into<String>,
        token_url: impl Into<String>,
    ) -> Self {
        self.base_url = base_url.into();
        self.token_url = token_url.into();
        self
    }
}

impl fmt::Debug for Config {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Config")
            .field("service_account_json", &"<redacted>")
            .field("package_name", &self.package_name)
            .field("base_url", &self.base_url)
            .field("token_url", &self.token_url)
            .finish()
    }
}

#[derive(Deserialize)]
struct ServiceAccountFile {
    #[serde(rename = "type", default)]
    kind: String,
    client_email: String,
    private_key: Zeroizing<String>,
    #[serde(default)]
    private_key_id: Option<String>,
}

#[derive(Serialize)]
struct Claims<'a> {
    iss: &'a str,
    scope: &'a str,
    aud: &'a str,
    iat: u64,
    exp: u64,
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: Zeroizing<String>,
    #[serde(default)]
    expires_in: Option<u64>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct OAuthError {
    error: String,
    error_description: String,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct GoogleErrorBody {
    error: GoogleError,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct GoogleError {
    message: String,
    status: String,
    errors: Vec<GoogleErrorItem>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct GoogleErrorItem {
    reason: String,
    message: String,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct ImagesList {
    images: Vec<Image>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct DeletedImages {
    deleted: Vec<Image>,
}

#[derive(Deserialize)]
struct UploadedImage {
    image: Image,
}

struct CachedToken {
    token: Zeroizing<String>,
    refresh_at: Instant,
}

/// Google Play client.
pub struct Client {
    package_name: String,
    email: String,
    private_key: Zeroizing<String>,
    private_key_id: Option<String>,
    base: Url,
    token_url: Url,
    http: Http,
    token: Mutex<Option<CachedToken>>,
}

impl fmt::Debug for Client {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Client")
            .field("package_name", &self.package_name)
            .field("email", &self.email)
            .field("base", &self.base.as_str())
            .field("http", &self.http)
            .finish_non_exhaustive()
    }
}

impl Client {
    /// Creates a client. Fails early on an unusable service account JSON.
    pub fn new(config: Config, options: ClientOptions) -> Result<Self> {
        let sa: ServiceAccountFile =
            serde_json::from_str(&config.service_account_json).map_err(|_| {
                Error::config(
                    "the service account file is not valid JSON with client_email and private_key",
                )
            })?;
        if !sa.kind.is_empty() && sa.kind != "service_account" {
            return Err(Error::config(format!(
                "expected a service account key file, found type \"{}\"",
                sa.kind
            )));
        }
        jsonwebtoken::EncodingKey::from_rsa_pem(sa.private_key.as_bytes()).map_err(|_| {
            Error::config("the service account private key is not a valid RSA PEM key")
        })?;
        if config.package_name.trim().is_empty() {
            return Err(Error::config("package name is empty"));
        }
        Ok(Self {
            package_name: config.package_name,
            email: sa.client_email,
            private_key: sa.private_key,
            private_key_id: sa.private_key_id,
            base: parse_base_url(&config.base_url)?,
            token_url: parse_base_url(&config.token_url)?,
            http: Http::new(options)?,
            token: Mutex::new(None),
        })
    }

    /// Service account e-mail (to show which account is used).
    pub fn client_email(&self) -> &str {
        &self.email
    }

    /// The options in use.
    pub fn options(&self) -> &ClientOptions {
        &self.http.opts
    }

    // ---- auth --------------------------------------------------------------

    fn assertion(&self) -> Result<Zeroizing<String>> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| Error::config("system clock before 1970"))?
            .as_secs();
        let claims = Claims {
            iss: &self.email,
            scope: SCOPE,
            aud: DEFAULT_TOKEN_URL,
            iat: now,
            exp: now + ASSERTION_LIFETIME.as_secs(),
        };
        let mut header = jsonwebtoken::Header::new(jsonwebtoken::Algorithm::RS256);
        header.kid.clone_from(&self.private_key_id);
        header.typ = Some("JWT".into());
        let key = jsonwebtoken::EncodingKey::from_rsa_pem(self.private_key.as_bytes())
            .map_err(|_| Error::config("invalid service account key"))?;
        jsonwebtoken::encode(&header, &claims, &key)
            .map(Zeroizing::new)
            .map_err(|_| Error::config("signing the service account assertion failed"))
    }

    async fn bearer(&self, force_new: bool) -> Result<Zeroizing<String>> {
        let mut guard = self.token.lock().await;
        if !force_new
            && let Some(cached) = guard.as_ref()
            && Instant::now() < cached.refresh_at
        {
            return Ok(cached.token.clone());
        }
        let assertion = self.assertion()?;
        let form = [
            ("grant_type", "urn:ietf:params:oauth:grant-type:jwt-bearer"),
            ("assertion", assertion.as_str()),
        ];
        let build = || self.http.client.post(self.token_url.clone()).form(&form);
        let reply = self
            .http
            .send(Retry::Idempotent, &build, map_token_error)
            .await?;
        let resp: TokenResponse = parse_json(&reply)?;
        let lifetime = Duration::from_secs(resp.expires_in.unwrap_or(3600));
        let margin = Duration::from_secs(60);
        *guard = Some(CachedToken {
            token: resp.access_token.clone(),
            refresh_at: Instant::now() + lifetime.saturating_sub(margin),
        });
        Ok(resp.access_token)
    }

    // ---- request helpers ---------------------------------------------------

    fn edit_url(&self, upload: bool, rest: &[&str]) -> Result<Url> {
        let mut segments: Vec<&str> = Vec::with_capacity(rest.len() + 5);
        if upload {
            segments.push("upload");
        }
        segments.extend(["androidpublisher", "v3", "applications", &self.package_name]);
        segments.extend(rest);
        join_segments(&self.base, &segments)
    }

    /// Authenticated call; on HTTP 401 the access token is fetched again once.
    async fn api(
        &self,
        method: Method,
        url: Url,
        body: Option<(Bytes, &str)>,
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
                    .bearer_auth(token.as_str());
                match &body {
                    Some((bytes, content_type)) => {
                        rb = rb
                            .header(reqwest::header::CONTENT_TYPE, *content_type)
                            .body(bytes.clone());
                    }
                    None if method == Method::POST => {
                        rb = rb.header(reqwest::header::CONTENT_LENGTH, "0");
                    }
                    None => {}
                }
                rb
            };
            match self.http.send(retry, &build, map_api_error).await {
                Err(e) if e.is_unauthorized() && !force_new => force_new = true,
                other => return other,
            }
        }
    }

    // ---- edits -------------------------------------------------------------

    /// Opens a new edit.
    pub async fn insert_edit(&self) -> Result<Edit> {
        let reply = self
            .api(
                Method::POST,
                self.edit_url(false, &["edits"])?,
                Some((Bytes::from_static(b"{}"), "application/json")),
                Retry::NotIdempotent,
            )
            .await?;
        parse_json(&reply)
    }

    /// Lists the images of one language and type.
    pub async fn list_images(
        &self,
        edit_id: &str,
        language: &str,
        image_type: ImageType,
    ) -> Result<Vec<Image>> {
        let url = self.edit_url(
            false,
            &["edits", edit_id, "listings", language, image_type.as_str()],
        )?;
        let reply = self.api(Method::GET, url, None, Retry::Idempotent).await?;
        Ok(parse_json_or_default::<ImagesList>(&reply)?.images)
    }

    /// Deletes all images of one language and type; returns the deleted ones.
    pub async fn delete_all(
        &self,
        edit_id: &str,
        language: &str,
        image_type: ImageType,
    ) -> Result<Vec<Image>> {
        let url = self.edit_url(
            false,
            &["edits", edit_id, "listings", language, image_type.as_str()],
        )?;
        let reply = self
            .api(Method::DELETE, url, None, Retry::Idempotent)
            .await?;
        Ok(parse_json_or_default::<DeletedImages>(&reply)?.deleted)
    }

    /// Uploads one image (`uploadType=media`). Images appear in upload order.
    pub async fn upload_image(
        &self,
        edit_id: &str,
        language: &str,
        image_type: ImageType,
        bytes: impl Into<Bytes>,
        mime: &str,
        progress: ProgressFn<'_>,
    ) -> Result<Image> {
        let bytes: Bytes = bytes.into();
        if bytes.is_empty() {
            return Err(Error::config("image file is empty"));
        }
        if bytes.len() > MAX_IMAGE_BYTES {
            return Err(Error::Validation {
                message: format!(
                    "image is {} bytes, Google Play accepts at most {MAX_IMAGE_BYTES}",
                    bytes.len()
                ),
                details: Vec::new(),
            });
        }
        if !mime.starts_with("image/") {
            return Err(Error::config(format!("unsupported content type {mime}")));
        }
        let total = bytes.len() as u64;
        let mut p = Progress::new(UploadStep::Transfer, total);
        progress(&p);
        let mut url = self.edit_url(
            true,
            &["edits", edit_id, "listings", language, image_type.as_str()],
        )?;
        url.query_pairs_mut().append_pair("uploadType", "media");
        // Not idempotent: a repeated upload would add the image twice.
        let reply = self
            .api(Method::POST, url, Some((bytes, mime)), Retry::NotIdempotent)
            .await?;
        let image = parse_json::<UploadedImage>(&reply)?.image;
        p.step = UploadStep::Done;
        p.bytes_sent = total;
        p.asset_id = Some(image.id.clone());
        progress(&p);
        Ok(image)
    }

    /// Commits the edit. `changes_not_sent_for_review` must be `true` for apps
    /// whose changes are not sent for review automatically (Play rejects the
    /// commit otherwise).
    pub async fn commit(&self, edit_id: &str, changes_not_sent_for_review: bool) -> Result<Edit> {
        let commit = format!("{edit_id}:commit");
        let mut url = self.edit_url(false, &["edits", &commit])?;
        if changes_not_sent_for_review {
            url.query_pairs_mut()
                .append_pair("changesNotSentForReview", "true");
        }
        let reply = self
            .api(Method::POST, url, None, Retry::NotIdempotent)
            .await?;
        parse_json(&reply)
    }

    /// Discards the edit (abort). An already deleted/expired edit counts as
    /// success.
    pub async fn delete_edit(&self, edit_id: &str) -> Result<()> {
        let url = self.edit_url(false, &["edits", edit_id])?;
        match self.api(Method::DELETE, url, None, Retry::Idempotent).await {
            Ok(_) | Err(Error::NotFound { .. }) => Ok(()),
            Err(e) => Err(e),
        }
    }
}

fn parse_json<T: DeserializeOwned>(reply: &Reply) -> Result<T> {
    serde_json::from_slice(&reply.body).map_err(|e| Error::Transport {
        status: Some(reply.status.as_u16()),
        message: format!("unexpected Google Play response: {e}"),
    })
}

/// Some calls answer with an empty body when there is nothing to report.
fn parse_json_or_default<T: DeserializeOwned + Default>(reply: &Reply) -> Result<T> {
    if reply.body.iter().all(u8::is_ascii_whitespace) {
        Ok(T::default())
    } else {
        parse_json(reply)
    }
}

fn map_token_error(status: StatusCode, body: &[u8]) -> Error {
    let e: OAuthError = serde_json::from_slice(body).unwrap_or_default();
    let message = if e.error.is_empty() {
        format!("token request failed (HTTP {})", status.as_u16())
    } else if e.error_description.is_empty() {
        e.error
    } else {
        format!("{}: {}", e.error, e.error_description)
    };
    match status.as_u16() {
        400 | 401 | 403 => Error::Auth {
            status: Some(status.as_u16()),
            message,
        },
        _ => status_error(status, message),
    }
}

fn map_api_error(status: StatusCode, body: &[u8]) -> Error {
    let e = serde_json::from_slice::<GoogleErrorBody>(body)
        .unwrap_or_default()
        .error;
    let message = if e.message.is_empty() {
        format!("HTTP {}", status.as_u16())
    } else {
        e.message.clone()
    };
    let mut details: Vec<ApiErrorDetail> = e
        .errors
        .iter()
        .map(|i| ApiErrorDetail {
            code: i.reason.clone(),
            title: String::new(),
            detail: i.message.clone(),
        })
        .collect();
    if details.is_empty() && !e.status.is_empty() {
        details.push(ApiErrorDetail {
            code: e.status.clone(),
            title: String::new(),
            detail: e.message.clone(),
        });
    }
    match (status.as_u16(), e.status.as_str()) {
        (_, "FAILED_PRECONDITION" | "ABORTED") | (409, _) => Error::Conflict { message, details },
        (400 | 422, _) => Error::Validation { message, details },
        _ => status_error(status, message),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn image_types_round_trip() {
        for t in ImageType::ALL {
            assert_eq!(ImageType::parse(t.as_str()), Some(t));
        }
        assert_eq!(ImageType::parse("nope"), None);
    }

    #[test]
    fn api_errors_are_categorised() {
        let body = br#"{"error":{"code":400,"message":"Image too small","status":"INVALID_ARGUMENT","errors":[{"reason":"badRequest","message":"Image too small"}]}}"#;
        let e = map_api_error(StatusCode::BAD_REQUEST, body);
        assert!(matches!(e, Error::Validation { .. }));
        assert_eq!(e.details()[0].code, "badRequest");
        let body = br#"{"error":{"code":400,"message":"This Edit has been deleted.","status":"FAILED_PRECONDITION"}}"#;
        assert!(matches!(
            map_api_error(StatusCode::BAD_REQUEST, body),
            Error::Conflict { .. }
        ));
        assert!(matches!(
            map_api_error(StatusCode::FORBIDDEN, b""),
            Error::Auth {
                status: Some(403),
                ..
            }
        ));
    }

    #[test]
    fn config_debug_hides_json() {
        let c = Config::new(r#"{"private_key":"SECRET"}"#, "com.example");
        assert!(!format!("{c:?}").contains("SECRET"));
    }
}
