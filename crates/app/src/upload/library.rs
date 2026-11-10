//! The App Asset Library flow (ASC API 4.5.1, ADR 0019): the second
//! implementation of [`AppleUploadFlow`].
//!
//! Per set (store locale × slot):
//!
//! 1. **Route**: the slot's placement type(s), display class and platforms
//!    come from `data/asset-library.yaml`; the placement **group** from the
//!    app's ref data (`placementProfileGroups` ∩ the type's `specMappings`;
//!    several → the one whose image specs accept the image size; creative
//!    assets get every matching group).
//! 2. **Assets**: a planned image the localization already shows (placement
//!    with the same content marker) or the library already holds (same
//!    marker, not failed) is reused; everything else is reserved, uploaded
//!    and committed (`{uploaded: true}`), then all are polled together
//!    (processing budget, like the set flow). `FAILED` fails the set before
//!    any placement changes.
//! 3. **Placements**: stale placements of the group are deleted, missing
//!    ones created (also for assets still processing – Apple allows it,
//!    the placement starts `ASSET_PROCESSING`), the group is ordered with
//!    an ordering request and verified (`sort=placementGroupPosition`).
//! 4. **Old assets**: images of deleted placements that Arrangia uploaded
//!    (content marker) are deleted afterwards – placements first, then the
//!    asset; `STATE_ERROR.ASSET_HAS_PLACEMENTS` means another localization
//!    still uses it and is fine.
//!
//! **Content marker.** Library images carry no checksum. Arrangia sets
//! `referenceName` to `<file stem> [<first 12 hex of the SHA-256>]` and
//! matches on that marker plus the file size.

use std::sync::Mutex;
use std::time::Instant;

use arrangia_core::catalog::asset_library;
use arrangia_core::model::{ApplePlacement, AppleSlot};
use arrangia_publish::apple::{
    self, AppRef, LibraryImage, LibraryUpload, Platform, RefData, RefDataFilter,
};
use arrangia_publish::{ApiErrorDetail, Error, ProgressFn, Result};
use serde::Serialize;

use super::PlanImage;
use super::remote::{
    AppleTarget, AppleUploadFlow, RemoteSet, ReplaceOptions, ReplaceOutcome, ScreenshotSetFlow,
    SetHooks, UploadFile,
};

/// `code` of the error when the ref data has no group for a slot.
pub const NO_PLACEMENT_GROUP: &str = "NO_PLACEMENT_GROUP";

/// The app's library and the ref data, loaded once per plan/run.
#[derive(Debug)]
pub(crate) struct LibraryContext {
    pub library_id: String,
    pub ref_data: RefData,
    /// Images of the library (listed on first use; uploads are added).
    images: Mutex<Option<Vec<LibraryImage>>>,
}

impl LibraryContext {
    async fn images(&self, client: &apple::Client) -> Result<Vec<LibraryImage>> {
        if let Some(list) = self
            .images
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
        {
            return Ok(list);
        }
        let list = client.library_images(&self.library_id, None).await?;
        *self
            .images
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(list.clone());
        Ok(list)
    }

    fn remember(&self, image: LibraryImage) {
        let mut g = self
            .images
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(list) = g.as_mut() {
            list.retain(|i| i.id != image.id);
            list.push(image);
        }
    }

    fn forget(&self, id: &str) {
        if let Some(list) = self
            .images
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_mut()
        {
            list.retain(|i| i.id != id);
        }
    }
}

/// One placement a slot gets: type + resolved group.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PlacementRoute {
    pub placement_type: String,
    pub group: String,
    /// An image spec of the group accepts the image size (`false`: Apple
    /// will probably reject it; shown as a warning).
    pub size_ok: bool,
}

impl PlacementRoute {
    /// `APP_SCREENSHOT → IPHONE_DYNAMIC_ISLAND_LARGE_PROFILE`.
    pub fn label(&self) -> String {
        format!("{} → {}", self.placement_type, self.group)
    }
}

