//! Typed models of the App Store Connect JSON:API resources used by the
//! client (only the fields Arrangia needs). See ADR 0017.

use serde::{Deserialize, Serialize};

use crate::error::ApiErrorDetail;

/// Platform of an App Store version (`filter[platform]`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Platform {
    /// iOS / iPadOS.
    Ios,
    /// macOS.
    MacOs,
    /// tvOS.
    TvOs,
    /// visionOS.
    VisionOs,
}

impl Platform {
    /// API value (`IOS`, `MAC_OS`, `TV_OS`, `VISION_OS`).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ios => "IOS",
            Self::MacOs => "MAC_OS",
            Self::TvOs => "TV_OS",
            Self::VisionOs => "VISION_OS",
        }
    }
}

/// How the app is identified in the project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppRef {
    /// Bundle id (`com.example.app`), resolved via `filter[bundleId]`.
    BundleId(String),
    /// Apple app id (numeric string, "Apple ID" in App Store Connect).
    Id(String),
}

/// `apps` resource.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct App {
    /// Resource id.
    pub id: String,
    /// Bundle id.
    pub bundle_id: String,
    /// App name.
    pub name: String,
}

/// One entry of [`crate::apple::Client::list_apps`] (the app picker).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AppSummary {
    /// Resource id = the numeric "Apple ID" of the app.
    pub id: String,
    /// App name.
    pub name: String,
    /// Bundle id.
    pub bundle_id: String,
    /// SKU (set by the developer when the app was created).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sku: Option<String>,
    /// Primary language (`en-US`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub primary_locale: Option<String>,
}

/// Result of [`crate::apple::Client::apps_overview`]: what the key can see.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppsOverview {
    /// Number of apps visible to the key (`meta.paging.total`), if reported.
    pub total: Option<u64>,
    /// The first app of the list, if any.
    pub first: Option<App>,
}

/// `appStoreVersions` resource.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppStoreVersion {
    /// Resource id.
    pub id: String,
    /// Platform (`IOS`, …).
    pub platform: String,
    /// Version string (`1.2.0`).
    pub version_string: String,
    /// `appVersionState` (falls back to the deprecated `appStoreState`).
    pub state: Option<String>,
}

/// `appStoreVersionLocalizations` resource.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Localization {
    /// Resource id.
    pub id: String,
    /// Store locale (`de-DE`, `en-US`).
    pub locale: String,
}

/// `appScreenshotSets` resource.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScreenshotSet {
    /// Resource id.
    pub id: String,
    /// `screenshotDisplayType` (`APP_IPHONE_67`, …).
    pub display_type: String,
}

/// One part of an asset upload (`uploadOperations[]`).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UploadOperation {
    /// HTTP method (`PUT`).
    pub method: String,
    /// Pre-signed upload URL (secret, never logged; no JWT is sent there).
    pub url: String,
    /// Bytes in this part.
    pub length: u64,
    /// Offset of this part in the file.
    pub offset: u64,
    /// Headers to send with the part.
    #[serde(default)]
    pub request_headers: Vec<HttpHeader>,
}

/// `name`/`value` header pair.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct HttpHeader {
    /// Header name.
    pub name: String,
    /// Header value.
    pub value: String,
}

/// `AppMediaAssetState.state`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeliveryState {
    /// Reserved, parts still uploading.
    AwaitingUpload,
    /// Committed, processing.
    UploadComplete,
    /// Processed successfully (final).
    Complete,
    /// Processing failed (final).
    Failed,
    /// A value this client does not know.
    Other(String),
}

impl DeliveryState {
    fn parse(s: &str) -> Self {
        match s {
            "AWAITING_UPLOAD" => Self::AwaitingUpload,
            "UPLOAD_COMPLETE" => Self::UploadComplete,
            "COMPLETE" => Self::Complete,
            "FAILED" => Self::Failed,
            other => Self::Other(other.to_owned()),
        }
    }

    /// API value.
    pub fn as_str(&self) -> &str {
        match self {
            Self::AwaitingUpload => "AWAITING_UPLOAD",
            Self::UploadComplete => "UPLOAD_COMPLETE",
            Self::Complete => "COMPLETE",
            Self::Failed => "FAILED",
            Self::Other(s) => s,
        }
    }
}

/// `appScreenshots` resource.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppScreenshot {
    /// Resource id.
    pub id: String,
    /// File name given at reservation.
    pub file_name: Option<String>,
    /// File size given at reservation.
    pub file_size: Option<u64>,
    /// MD5 checksum given at commit.
    pub source_file_checksum: Option<String>,
    /// Processing state.
    pub delivery_state: Option<DeliveryState>,
    /// Errors reported in `assetDeliveryState.errors`.
    pub delivery_errors: Vec<ApiErrorDetail>,
    /// Upload operations (only right after the reservation).
    pub upload_operations: Vec<UploadOperation>,
}

