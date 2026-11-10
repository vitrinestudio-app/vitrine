//! App Store Connect **App Asset Library** (ASC API 4.5.1; ADR 0019).
//!
//! Flow: [`Client::asset_library`] (library id of the app) →
//! [`Client::ref_data`] (placement types, groups, display classes, specs –
//! runtime strings, never hard-coded) → [`Client::upload_library_image_committed`]
//! (reserve → parts → commit `{uploaded: true}`, no checksum) → poll
//! [`Client::library_image`] until processed (`PREPARE_FOR_SUBMISSION`, `specId`
//! set) → [`Client::create_placement`] per localization → [`Client::order_placements`].
//! Removing: [`Client::delete_placement`] first, then
//! [`Client::delete_library_image`] (an asset with placements cannot be deleted:
//! `STATE_ERROR.ASSET_HAS_PLACEMENTS`).

use bytes::Bytes;
use reqwest::{Method, Url};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::model::{Document, Links, Resource, UploadOperation};
use super::{Client, parse_json};
use crate::error::{ApiErrorDetail, Error, Result};
use crate::http::Retry;
use crate::progress::{Progress, ProgressFn, UploadStep};

/// `AppAssetLibraryAssetCategory` of screenshots and previews.
pub const CATEGORY_SCREENSHOTS: &str = "APP_SCREENSHOTS_AND_PREVIEWS";
/// `AppAssetLibraryAssetCategory` of other marketing media (header,
/// search results, in-app event artwork).
pub const CATEGORY_CREATIVE: &str = "CREATIVE_ASSETS";
/// Error code of deleting an asset that still has placements.
pub const ASSET_HAS_PLACEMENTS: &str = "STATE_ERROR.ASSET_HAS_PLACEMENTS";

/// `appAssetLibraries` resource (one per app, id stable for its life).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AssetLibrary {
    pub id: String,
}

// ----- reference data ---------------------------------------------------------------------

/// `appAssetLibraryRefData`: the catalog of specifications, placement
/// groups and limits (all `data[]` entries merged). Field names as Apple
/// sends them (camelCase).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct RefData {
    pub features: Vec<RefFeature>,
    pub placement_profile_groups: Vec<RefGroup>,
    pub image_specs: Vec<RefSpec>,
    pub video_specs: Vec<RefSpec>,
    pub placement_types: Vec<RefPlacementType>,
    pub display_classes: Vec<RefDisplayClass>,
}

/// `features[]`: what an App Store feature supports.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct RefFeature {
    /// `APP_STORE_VERSIONS`, `CUSTOM_PRODUCT_PAGES`, …
    pub feature_id: String,
    pub placement_policies: Vec<RefPlacementPolicy>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct RefPlacementPolicy {
    pub placement_type: String,
    pub group_limits: Vec<RefGroupLimit>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct RefGroupLimit {
    pub group_ids: Vec<String>,
    pub max_count: Option<u32>,
}

/// `placementProfileGroups[]`: a group id with its platform and display class.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct RefGroup {
    pub placement_profile_group_id: String,
    /// `AppAssetLibraryPlacementPlatform` (`IPHONE_APP_STORE`, …).
    pub platform: String,
    /// `AppAssetLibraryDisplayClass` (`IPHONE_DUO`, …).
    pub display_class_id: String,
}

/// `imageSpecs[]` / `videoSpecs[]` (the fields Arrangia reads).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct RefSpec {
    pub spec_id: String,
    pub short_name: String,
    pub dimensions: RefDimensions,
    pub aspect_ratio: Option<String>,
    pub compatible_placement_types: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alpha_allowed: Option<bool>,
    pub file_extensions: Vec<String>,
    pub mime_types: Vec<String>,
    pub max_file_size: Option<u64>,
    pub universal_asset: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct RefDimensions {
    pub min_width: u32,
    pub max_width: u32,
    pub min_height: u32,
    pub max_height: u32,
}

impl RefSpec {
    /// Does an image of `w`×`h` pixels satisfy the spec's dimensions?
    pub fn accepts_size(&self, w: u32, h: u32) -> bool {
        let d = self.dimensions;
        (d.min_width..=d.max_width.max(d.min_width)).contains(&w)
            && (d.min_height..=d.max_height.max(d.min_height)).contains(&h)
    }

