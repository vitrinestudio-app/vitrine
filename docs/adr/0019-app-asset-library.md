# 0019 – App Store Connect App Asset Library upload flow

- Status: accepted (user decision 2026-11-10: the asset library is the
  default; real run with a key pending)
- Date: 2026-11-10
- Amends: ADR 0017 ("Deprecation"), ADR 0018 ("Apple flow abstraction")

## Context

ASC API 4.5.1 deprecated `appScreenshotSets` / `appScreenshots` (ADR 0017).
The successor, the **App Asset Library**, is also the only way to upload
iPhone Duo screenshots (`IPHONE_DUO` exists only as an asset-library
display class; `ScreenshotDisplayType` has no Duo value) and the new
creative assets (product page header, search results asset; research
`docs/research/2026-11-10-appstore-assets-duo-watch.md`). ADR 0018 already
put the Apple upload behind the `AppleUploadFlow` trait so a second flow
changes neither plan, executor nor UI.

## Verified API facts (fetched 2026-11-10)

Sources (Apple documentation JSON under
`developer.apple.com/tutorials/data/documentation/appstoreconnectapi/<slug>.json`):
*Understanding the App Asset Library*, *Migrating to the App Asset
Library*, *Discovering asset specifications*, *Placing assets on your App
Store surfaces*, *Uploading and managing image assets*, *App Store Connect
API 4.5.1 release notes*, and the endpoint pages `get-v1-apps-_id_-assetlibrary`,
`get-v1-appassetlibraryrefdata`, `post-v1-appassetlibraryimages`,
`patch-v1-appassetlibraryimages-_id_`, `get-v1-appassetlibraryimages-_id_`,
`delete-v1-appassetlibraryimages-_id_`, `get-v1-appassetlibraries-_id_-images`,
`get-v1-appstoreversionlocalizations-_id_-placements`,
`post-v1-appassetlibraryplacements`, `delete-v1-appassetlibraryplacements-_id_`,
`post-v1-appassetlibraryplacementorderingrequests`, and the enum pages.

| Step | Call |
|---|---|
| Library | `GET /v1/apps/{id}/assetLibrary` → `{data:{type:"appAssetLibraries", id}}`; one per app, id stable for the app's life |
| Ref data | `GET /v1/appAssetLibraryRefData` (`filter[features\|placementTypes\|placementProfileGroups\|specs]`, `fields[appAssetLibraryRefData]`) → `data[]` of `appAssetLibraryRefData` with six arrays: `features[{featureId, placementPolicies[{placementType, groupLimits[{groupIds, maxCount}]}]}]`, `placementProfileGroups[{placementProfileGroupId, platform, displayClassId}]`, `imageSpecs[{specId, shortName, dimensions{min/maxWidth, min/maxHeight}, aspectRatio, compatiblePlacementTypes, alphaAllowed, fileExtensions, mimeTypes, maxFileSize, universalAsset}]`, `videoSpecs[…, frameRates, duration, audioRequired]`, `placementTypes[{placementTypeId, acceptsAssetCategories, specMappings[{placementGroupId, specs[]}]}]`, `displayClasses[{displayClassId, deviceFamily, screenDimensions[]}]` |
| Reserve image | `POST /v1/appAssetLibraryImages` `{data:{type:"appAssetLibraryImages", attributes:{fileName, fileSize, category, referenceName?}, relationships:{assetLibrary:{data:{type:"appAssetLibraries", id}}}}}` → 201, `state: AWAITING_UPLOAD`, `uploadOperations[]` (same shape as screenshots) |
| Parts | unchanged (pre-signed URLs, no JWT, concurrent, any order) |
| Commit | `PATCH /v1/appAssetLibraryImages/{id}` `{data:{type, id, attributes:{uploaded:true}}}` – **no checksum**; validated against `fileSize` → `UPLOAD_COMPLETE` |
| Poll | `GET /v1/appAssetLibraryImages/{id}` → `state` (`AppAssetLibraryAssetState`: `AWAITING_UPLOAD`, `UPLOAD_COMPLETE`, `FAILED`, `COMPLETE`, `PREPARE_FOR_SUBMISSION`, `READY_FOR_REVIEW`, `WAITING_FOR_REVIEW`, `IN_REVIEW`, `ACCEPTED`, `APPROVED`, `REJECTED`, `ARCHIVED`), `stateDetails[{code, description}]`, then `specId` and `imageAsset{templateUrl, width, height}`. Processed = `PREPARE_FOR_SUBMISSION`; `FAILED` = delete and start over |
| List images | `GET /v1/appAssetLibraries/{id}/images` (`filter[category\|state\|referenceName\|specId\|id]`, `sort`, `limit`) |
| Delete image | `DELETE /v1/appAssetLibraryImages/{id}` → 204; with placements 409 `STATE_ERROR.ASSET_HAS_PLACEMENTS` |
| Placements of a localization | `GET /v1/appStoreVersionLocalizations/{id}/placements?filter[placementType]&filter[placementGroup]&sort=placementGroupPosition&include=image` (the response order is the display order; there is no position attribute) |
| Create placement | `POST /v1/appAssetLibraryPlacements` `{data:{type:"appAssetLibraryPlacements", attributes:{placementType, placementGroup}, relationships:{image:{data:{type:"appAssetLibraryImages", id}}, appStoreVersionLocalization:{data:{…}}}}}` → 201 with `mediaType`, `state` (`AppAssetLibraryPlacementState`: `ASSET_PROCESSING`, `FAILED`, `PARENT_PREPARE_FOR_SUBMISSION` … `PARENT_APPROVED`). Exactly one asset and one surface relationship; placements are immutable (delete + create); the parent must be editable (409 `STATE_ERROR.INVALID_STATE`); a type/group/category mismatch is 409 `ENTITY_ERROR.ATTRIBUTE.INVALID`; a placement of an asset still processing starts in `ASSET_PROCESSING` |
| Delete placement | `DELETE /v1/appAssetLibraryPlacements/{id}` → 204 (the asset stays) |
| Order | `POST /v1/appAssetLibraryPlacementOrderingRequests` `{data:{type, attributes:{placementGroup}, relationships:{orderedPlacements:{data:[…]}, appStoreVersionLocalization:{data:{…}}}}}` → 201; one group of one localization |