/// The slot behind a plan slot key (`APP_IPHONE_67`,
/// `PRODUCT_PAGE_HEADER_ASSET`, …).
pub(crate) fn slot_from_key(key: &str) -> AppleSlot {
    for p in [
        ApplePlacement::Header,
        ApplePlacement::SearchResults,
        ApplePlacement::HeaderAndSearch,
    ] {
        if p.api_name() == key {
            return AppleSlot {
                placement: Some(p),
                ..AppleSlot::default()
            };
        }
    }
    AppleSlot::screenshots(key)
}

/// Only the asset library can upload this slot (Duo, creative assets).
pub(crate) fn asset_library_only(key: &str) -> bool {
    asset_library().asset_library_only(&slot_from_key(key))
}

/// Local description of a slot's placement (`APP_SCREENSHOT ·
/// IPHONE_DUO`), before the group is known.
pub(crate) fn local_label(key: &str) -> Option<String> {
    let s = asset_library().slot(&slot_from_key(key))?;
    let target = s.display_class.unwrap_or_else(|| s.platforms.join("/"));
    Some(format!("{} · {target}", s.placement_types.join(" + ")))
}

/// Resolves the placement routes of a slot from the ref data.
pub(crate) fn resolve(
    rd: &RefData,
    key: &str,
    size: Option<(u32, u32)>,
) -> std::result::Result<Vec<PlacementRoute>, String> {
    let Some(sp) = asset_library().slot(&slot_from_key(key)) else {
        return Err(format!(
            "`{key}` has no App Asset Library mapping (data/asset-library.yaml)"
        ));
    };
    let mut out = Vec::new();
    for t in &sp.placement_types {
        let groups = rd.groups_for(t, sp.display_class.as_deref(), &sp.platforms);
        if groups.is_empty() {
            return Err(format!(
                "App Store Connect's asset types list no {t} group for {} ({}) – see `arrangia upload refdata`",
                sp.display_class.as_deref().unwrap_or("any display class"),
                sp.platforms.join(", ")
            ));
        }
        let accepts = |g: &str| {
            let specs = rd.image_specs_for(t, g);
            size.is_none_or(|(w, h)| specs.is_empty() || specs.iter().any(|s| s.accepts_size(w, h)))
        };
        let ids: Vec<&str> = groups
            .iter()
            .map(|g| g.placement_profile_group_id.as_str())
            .collect();
        let fitting: Vec<&str> = ids.iter().copied().filter(|g| accepts(g)).collect();
        if sp.every_group {
            if fitting.is_empty() {
                return Err(format!(
                    "no {t} group accepts {} (groups: {})",
                    size.map_or_else(String::new, |(w, h)| format!("{w}x{h}")),
                    ids.join(", ")
                ));
            }
            out.extend(fitting.into_iter().map(|g| PlacementRoute {
                placement_type: t.clone(),
                group: g.to_owned(),
                size_ok: true,
            }));
        } else {
            let (group, size_ok) = match fitting.first() {
                Some(g) => (*g, true),
                None => (ids[0], false),
            };
            out.push(PlacementRoute {
                placement_type: t.clone(),
                group: group.to_owned(),
                size_ok,
            });
        }
    }
    Ok(out)
}

/// `referenceName` of an uploaded image: `<file stem> [<sha256[..12]>]`.
pub(crate) fn reference_name(img: &PlanImage) -> String {
    let stem = img
        .file_name
        .rsplit_once('.')
        .map_or(img.file_name.as_str(), |(s, _)| s);
    format!("{stem} {}", marker(&img.sha256))
}

fn marker(sha256: &str) -> String {
    format!("[{}]", &sha256[..sha256.len().min(12)])
}