    /// `1290x2796` or `1920-3840x1280-2560`.
    pub fn size_label(&self) -> String {
        let d = self.dimensions;
        let range = |a: u32, b: u32| {
            if a == b || b == 0 {
                a.to_string()
            } else {
                format!("{a}-{b}")
            }
        };
        format!(
            "{}x{}",
            range(d.min_width, d.max_width),
            range(d.min_height, d.max_height)
        )
    }
}

/// `placementTypes[]`: accepted categories and group → spec ids.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct RefPlacementType {
    pub placement_type_id: String,
    pub accepts_asset_categories: Vec<String>,
    pub spec_mappings: Vec<RefSpecMapping>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct RefSpecMapping {
    pub placement_group_id: String,
    pub specs: Vec<String>,
}

/// `displayClasses[]`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct RefDisplayClass {
    pub display_class_id: String,
    pub device_family: Option<String>,
    pub screen_dimensions: Vec<String>,
}

impl RefData {
    fn merge(&mut self, other: Self) {
        self.features.extend(other.features);
        self.placement_profile_groups
            .extend(other.placement_profile_groups);
        self.image_specs.extend(other.image_specs);
        self.video_specs.extend(other.video_specs);
        self.placement_types.extend(other.placement_types);
        self.display_classes.extend(other.display_classes);
    }

    pub fn group(&self, id: &str) -> Option<&RefGroup> {
        self.placement_profile_groups
            .iter()
            .find(|g| g.placement_profile_group_id == id)
    }

    pub fn placement_type(&self, id: &str) -> Option<&RefPlacementType> {
        self.placement_types
            .iter()
            .find(|t| t.placement_type_id == id)
    }

    /// An image or video spec by id.
    pub fn spec(&self, id: &str) -> Option<&RefSpec> {
        self.image_specs
            .iter()
            .chain(&self.video_specs)
            .find(|s| s.spec_id == id)
    }

    /// Image specs a group accepts for a placement type.
    pub fn image_specs_for(&self, placement_type: &str, group: &str) -> Vec<&RefSpec> {
        let Some(t) = self.placement_type(placement_type) else {
            return Vec::new();
        };
        t.spec_mappings
            .iter()
            .filter(|m| m.placement_group_id == group)
            .flat_map(|m| &m.specs)
            .filter_map(|id| self.image_specs.iter().find(|s| &s.spec_id == id))
            .collect()
    }

    /// Groups of `placement_type` (from its spec mappings) with the given
    /// display class (`None` = any) on one of `platforms`, in ref-data order.
    pub fn groups_for(
        &self,
        placement_type: &str,
        display_class: Option<&str>,
        platforms: &[String],
    ) -> Vec<&RefGroup> {
        let Some(t) = self.placement_type(placement_type) else {
            return Vec::new();
        };
        let mut out: Vec<&RefGroup> = Vec::new();
        for m in &t.spec_mappings {
            let Some(g) = self.group(&m.placement_group_id) else {
                continue;
            };
            if display_class.is_some_and(|c| c != g.display_class_id) {
                continue;
            }
            if !platforms.is_empty() && !platforms.contains(&g.platform) {
                continue;
            }
            if !out
                .iter()
                .any(|x| x.placement_profile_group_id == g.placement_profile_group_id)
            {
                out.push(g);
            }
        }
        out
    }

    /// `maxCount` of a placement type in a group for a feature
    /// (`APP_STORE_VERSIONS`).
    pub fn max_count(&self, feature: &str, placement_type: &str, group: &str) -> Option<u32> {
        self.features
            .iter()
            .filter(|f| f.feature_id == feature)
            .flat_map(|f| &f.placement_policies)
            .filter(|p| p.placement_type == placement_type)
            .flat_map(|p| &p.group_limits)
            .find(|l| l.group_ids.iter().any(|g| g == group))
            .and_then(|l| l.max_count)
    }
}

/// Filters of [`Client::ref_data`] (empty = everything).
#[derive(Debug, Clone, Default)]
pub struct RefDataFilter {
    pub features: Vec<String>,
    pub placement_types: Vec<String>,
    pub placement_profile_groups: Vec<String>,
    pub specs: Vec<String>,
}

// ----- images and placements --------------------------------------------------------------