Enums: `AppAssetLibraryPlacementType` `APP_SCREENSHOT`,
`IMESSAGE_APP_SCREENSHOT`, `APP_PREVIEW`, `PRODUCT_PAGE_HEADER_ASSET`,
`APP_STORE_SEARCH_RESULTS_ASSET`, `SEARCH_RESULTS_ADS_ASSET`,
`TODAY_TAB_ADS_ASSET`, `EVENT_CARD_ASSET`, `EVENT_DETAILS_PAGE_ASSET`,
`RETENTION_MESSAGE_ASSET`; `AppAssetLibraryAssetCategory`
`APP_SCREENSHOTS_AND_PREVIEWS` | `CREATIVE_ASSETS` (set at reservation,
immutable; each placement type lists `acceptsAssetCategories`);
`AppAssetLibraryDisplayClass` `DEFAULT`, `WATCH_ULTRA`, `WATCH_SERIES_10/7/4/3`,
`IPHONE_DUO`, `IPHONE_DYNAMIC_ISLAND_LARGE/MEDIUM_DISPLAY`,
`IPHONE_FACE_ID_LARGE/MEDIUM_DISPLAY`, `IPHONE_HOME_BUTTON_LARGE/MEDIUM/40/35_DISPLAY`,
`IPAD_13/11/129/105/97_DISPLAY`; `AppAssetLibraryPlacementPlatform`
`IPHONE_APP_STORE`, `IPAD_APP_STORE`, `WATCH_APP_STORE`, `IMESSAGE_APP_STORE`,
`VISION_PRO_APP_STORE`, `MAC_APP_STORE`, `TV_APP_STORE`, `ANY`. Limits for
App Store versions: 10 screenshots, 3 previews, 1 header per group (read
`groupLimits`, not hard-coded). Roles: Account Holder, Admin, App Manager.

**Not documented** (to be read from the real ref data, `arrangia upload
refdata`): the group ids of header/search results assets and whether the
Duo's outer and inner display are separate groups. Placement group ids are
runtime strings ("don't hard-code").

## Decision

**Client** (`arrangia_publish::apple::library`): typed calls for every row
above (`asset_library`, `ref_data` – all `data[]` entries merged, Apple's
camelCase kept for the dump – `library_images`, `library_image`,
`upload_library_image_committed` (reserve → shared part transfer → commit
`{uploaded:true}`; failed transfer deletes the reservation),
`delete_library_image` (404 ok; `is_asset_has_placements`),
`localization_placements` (with included images), `create_placement`,
`delete_placement` (404 ok), `order_placements` (retried like an
idempotent call: repeating it is harmless)); same retry/cancel/observer/
progress rules as ADR 0017. Videos (previews) later.

**Mapping as data** (`crates/core/data/asset-library.yaml`,
`catalog::asset_library`): legacy display type → `placement_type`,
`display_class`, `platforms` (+ `asset_library_only` for `APP_IPHONE_DUO`);
creative placements `header`, `search_results`, `universal` →
placement types + platforms (iPhone, iPad, ANY); placement type → category.
Group ids are **resolved at runtime**: groups of `placementProfileGroups`
with that display class (any for creative) on those platforms ∩ the type's
`specMappings`; several → the one whose image specs accept the image size
(screenshots; none fits → the first, plan warning `placement_size`);
creative assets get **every** fitting group (one header per group).