/// The library image holds the planned image (same marker and size).
pub(crate) fn same_content(li: &LibraryImage, img: &PlanImage) -> bool {
    li.reference_name
        .as_deref()
        .is_some_and(|r| r.ends_with(&marker(&img.sha256)))
        && li.file_size.is_none_or(|s| s == img.bytes)
}

/// Uploaded by Arrangia (its reference name ends with a content marker).
fn has_marker(li: &LibraryImage) -> bool {
    li.reference_name.as_deref().is_some_and(|r| {
        r.strip_suffix(']')
            .and_then(|r| r.rsplit_once(" ["))
            .is_some_and(|(_, h)| h.len() == 12 && h.bytes().all(|b| b.is_ascii_hexdigit()))
    })
}

/// Errors that mean "this app/key has no asset library" (`auto` falls back).
pub(crate) fn unavailable(e: &Error) -> bool {
    matches!(
        e,
        Error::Auth {
            status: Some(403),
            ..
        } | Error::NotFound { .. }
            | Error::Validation { .. }
    )
}

fn no_group_error(message: String) -> Error {
    Error::Validation {
        details: vec![ApiErrorDetail {
            code: NO_PLACEMENT_GROUP.into(),
            title: String::new(),
            detail: message.clone(),
        }],
        message,
    }
}

fn size_of(images: &[PlanImage]) -> Option<(u32, u32)> {
    images.first().map(|i| (i.width, i.height))
}

/// The asset-library flow.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct AssetLibraryFlow;

impl AssetLibraryFlow {
    /// Loads the app's library and the ref data (read-only).
    pub(crate) async fn attach(client: &apple::Client, app_id: &str) -> Result<LibraryContext> {
        let library = client.asset_library(app_id).await?;
        let ref_data = client.ref_data(&RefDataFilter::default()).await?;
        Ok(LibraryContext {
            library_id: library.id,
            ref_data,
            images: Mutex::new(None),
        })
    }

    fn context(target: &AppleTarget) -> Result<&LibraryContext> {
        target.library.as_ref().ok_or_else(|| Error::Config {
            message: "the App Asset Library was not opened".into(),
        })
    }

    fn routes(
        target: &AppleTarget,
        key: &str,
        images: &[PlanImage],
    ) -> Result<Vec<PlacementRoute>> {
        let ctx = Self::context(target)?;
        resolve(&ctx.ref_data, key, size_of(images)).map_err(no_group_error)
    }
}

impl AppleUploadFlow for AssetLibraryFlow {
    async fn open(
        &self,
        client: &apple::Client,
        app: &AppRef,
        platform: Platform,
    ) -> Result<AppleTarget> {
        let mut target = ScreenshotSetFlow.open(client, app, platform).await?;
        target.library = Some(Self::attach(client, &target.app.id).await?);
        Ok(target)
    }