/// `appAssetLibraryImages` resource.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct LibraryImage {
    pub id: String,
    pub file_name: Option<String>,
    pub file_size: Option<u64>,
    pub category: Option<String>,
    pub reference_name: Option<String>,
    /// Specification App Store Connect matched during processing.
    pub spec_id: Option<String>,
    /// `AppAssetLibraryAssetState` (`AWAITING_UPLOAD`, `UPLOAD_COMPLETE`,
    /// `PREPARE_FOR_SUBMISSION`, `FAILED`, …).
    pub state: Option<String>,
    /// `stateDetails` (`FAILED` reasons).
    pub state_details: Vec<ApiErrorDetail>,
    /// Processed size (`imageAsset.width/height`).
    pub width: Option<u32>,
    pub height: Option<u32>,
    #[serde(skip)]
    pub upload_operations: Vec<UploadOperation>,
}

/// States of a processed asset (it can be placed).
const READY_STATES: &[&str] = &[
    "PREPARE_FOR_SUBMISSION",
    "COMPLETE",
    "READY_FOR_REVIEW",
    "WAITING_FOR_REVIEW",
    "IN_REVIEW",
    "ACCEPTED",
    "APPROVED",
];

impl LibraryImage {
    fn state_is(&self, s: &str) -> bool {
        self.state.as_deref() == Some(s)
    }

    /// Processed successfully (`PREPARE_FOR_SUBMISSION` or a later review state).
    pub fn is_ready(&self) -> bool {
        self.state
            .as_deref()
            .is_some_and(|s| READY_STATES.contains(&s))
    }

    /// Committed, Apple is processing it.
    pub fn is_processing(&self) -> bool {
        self.state_is("UPLOAD_COMPLETE")
    }

    /// Processing failed (final; delete and upload again).
    pub fn is_failed(&self) -> bool {
        self.state_is("FAILED")
    }

    /// Ready or still processing (worth keeping).
    pub fn is_usable(&self) -> bool {
        self.is_ready() || self.is_processing()
    }
}

/// `appAssetLibraryPlacements` resource.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Placement {
    pub id: String,
    pub placement_type: String,
    pub placement_group: String,
    /// `AppAssetLibraryPlacementState` (`ASSET_PROCESSING`,
    /// `PARENT_PREPARE_FOR_SUBMISSION`, …).
    pub state: Option<String>,
    /// The placed image (`relationships.image`).
    pub image_id: Option<String>,
    /// The image itself when the listing included it.
    pub image: Option<LibraryImage>,
}

