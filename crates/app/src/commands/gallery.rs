//! Template gallery (`api.vitrinestudio.app`, ADR 0025): browse, preview
//! and download without an account. Downloads are checked with the same
//! safe-import code as local template files (`Package::from_zip_bytes`)
//! and installed into the app-wide template store.
//!
//! An unreachable server (offline, DNS, timeout, proxy error) is
//! [`ErrorCode::Unreachable`](crate::error::ErrorCode::Unreachable), so a
//! front end can show "Gallery not reachable" without alarming the user.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use vitrine_core::template::{Limits, Package, TemplateSummary};
use vitrine_publish::gallery::{GalleryClient, ListQuery, RemoteTemplate, api_url};

use super::AppState;
use super::templates::image_size;
use crate::error::{CmdResult, CommandError};
use crate::preview::{PixelFormat, PreviewImage};
use crate::upload::remote::block_on;

/// Page size of [`gallery_list`] (the server allows 1–100).
pub const GALLERY_PAGE: u32 = 50;

impl AppState {
    /// Client of the gallery API (override, `VITRINE_API_URL` on loopback,
    /// or the production server).
    pub fn gallery_client(&self) -> CmdResult<GalleryClient> {
        let base = api_url(self.gallery_url.as_deref())?;
        Ok(GalleryClient::new(base)?)
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GalleryListArgs {
    /// Search in name, description and tags.
    #[serde(default)]
    pub query: Option<String>,
    #[serde(default)]
    pub tag: Option<String>,
    #[serde(default)]
    pub offset: Option<u32>,
    /// 1–100 (default 50).
    #[serde(default)]
    pub limit: Option<u32>,
}

/// A gallery template plus the installed version of the same id, if any.
#[derive(Debug, Clone, Serialize)]
pub struct GalleryItem {
    #[serde(flatten)]
    pub template: RemoteTemplate,
    pub installed_version: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GalleryListResult {
    pub templates: Vec<GalleryItem>,
    pub offset: u32,
    pub limit: u32,
    /// A full page came back: ask again with `offset + limit`.
    pub has_more: bool,
}

/// Approved gallery templates, newest first.
pub fn gallery_list(app: &AppState, args: GalleryListArgs) -> CmdResult<GalleryListResult> {
    let client = app.gallery_client()?;
    let q = ListQuery {
        query: args.query,
        tag: args.tag,
        limit: Some(args.limit.unwrap_or(GALLERY_PAGE).clamp(1, 100)),
        offset: args.offset,
    };
    let page = block_on(async move { client.list(&q).await })??;
    let installed = app.templates().list();
    let has_more = page.limit > 0 && page.templates.len() >= page.limit as usize;
    let templates = page
        .templates
        .into_iter()
        .map(|t| GalleryItem {
            installed_version: installed
                .iter()
                .find(|i| i.id == t.id)
                .map(|i| i.version.clone()),
            template: t,
        })
        .collect();
    Ok(GalleryListResult {
        templates,
        offset: page.offset,
        limit: page.limit,
        has_more,
    })
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GalleryPreviewArgs {
    pub id: String,
    /// Canvas (default: the template's first).
    #[serde(default)]
    pub canvas: Option<String>,
}

/// Preview PNG of a gallery template canvas (rendered by the server).
pub fn gallery_preview(app: &AppState, args: GalleryPreviewArgs) -> CmdResult<Arc<PreviewImage>> {
    let client = app.gallery_client()?;
    let png = block_on(async move {
        let canvas = match args.canvas {
            Some(c) => c,
            None => client
                .detail(&args.id)
                .await?
                .canvases
                .first()
                .map(|c| c.id.clone())
                .ok_or_else(|| {
                    vitrine_publish::gallery::GalleryError::Invalid(
                        "the template has no canvas".into(),
                    )
                })?,
        };
        client.preview(&args.id, &canvas).await
    })??;
    if !png.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Err(CommandError::new(
            crate::error::ErrorCode::Remote,
            "the gallery sent a preview that is not a PNG",
        ));
    }
    let (width, height) = image_size(&png)?;
    Ok(Arc::new(PreviewImage {
        width,
        height,
        format: PixelFormat::Png,
        origin_x: 0.0,
        origin_y: 0.0,
        scale: 0.0,
        data: png,
    }))
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GalleryDownloadArgs {
    pub id: String,
    /// Overwrite an installed template with the same id (update).
    #[serde(default)]
    pub replace: bool,
}

/// Downloads a gallery template, checks it like any template file and
/// installs it into the app-wide store.
pub fn gallery_download(app: &AppState, args: GalleryDownloadArgs) -> CmdResult<TemplateSummary> {
    let client = app.gallery_client()?;
    let id = args.id.clone();
    let bytes = block_on(async move { client.download(&id).await })??;
    let pkg = Package::from_zip_bytes(&bytes, Limits::default())?;
    if pkg.manifest.id != args.id {
        return Err(CommandError::new(
            crate::error::ErrorCode::Remote,
            format!(
                "the gallery sent template `{}` instead of `{}`",
                pkg.manifest.id, args.id
            ),
        ));
    }
    Ok(app.templates().install(&pkg, args.replace)?)
}