    async fn inspect(
        &self,
        client: &apple::Client,
        target: &AppleTarget,
        locale: &str,
        slot: &str,
        images: &[PlanImage],
    ) -> Result<RemoteSet> {
        let ctx = Self::context(target)?;
        let routes = Self::routes(target, slot, images)?;
        let library = ctx.images(client).await?;
        let in_library = |img: &PlanImage| {
            library
                .iter()
                .any(|l| l.is_usable() && same_content(l, img))
        };
        let creative = slot_from_key(slot).is_creative();
        let mut steps: Vec<&str> = Vec::new();
        let Some(loc) = ScreenshotSetFlow::localization_id(client, target, locale, false).await?
        else {
            let reusable = images.iter().filter(|i| in_library(i)).count();
            steps.push("create_localization");
            if reusable < images.len() {
                steps.push("upload");
            }
            steps.push("create_placements");
            if !creative {
                steps.push("set_order");
            }
            return Ok(RemoteSet {
                steps: steps.into_iter().map(str::to_owned).collect(),
                reusable,
                ..RemoteSet::default()
            });
        };
        let mut same = true;
        let mut count = 0;
        let mut processing = 0;
        let mut placed: Vec<LibraryImage> = Vec::new();
        for r in &routes {
            let existing = client
                .localization_placements(&loc, Some(&r.placement_type), Some(&r.group))
                .await?;
            count = count.max(existing.len());
            let route_same = existing.len() == images.len()
                && existing.iter().zip(images).all(|(p, i)| {
                    p.image
                        .as_ref()
                        .is_some_and(|li| li.is_usable() && same_content(li, i))
                });
            if route_same {
                processing = processing.max(
                    existing
                        .iter()
                        .filter(|p| p.image.as_ref().is_some_and(LibraryImage::is_processing))
                        .count(),
                );
            }
            same &= route_same;
            placed.extend(existing.into_iter().filter_map(|p| p.image));
        }
        let reusable = images
            .iter()
            .filter(|i| in_library(i) || placed.iter().any(|l| l.is_usable() && same_content(l, i)))
            .count();
        if count > 0 {
            steps.push("delete_placements");
        }
        if reusable < images.len() {
            steps.push("upload");
        }
        steps.push("create_placements");
        if !creative {
            steps.push("set_order");
        }
        Ok(RemoteSet {
            steps: steps.into_iter().map(str::to_owned).collect(),
            count,
            same,
            processing: if same { processing } else { 0 },
            reusable,
        })
    }