**Target slot** (model from the catalog work merged first:
`AppleSlot.placement: Option<CreativePlacement>`): `store: {apple:
{display_type: PRODUCT_PAGE_HEADER_ASSET, placement: header}}`,
`{display_type: APP_STORE_SEARCH_RESULTS_ASSET, placement: search_results}`,
`{display_type: PRODUCT_PAGE_HEADER_ASSET, placement: universal}` (presets
`ios-header`, `ios-search-results`, `ios-creative-universal`). A creative
slot's key (plan/log, `AppleSlot::key`) is its placement type(s); the
universal one is `PRODUCT_PAGE_HEADER_ASSET+APP_STORE_SEARCH_RESULTS_ASSET`.
Slots with `placement` and `APP_IPHONE_DUO` never go into the set-based
flow.

**Flow selection** – `store.apple.upload_flow` in `project.yaml`
(project store settings → "Upload method"): `auto` (default, not written),
`asset_library`, `screenshot_sets`.

- Local plan: `flow` = `asset_library` unless `screenshot_sets`; each set
  shows its placement (`APP_SCREENSHOT · IPHONE_DUO`); a slot without
  mapping falls back to sets under `auto` (info `no_placement_mapping`), is
  blocking otherwise. Duo and creative slots never go into a screenshot
  set: with `screenshot_sets` they are **skipped** (action `skip`, warning
  `needs_asset_library`, not rendered) and the other sets still upload.
- Remote plan / run (`remote::open_apple`): app + editable version, then
  the library and the ref data. `auto` falls back to the set API for the
  whole store when the library is not available (403, 404, 400/409/422);
  the plan shows `flow: screenshot_sets` + `flow_note`, and its Duo /
  creative sets are skipped (`needs_asset_library`; a run marks such a set
  `skipped`). Forced
  `asset_library` → blocking `asset_library_unavailable`. Per set
  (`remote::route`): groups resolved → `placement: "APP_SCREENSHOT →
  IPHONE_…_PROFILE"`; no group → under `auto` the set uses the set API
  (`set.flow`, info `flow_fallback`) unless it is library-only (blocking
  `no_placement_group`).

**Asset library flow** (`app::upload::library::AssetLibraryFlow`, second
`AppleUploadFlow`), per set (locale × slot):

1. Localization (created when missing, as before).
2. Current placements per route (type + group).
3. An asset per image: one the localization already shows or the library
   already holds with the same **content marker** is reused; else reserve
   → parts → commit. Library images have no checksum, so Arrangia sets
   `referenceName = "<file stem> [<sha256[..12]>]"` and matches marker +
   file size (one library listing per run; uploads are added, so identical
   images in other locales are uploaded once).
4. Poll all pending assets together within the processing budget (ADR
   0018 b). `FAILED` (also `REJECTED`/`ARCHIVED`): file failed, asset
   deleted, the set fails **before any placement changes** – the store page
   is untouched, processed uploads stay in the library and are reused.
5. Per route: delete stale placements, create the missing ones (also for
   assets still processing; if Apple refuses that, a note and the next run
   places it), ordering request (screenshots; not for creative assets, max
   one per group), verify with `sort=placementGroupPosition`.
6. Images of deleted placements that carry Arrangia's marker are deleted
   **after** their placements; `ASSET_HAS_PLACEMENTS` (still used by another
   localization) is fine; hand-made assets are never deleted.

Steps shown in plan/progress: `create_localization`, `delete_placements`,
`upload`, `create_placements`, `set_order`, `verify`, `delete_old_assets`.
Executor, log, resume, rate limits, watchdog are unchanged (ADR 0018); the
log's per-file store id is the asset id.

**Read-only check** – `upload_refdata {account?, app?}` (GUI "Show App
Store asset types" in the upload dialog, CLI `arrangia upload refdata
[--account ID] [--app ID|BUNDLE] [--format json]`): the full ref data,
the app's library id (or why not), and every Arrangia slot resolved to
its candidate groups with image sizes. Only GETs.

## Consequences

- Duo and header/search assets can be uploaded; screenshots no longer use
  deprecated endpoints when the library is available.
- The mapping file and the size-based group choice are assumptions until
  the user runs `arrangia upload refdata` with the real key (display
  classes of the 6.5"/5.5" classes, header/search group ids, Duo groups).
- Previews (videos), custom product pages, PPO and in-app events are not
  covered (the client is ready for more surfaces).
- Tests: `crates/publish/tests/apple_library.rs` (shapes from Apple's
  examples), `crates/app/tests/upload.rs` (fake library: upload, placement,
  order, Duo group by size, header, reuse + delete order, FAILED, forced
  flows, per-set fallback, 403/404 fallback, refdata read-only),
  `crates/app/src/upload/library.rs` unit tests (routing, markers),
  `crates/cli/tests/upload.rs` + `crates/cli/src/upload.rs` (refdata).