// ---- raw JSON:API shapes -------------------------------------------------

#[derive(Debug, Deserialize)]
pub(crate) struct Document<T> {
    pub data: T,
    #[serde(default)]
    pub links: Option<Links>,
    #[serde(default)]
    pub meta: Option<Meta>,
}

#[derive(Debug, Default, Deserialize)]
pub(crate) struct Meta {
    #[serde(default)]
    pub paging: Option<Paging>,
}

#[derive(Debug, Default, Deserialize)]
pub(crate) struct Paging {
    #[serde(default)]
    pub total: Option<u64>,
}

#[derive(Debug, Default, Deserialize)]
pub(crate) struct Links {
    #[serde(default)]
    pub next: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct Resource<A> {
    pub id: String,
    #[serde(default = "Option::default")]
    pub attributes: Option<A>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct AppAttrs {
    pub bundle_id: String,
    pub name: String,
    pub sku: Option<String>,
    pub primary_locale: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct VersionAttrs {
    pub platform: String,
    pub version_string: String,
    pub app_version_state: Option<String>,
    pub app_store_state: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct LocalizationAttrs {
    pub locale: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct SetAttrs {
    pub screenshot_display_type: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct ScreenshotAttrs {
    pub file_name: Option<String>,
    pub file_size: Option<u64>,
    pub source_file_checksum: Option<String>,
    pub asset_delivery_state: Option<AssetState>,
    pub upload_operations: Option<Vec<UploadOperation>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub(crate) struct AssetState {
    pub state: Option<String>,
    pub errors: Vec<MediaError>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub(crate) struct MediaError {
    pub code: String,
    pub description: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub(crate) struct ErrorResponse {
    pub errors: Vec<ErrorEntry>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub(crate) struct ErrorEntry {
    pub code: String,
    pub title: String,
    pub detail: String,
}

impl From<Resource<AppAttrs>> for App {
    fn from(r: Resource<AppAttrs>) -> Self {
        let a = r.attributes.unwrap_or_default();
        Self {
            id: r.id,
            bundle_id: a.bundle_id,
            name: a.name,
        }
    }
}

impl From<Resource<AppAttrs>> for AppSummary {
    fn from(r: Resource<AppAttrs>) -> Self {
        let a = r.attributes.unwrap_or_default();
        Self {
            id: r.id,
            name: a.name,
            bundle_id: a.bundle_id,
            sku: a.sku.filter(|s| !s.is_empty()),
            primary_locale: a.primary_locale.filter(|s| !s.is_empty()),
        }
    }
}

impl From<Resource<VersionAttrs>> for AppStoreVersion {
    fn from(r: Resource<VersionAttrs>) -> Self {
        let a = r.attributes.unwrap_or_default();
        Self {
            id: r.id,
            platform: a.platform,
            version_string: a.version_string,
            state: a.app_version_state.or(a.app_store_state),
        }
    }
}

impl From<Resource<LocalizationAttrs>> for Localization {
    fn from(r: Resource<LocalizationAttrs>) -> Self {
        Self {
            id: r.id,
            locale: r.attributes.unwrap_or_default().locale,
        }
    }
}

impl From<Resource<SetAttrs>> for ScreenshotSet {
    fn from(r: Resource<SetAttrs>) -> Self {
        Self {
            id: r.id,
            display_type: r.attributes.unwrap_or_default().screenshot_display_type,
        }
    }
}

impl From<Resource<ScreenshotAttrs>> for AppScreenshot {
    fn from(r: Resource<ScreenshotAttrs>) -> Self {
        let a = r.attributes.unwrap_or_default();
        let (delivery_state, delivery_errors) = match a.asset_delivery_state {
            Some(s) => (
                s.state.as_deref().map(DeliveryState::parse),
                s.errors
                    .into_iter()
                    .map(|e| ApiErrorDetail {
                        code: e.code,
                        title: String::new(),
                        detail: e.description,
                    })
                    .collect(),
            ),
            None => (None, Vec::new()),
        };
        Self {
            id: r.id,
            file_name: a.file_name,
            file_size: a.file_size,
            source_file_checksum: a.source_file_checksum,
            delivery_state,
            delivery_errors,
            upload_operations: a.upload_operations.unwrap_or_default(),
        }
    }
}

impl From<ErrorEntry> for ApiErrorDetail {
    fn from(e: ErrorEntry) -> Self {
        Self {
            code: e.code,
            title: e.title,
            detail: e.detail,
        }
    }
}