// ----- raw shapes -------------------------------------------------------------------------

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct ImageAttrs {
    file_name: Option<String>,
    file_size: Option<u64>,
    category: Option<String>,
    reference_name: Option<String>,
    spec_id: Option<String>,
    state: Option<String>,
    state_details: Option<Vec<StateDetail>>,
    image_asset: Option<ImageAsset>,
    upload_operations: Option<Vec<UploadOperation>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct StateDetail {
    code: String,
    description: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct ImageAsset {
    width: Option<u32>,
    height: Option<u32>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct PlacementAttrs {
    placement_type: String,
    placement_group: String,
    state: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PlacementRes {
    id: String,
    #[serde(default)]
    attributes: Option<PlacementAttrs>,
    #[serde(default)]
    relationships: Option<PlacementRels>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct PlacementRels {
    image: Option<Linkage>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct Linkage {
    data: Option<ResourceId>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct ResourceId {
    #[serde(rename = "type")]
    kind: String,
    id: String,
}

#[derive(Debug, Deserialize)]
struct PlacementsDoc {
    data: Vec<PlacementRes>,
    #[serde(default)]
    included: Vec<IncludedRes>,
    #[serde(default)]
    links: Option<Links>,
}

#[derive(Debug, Deserialize)]
struct IncludedRes {
    #[serde(rename = "type", default)]
    kind: String,
    id: String,
    #[serde(default)]
    attributes: Option<Value>,
}

impl From<Resource<ImageAttrs>> for LibraryImage {
    fn from(r: Resource<ImageAttrs>) -> Self {
        image_from(r.id, r.attributes.unwrap_or_default())
    }
}

fn image_from(id: String, a: ImageAttrs) -> LibraryImage {
    LibraryImage {
        id,
        file_name: a.file_name,
        file_size: a.file_size,
        category: a.category,
        reference_name: a.reference_name,
        spec_id: a.spec_id,
        state: a.state,
        state_details: a
            .state_details
            .unwrap_or_default()
            .into_iter()
            .map(|d| ApiErrorDetail {
                code: d.code,
                title: String::new(),
                detail: d.description,
            })
            .collect(),
        width: a.image_asset.as_ref().and_then(|i| i.width),
        height: a.image_asset.as_ref().and_then(|i| i.height),
        upload_operations: a.upload_operations.unwrap_or_default(),
    }
}

/// Is `e` Apple's refusal to delete an asset that still has placements?
pub fn is_asset_has_placements(e: &Error) -> bool {
    e.details().iter().any(|d| d.code == ASSET_HAS_PLACEMENTS)
}

// ----- client -----------------------------------------------------------------------------

impl Client {
    /// The app's asset library (`GET /v1/apps/{id}/assetLibrary`).
    pub async fn asset_library(&self, app_id: &str) -> Result<AssetLibrary> {
        let doc: Document<Resource<Value>> = self
            .get_doc(self.url(&["v1", "apps", app_id, "assetLibrary"])?)
            .await?;
        Ok(AssetLibrary { id: doc.data.id })
    }

    /// The reference data (`GET /v1/appAssetLibraryRefData`), all entries
    /// merged. Read-only.
    pub async fn ref_data(&self, filter: &RefDataFilter) -> Result<RefData> {
        let mut url = self.url(&["v1", "appAssetLibraryRefData"])?;
        {
            let mut q = url.query_pairs_mut();
            for (name, values) in [
                ("filter[features]", &filter.features),
                ("filter[placementTypes]", &filter.placement_types),
                (
                    "filter[placementProfileGroups]",
                    &filter.placement_profile_groups,
                ),
                ("filter[specs]", &filter.specs),
            ] {
                if !values.is_empty() {
                    q.append_pair(name, &values.join(","));
                }
            }
        }
        let mut out = RefData::default();
        for r in self.get_all::<RefData>(url).await? {
            if let Some(a) = r.attributes {
                out.merge(a);
            }
        }
        Ok(out)
    }

    /// Images of a library (`GET /v1/appAssetLibraries/{id}/images`),
    /// optionally of one category.
    pub async fn library_images(
        &self,
        library_id: &str,
        category: Option<&str>,
    ) -> Result<Vec<LibraryImage>> {
        let mut url = self.url(&["v1", "appAssetLibraries", library_id, "images"])?;
        {
            let mut q = url.query_pairs_mut();
            if let Some(c) = category {
                q.append_pair("filter[category]", c);
            }
            q.append_pair("limit", "200");
        }
        Ok(self
            .get_all::<ImageAttrs>(url)
            .await?
            .into_iter()
            .map(LibraryImage::from)
            .collect())
    }

    /// One image (state polling).
    pub async fn library_image(&self, id: &str) -> Result<LibraryImage> {
        let doc: Document<Resource<ImageAttrs>> = self
            .get_doc(self.url(&["v1", "appAssetLibraryImages", id])?)
            .await?;
        Ok(doc.data.into())
    }

    /// Reserve → upload the parts → commit with `{uploaded: true}` (no
    /// checksum; Apple validates the byte count against `fileSize`). Does
    /// not wait for processing; poll [`Client::library_image`].
    ///
    /// A failed transfer/commit deletes the reservation (best effort); a
    /// cancellation leaves it (an `AWAITING_UPLOAD` leftover, cleaned up by
    /// the next run).
    pub async fn upload_library_image_committed(
        &self,
        library_id: &str,
        upload: &LibraryUpload<'_>,
        bytes: impl Into<Bytes>,
        progress: ProgressFn<'_>,
    ) -> Result<LibraryImage> {
        let bytes: Bytes = bytes.into();
        let total = bytes.len() as u64;
        if bytes.is_empty() {
            return Err(Error::config("image file is empty"));
        }
        let mut attrs = json!({
            "fileName": upload.file_name,
            "fileSize": total,
            "category": upload.category,
        });
        if let Some(r) = upload.reference_name {
            attrs["referenceName"] = json!(r);
        }
        let body = json!({
            "data": {
                "type": "appAssetLibraryImages",
                "attributes": attrs,
                "relationships": {
                    "assetLibrary": { "data": { "type": "appAssetLibraries", "id": library_id } }
                }
            }
        });
        let reply = self
            .api(
                Method::POST,
                self.url(&["v1", "appAssetLibraryImages"])?,
                Some(body),
                Retry::NotIdempotent,
            )
            .await?;
        let doc: Document<Resource<ImageAttrs>> = parse_json(&reply)?;
        let reserved: LibraryImage = doc.data.into();
        let id = reserved.id.clone();
        let mut p = Progress::new(UploadStep::Reserve, total);
        p.asset_id = Some(id.clone());
        p.parts = Some((0, reserved.upload_operations.len()));
        progress(&p);
        let result = async {
            self.transfer_parts(&reserved.upload_operations, &bytes, &mut p, progress)
                .await?;
            let body = json!({
                "data": {
                    "type": "appAssetLibraryImages",
                    "id": id,
                    "attributes": { "uploaded": true }
                }
            });
            let reply = self
                .api(
                    Method::PATCH,
                    self.url(&["v1", "appAssetLibraryImages", &id])?,
                    Some(body),
                    Retry::Idempotent,
                )
                .await?;
            p.step = UploadStep::Commit;
            progress(&p);
            Ok::<_, Error>(
                parse_json::<Document<Resource<ImageAttrs>>>(&reply)
                    .ok()
                    .map(|d| LibraryImage::from(d.data)),
            )
        }
        .await;
        match result {
            Ok(committed) => Ok(committed.unwrap_or(LibraryImage {
                state: Some("UPLOAD_COMPLETE".into()),
                upload_operations: Vec::new(),
                ..reserved
            })),
            Err(e) => {
                if !matches!(e, Error::Cancelled) {
                    let _ = self.delete_library_image(&id).await;
                }
                Err(e)
            }
        }
    }

    /// Deletes an image. 404 counts as success. Fails with a conflict
    /// ([`is_asset_has_placements`]) while placements still use it.
    pub async fn delete_library_image(&self, id: &str) -> Result<()> {
        match self
            .api(
                Method::DELETE,
                self.url(&["v1", "appAssetLibraryImages", id])?,
                None,
                Retry::Idempotent,
            )
            .await
        {
            Ok(_) | Err(Error::NotFound { .. }) => Ok(()),
            Err(e) => Err(e),
        }
    }

    /// Placements of an App Store version localization in display order
    /// (`sort=placementGroupPosition`), with their images included;
    /// optionally of one placement type / group.
    pub async fn localization_placements(
        &self,
        localization_id: &str,
        placement_type: Option<&str>,
        placement_group: Option<&str>,
    ) -> Result<Vec<Placement>> {
        let mut url = self.url(&[
            "v1",
            "appStoreVersionLocalizations",
            localization_id,
            "placements",
        ])?;
        {
            let mut q = url.query_pairs_mut();
            if let Some(t) = placement_type {
                q.append_pair("filter[placementType]", t);
            }
            if let Some(g) = placement_group {
                q.append_pair("filter[placementGroup]", g);
            }
            q.append_pair("sort", "placementGroupPosition")
                .append_pair("include", "image")
                .append_pair(
                    "fields[appAssetLibraryImages]",
                    "fileName,fileSize,referenceName,specId,state,stateDetails,imageAsset,category",
                )
                .append_pair("limit", "200");
        }
        let mut out = Vec::new();
        loop {
            let reply = self.api(Method::GET, url, None, Retry::Idempotent).await?;
            let doc: PlacementsDoc = parse_json(&reply)?;
            let images: Vec<LibraryImage> = doc
                .included
                .into_iter()
                .filter(|r| r.kind == "appAssetLibraryImages")
                .map(|r| {
                    let attrs = r
                        .attributes
                        .and_then(|v| serde_json::from_value(v).ok())
                        .unwrap_or_default();
                    image_from(r.id, attrs)
                })
                .collect();
            for r in doc.data {
                let a = r.attributes.unwrap_or_default();
                let image_id = r
                    .relationships
                    .and_then(|x| x.image)
                    .and_then(|l| l.data)
                    .filter(|d| d.kind.is_empty() || d.kind == "appAssetLibraryImages")
                    .map(|d| d.id);
                let image = image_id
                    .as_ref()
                    .and_then(|id| images.iter().find(|i| &i.id == id).cloned());
                out.push(Placement {
                    id: r.id,
                    placement_type: a.placement_type,
                    placement_group: a.placement_group,
                    state: a.state,
                    image_id,
                    image,
                });
            }
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

    /// Places an image on an App Store version localization. Placements
    /// cannot be changed later (delete and create again).
    pub async fn create_placement(
        &self,
        placement_type: &str,
        placement_group: &str,
        image_id: &str,
        localization_id: &str,
    ) -> Result<Placement> {
        let body = json!({
            "data": {
                "type": "appAssetLibraryPlacements",
                "attributes": { "placementType": placement_type, "placementGroup": placement_group },
                "relationships": {
                    "image": { "data": { "type": "appAssetLibraryImages", "id": image_id } },
                    "appStoreVersionLocalization": {
                        "data": { "type": "appStoreVersionLocalizations", "id": localization_id }
                    }
                }
            }
        });
        let reply = self
            .api(
                Method::POST,
                self.url(&["v1", "appAssetLibraryPlacements"])?,
                Some(body),
                Retry::NotIdempotent,
            )
            .await?;
        let doc: Document<PlacementRes> = parse_json(&reply)?;
        let a = doc.data.attributes.unwrap_or_default();
        Ok(Placement {
            id: doc.data.id,
            placement_type: if a.placement_type.is_empty() {
                placement_type.to_owned()
            } else {
                a.placement_type
            },
            placement_group: if a.placement_group.is_empty() {
                placement_group.to_owned()
            } else {
                a.placement_group
            },
            state: a.state,
            image_id: Some(image_id.to_owned()),
            image: None,
        })
    }

    /// Removes a placement (the asset stays in the library). 404 = success.
    pub async fn delete_placement(&self, id: &str) -> Result<()> {
        match self
            .api(
                Method::DELETE,
                self.url(&["v1", "appAssetLibraryPlacements", id])?,
                None,
                Retry::Idempotent,
            )
            .await
        {
            Ok(_) | Err(Error::NotFound { .. }) => Ok(()),
            Err(e) => Err(e),
        }
    }

    /// Sets the display order of a group's placements in a localization
    /// (`POST /v1/appAssetLibraryPlacementOrderingRequests`). Repeating the
    /// same request is harmless, so it is retried like an idempotent call.
    pub async fn order_placements(
        &self,
        placement_group: &str,
        localization_id: &str,
        placement_ids: &[String],
    ) -> Result<()> {
        let ordered: Vec<Value> = placement_ids
            .iter()
            .map(|id| json!({ "type": "appAssetLibraryPlacements", "id": id }))
            .collect();
        let body = json!({
            "data": {
                "type": "appAssetLibraryPlacementOrderingRequests",
                "attributes": { "placementGroup": placement_group },
                "relationships": {
                    "orderedPlacements": { "data": ordered },
                    "appStoreVersionLocalization": {
                        "data": { "type": "appStoreVersionLocalizations", "id": localization_id }
                    }
                }
            }
        });
        self.api(
            Method::POST,
            self.url(&["v1", "appAssetLibraryPlacementOrderingRequests"])?,
            Some(body),
            Retry::Idempotent,
        )
        .await?;
        Ok(())
    }
}

/// What [`Client::upload_library_image_committed`] reserves.
#[derive(Debug, Clone, Copy)]
pub struct LibraryUpload<'a> {
    pub file_name: &'a str,
    /// `APP_SCREENSHOTS_AND_PREVIEWS` or `CREATIVE_ASSETS` (cannot change later).
    pub category: &'a str,
    /// Shown in App Store Connect; Arrangia stores a content marker in it.
    pub reference_name: Option<&'a str>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spec_sizes() {
        let mut s = RefSpec {
            dimensions: RefDimensions {
                min_width: 1290,
                max_width: 1290,
                min_height: 2796,
                max_height: 2796,
            },
            ..RefSpec::default()
        };
        assert!(s.accepts_size(1290, 2796));
        assert!(!s.accepts_size(1320, 2868));
        assert_eq!(s.size_label(), "1290x2796");
        s.dimensions = RefDimensions {
            min_width: 1920,
            max_width: 3840,
            min_height: 1280,
            max_height: 2560,
        };
        assert!(s.accepts_size(3000, 2000));
        assert_eq!(s.size_label(), "1920-3840x1280-2560");
    }
}