    #[allow(clippy::too_many_lines, clippy::too_many_arguments)] // one linear flow
    async fn replace(
        &self,
        client: &apple::Client,
        target: &AppleTarget,
        locale: &str,
        slot: &str,
        files: &[UploadFile],
        opts: ReplaceOptions,
        hooks: &dyn SetHooks,
    ) -> Result<ReplaceOutcome> {
        let ctx = Self::context(target)?;
        let images: Vec<PlanImage> = files.iter().map(|f| f.image.clone()).collect();
        let routes = Self::routes(target, slot, &images)?;
        let category = asset_library().slot(&slot_from_key(slot)).map_or_else(
            || apple::library::CATEGORY_SCREENSHOTS.to_owned(),
            |s| s.category,
        );
        let creative = slot_from_key(slot).is_creative();
        let loc = ScreenshotSetFlow::localization_id(client, target, locale, true)
            .await?
            .unwrap_or_default();

        // 1. What the localization shows now, per route.
        let mut existing = Vec::new();
        for r in &routes {
            existing.push(
                client
                    .localization_placements(&loc, Some(&r.placement_type), Some(&r.group))
                    .await?,
            );
        }

        // 2. An asset per file: shown already, in the library, or uploaded.
        let library = ctx.images(client).await?;
        let mut outcome = ReplaceOutcome::default();
        let mut assets: Vec<String> = Vec::with_capacity(files.len());
        let mut pending: Vec<(usize, String, Option<String>)> = Vec::new();
        for (i, f) in files.iter().enumerate() {
            let shown = existing
                .iter()
                .flatten()
                .filter_map(|p| p.image.clone())
                .find(|li| li.is_usable() && same_content(li, &f.image));
            let found = if opts.reuse {
                shown.or_else(|| {
                    library
                        .iter()
                        .find(|li| li.is_usable() && same_content(li, &f.image))
                        .cloned()
                })
            } else {
                None
            };
            if let Some(li) = found.filter(|li| !assets.contains(&li.id)) {
                let processing = !li.is_ready();
                hooks.file_kept(i, &li.id, processing);
                if processing {
                    pending.push((i, li.id.clone(), None));
                }
                assets.push(li.id);
                outcome.kept += 1;
                continue;
            }
            hooks.file_started(i);
            let progress = |p: &arrangia_publish::Progress| hooks.progress(i, p);
            let progress: ProgressFn<'_> = &progress;
            let reference = reference_name(&f.image);
            let img = client
                .upload_library_image_committed(
                    &ctx.library_id,
                    &LibraryUpload {
                        file_name: &f.image.file_name,
                        category: &category,
                        reference_name: Some(&reference),
                    },
                    f.bytes.clone(),
                    progress,
                )
                .await?;
            hooks.file_uploaded(i, &img.id);
            assets.push(img.id.clone());
            pending.push((i, img.id.clone(), None));
            ctx.remember(LibraryImage {
                reference_name: Some(reference),
                file_size: Some(f.image.bytes),
                ..img
            });
        }

        // 3. Poll everything still processing, together, up to the budget.
        let started = Instant::now();
        let mut interval = opts.poll.initial_interval;
        let mut failures: Vec<(usize, String, Vec<ApiErrorDetail>)> = Vec::new();
        let cancel = client.options().cancel.clone();
        while !pending.is_empty() {
            let mut still = Vec::new();
            for (i, id, last) in pending {
                let img = client.library_image(&id).await?;
                let state = img.state.clone().unwrap_or_else(|| "UNKNOWN".into());
                if last.as_deref() != Some(state.as_str()) {
                    hooks.file_state(i, &state, started.elapsed());
                }
                if img.is_ready() {
                    hooks.file_done(i, &id);
                } else if img.is_failed() || matches!(state.as_str(), "REJECTED" | "ARCHIVED") {
                    let msg = if img.state_details.is_empty() {
                        format!("Apple reported {state} without details")
                    } else {
                        img.state_details
                            .iter()
                            .map(ToString::to_string)
                            .collect::<Vec<_>>()
                            .join("; ")
                    };
                    hooks.file_failed(i, &id, &msg);
                    // Apple: delete the failed image and start over.
                    let _ = client.delete_library_image(&id).await;
                    ctx.forget(&id);
                    failures.push((i, id, img.state_details));
                } else {
                    still.push((i, id, Some(state)));
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
        if let Some((_, id, _)) = failures.first() {
            // Nothing on the store page changed; processed uploads stay in
            // the library and are reused by the next run.
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
        outcome.processing = pending.len();
        let processing_ids: Vec<&str> = pending.iter().map(|(_, id, _)| id.as_str()).collect();

        // 4. Placements per route: delete stale, create missing, order, verify.
        let mut stale_images: Vec<LibraryImage> = Vec::new();
        let mut first_ids: Option<Vec<String>> = None;
        for (r, current) in routes.iter().zip(existing) {
            let mut placement_of: Vec<Option<String>> = vec![None; assets.len()];
            let mut stale = Vec::new();
            for p in current {
                let slot_index = assets.iter().enumerate().position(|(i, a)| {
                    placement_of[i].is_none() && p.image_id.as_deref() == Some(a.as_str())
                });
                match slot_index {
                    Some(i) => placement_of[i] = Some(p.id.clone()),
                    None => stale.push(p),
                }
            }
            if !stale.is_empty() {
                hooks.step("delete_placements");
            }
            for p in stale {
                client.delete_placement(&p.id).await?;
                if let Some(img) = p.image
                    && !assets.contains(&img.id)
                    && !stale_images.iter().any(|x| x.id == img.id)
                {
                    stale_images.push(img);
                }
            }
            if placement_of.iter().any(Option::is_none) {
                hooks.step("create_placements");
            }
            for (i, asset) in assets.iter().enumerate() {
                if placement_of[i].is_some() {
                    continue;
                }
                match client
                    .create_placement(&r.placement_type, &r.group, asset, &loc)
                    .await
                {
                    Ok(p) => placement_of[i] = Some(p.id),
                    Err(e)
                        if processing_ids.contains(&asset.as_str())
                            && !matches!(e, Error::Cancelled) =>
                    {
                        outcome.note.get_or_insert_with(|| {
                            format!(
                                "{} could not be placed while Apple is processing it ({e}); the next run places it",
                                files[i].image.file_name
                            )
                        });
                    }
                    Err(e) => return Err(e),
                }
            }
            let order: Vec<String> = placement_of.iter().flatten().cloned().collect();
            if !creative && !order.is_empty() {
                hooks.step("set_order");
                if let Err(e) = client.order_placements(&r.group, &loc, &order).await {
                    if outcome.processing == 0 || matches!(e, Error::Cancelled) {
                        return Err(e);
                    }
                    outcome.note.get_or_insert_with(|| {
                        format!(
                            "the order could not be set while Apple is processing ({e}); check the store state later"
                        )
                    });
                }
            }
            hooks.step("verify");
            let now: Vec<String> = client
                .localization_placements(&loc, Some(&r.placement_type), Some(&r.group))
                .await?
                .into_iter()
                .map(|p| p.id)
                .collect();
            if now != order {
                if outcome.processing == 0 {
                    return Err(Error::Conflict {
                        message: format!(
                            "{} holds {} placement(s) in another order than uploaded ({} expected)",
                            r.group,
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
            first_ids.get_or_insert(order);
        }

        // 5. Old assets Arrangia uploaded: placements are gone, now the asset
        //    (still used by another localization = Apple refuses, fine).
        let old: Vec<&LibraryImage> = stale_images.iter().filter(|i| has_marker(i)).collect();
        if !old.is_empty() {
            hooks.step("delete_old_assets");
        }
        for img in old {
            match client.delete_library_image(&img.id).await {
                Ok(()) => ctx.forget(&img.id),
                Err(e) if apple::is_asset_has_placements(&e) => {}
                Err(Error::Cancelled) => return Err(Error::Cancelled),
                Err(e) => {
                    outcome.note.get_or_insert_with(|| {
                        format!("an old image could not be removed from the asset library ({e})")
                    });
                }
            }
        }
        outcome.ids = first_ids.unwrap_or_default();
        Ok(outcome)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ref_data() -> RefData {
        let spec = |id: &str, w: (u32, u32), h: (u32, u32)| {
            serde_json::json!({"specId": id, "dimensions":
                {"minWidth": w.0, "maxWidth": w.1, "minHeight": h.0, "maxHeight": h.1}})
        };
        serde_json::from_value(serde_json::json!({
            "placementProfileGroups": [
                {"placementProfileGroupId": "DUO_IN", "platform": "IPHONE_APP_STORE", "displayClassId": "IPHONE_DUO"},
                {"placementProfileGroupId": "DUO_OUT", "platform": "IPHONE_APP_STORE", "displayClassId": "IPHONE_DUO"},
                {"placementProfileGroupId": "HEADER_IPHONE", "platform": "IPHONE_APP_STORE", "displayClassId": "DEFAULT"},
                {"placementProfileGroupId": "HEADER_IPAD", "platform": "IPAD_APP_STORE", "displayClassId": "DEFAULT"},
                {"placementProfileGroupId": "HEADER_MAC", "platform": "MAC_APP_STORE", "displayClassId": "DEFAULT"},
                {"placementProfileGroupId": "SEARCH_IPHONE", "platform": "IPHONE_APP_STORE", "displayClassId": "DEFAULT"}
            ],
            "imageSpecs": [
                spec("in", (2007, 2007), (2853, 2853)),
                spec("out", (1398, 1398), (2034, 2034)),
                spec("header", (3840, 3840), (1646, 1646)),
                spec("universal", (5244, 5244), (2950, 2950)),
                spec("search", (1920, 3840), (1280, 2560))
            ],
            "placementTypes": [
                {"placementTypeId": "APP_SCREENSHOT", "specMappings": [
                    {"placementGroupId": "DUO_IN", "specs": ["in"]},
                    {"placementGroupId": "DUO_OUT", "specs": ["out"]}]},
                {"placementTypeId": "PRODUCT_PAGE_HEADER_ASSET", "specMappings": [
                    {"placementGroupId": "HEADER_IPHONE", "specs": ["header", "universal"]},
                    {"placementGroupId": "HEADER_IPAD", "specs": ["header", "universal"]},
                    {"placementGroupId": "HEADER_MAC", "specs": ["header", "universal"]}]},
                {"placementTypeId": "APP_STORE_SEARCH_RESULTS_ASSET", "specMappings": [
                    {"placementGroupId": "SEARCH_IPHONE", "specs": ["search", "universal"]}]}
            ]
        }))
        .unwrap()
    }

    fn groups(r: &[PlacementRoute]) -> Vec<String> {
        r.iter().map(PlacementRoute::label).collect()
    }

    #[test]
    fn routes_follow_the_ref_data() {
        let rd = ref_data();
        // Duo: the size picks the group; no fitting group still routes
        // (Apple decides), flagged.
        let r = resolve(&rd, "APP_IPHONE_DUO", Some((1398, 2034))).unwrap();
        assert_eq!(groups(&r), ["APP_SCREENSHOT → DUO_OUT"]);
        let r = resolve(&rd, "APP_IPHONE_DUO", Some((2007, 2853))).unwrap();
        assert_eq!(groups(&r), ["APP_SCREENSHOT → DUO_IN"]);
        let r = resolve(&rd, "APP_IPHONE_DUO", Some((100, 100))).unwrap();
        assert!(!r[0].size_ok);
        // Creative assets: every group of the iOS platforms (not Mac).
        let r = resolve(&rd, "PRODUCT_PAGE_HEADER_ASSET", Some((3840, 1646))).unwrap();
        assert_eq!(
            groups(&r),
            [
                "PRODUCT_PAGE_HEADER_ASSET → HEADER_IPHONE",
                "PRODUCT_PAGE_HEADER_ASSET → HEADER_IPAD"
            ]
        );
        // A universal asset is placed as header and search results asset.
        let key = ApplePlacement::HeaderAndSearch.api_name();
        let r = resolve(&rd, key, Some((5244, 2950))).unwrap();
        assert_eq!(r.len(), 3);
        assert_eq!(
            r[2].label(),
            "APP_STORE_SEARCH_RESULTS_ASSET → SEARCH_IPHONE"
        );
        // A header at the search size fits no header group.
        assert!(resolve(&rd, "PRODUCT_PAGE_HEADER_ASSET", Some((3000, 2000))).is_err());
        // No group for the display class / no mapping.
        let e = resolve(&rd, "APP_IPHONE_67", Some((1320, 2868))).unwrap_err();
        assert!(e.contains("IPHONE_DYNAMIC_ISLAND_LARGE_DISPLAY"), "{e}");
        assert!(
            resolve(&rd, "APP_NOPE", None)
                .unwrap_err()
                .contains("no App Asset Library mapping")
        );
    }

    #[test]
    fn content_markers() {
        let img = PlanImage {
            index: 1,
            file: "x/01.png".into(),
            file_name: "iphone-en-01_one.png".into(),
            width: 1,
            height: 1,
            bytes: 42,
            sha256: "0123456789abcdef".repeat(4),
            md5: String::new(),
        };
        let name = reference_name(&img);
        assert_eq!(name, "iphone-en-01_one [0123456789ab]");
        let mut li = LibraryImage {
            reference_name: Some(name),
            file_size: Some(42),
            ..LibraryImage::default()
        };
        assert!(same_content(&li, &img) && has_marker(&li));
        li.file_size = Some(41);
        assert!(!same_content(&li, &img));
        li.reference_name = Some("Hand-made".into());
        assert!(!has_marker(&li));
        assert!(asset_library_only("APP_IPHONE_DUO"));
        assert!(asset_library_only("PRODUCT_PAGE_HEADER_ASSET"));
        assert!(!asset_library_only("APP_IPHONE_67"));
        assert_eq!(
            local_label("APP_IPHONE_DUO").as_deref(),
            Some("APP_SCREENSHOT · IPHONE_DUO")
        );
    }
}
