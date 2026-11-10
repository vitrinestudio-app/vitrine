# App Store research, 2026-11-10: Header and Search results assets, iPhone Duo, Apple Watch / Wear OS

All sources fetched on 2026-11-10. Apple help pages carry no "last updated" date, only "© 2026", so treat them as "verified 2026-11-10".
Apple doc JSON was read through `https://developer.apple.com/tutorials/data/documentation/appstoreconnectapi/<slug>.json` because the HTML pages render with JS.

---

## 1. Creative assets: "Product page header" (Kopfzeile) and "Search results" (Suchergebnisse)

### What they are and where they appear
- These are "creative assets": rich images or videos showing the app's brand, value or excitement. Apple keeps them "distinct from the in-use functionality requirement in app previews and screenshots". They are **optional** and separate from screenshots and previews. (ASC Help, *Manage your App Store assets*.)
- **Product page header:** the first visual on the product page. Use it for brand, seasonal offers or new content. Guidance: "single, clear idea", design for a first-time visitor.
- **Search results asset:** the first asset in Search tab results. Guidance: "State the obvious", "Showcase the firsthand experience". If none is set, In-App Events, previews and screenshots appear as before.
- They are shown only on **iOS 27 / iPadOS 27 and later**, for iOS and iPadOS apps. No Mac, TV, Watch or Vision placement is mentioned.
- Timeline:
  - WWDC26 (June 2026): announced (session https://developer.apple.com/videos/play/wwdc2026/205/).
  - **2026-10-05:** enabled in ASC (news https://developer.apple.com/news/?id=ljpl7kyn). The same post introduced the ASC Asset Library and the product page preview tool, which supports Dark Mode, devices and orientations.
- A **universal asset** (16:9 PNG at 5244×2950) can serve both placements ("Use header asset in search results"). Otherwise each placement gets its own asset.
- Upload paths in ASC: the version page (Product Page Information, then the "Header and Search Results" tab), Asset Library (standalone submission, reviewed against the latest app version), Custom Product Pages, Product Page Optimization (its own Header/Search Results tab), and In-App Events.
- Roles:
  - ASC UI: Account Holder, Admin, App Manager or Marketing.
  - API: ACCOUNT_HOLDER, ADMIN or APP_MANAGER.

### Specifications
Source: https://developer.apple.com/help/app-store-connect/reference/app-information/creative-assets-specifications

Images (no alpha or transparency):

| Placement | Ratio | Pixels | Format |
|---|---|---|---|
| Product page header | 21:9 | 3840 × 1646 (exact) | jpeg/jpg/png |
| Product page header | 16:9 (universal) | 5244 × 2950 | png only |
| Search results | 3:2 | min 1920 × 1280, max 3840 × 2560 | jpeg/jpg/png |
| Search results | 16:9 (universal) | 5244 × 2950 | png only |

Video (header 21:9 at 3840×1646; search 3:2 between 1920×1280 and 3840×2560):
- 30 or 60 fps, 5 to 30 s, .mov/.m4v/.mp4, looping.
- Muted by default. The user can unmute on the product page, but not in search results.

In-App Event media for comparison:
- Card: 16:9, 1920×1080 to 3840×2160.
- Details page: 9:16, 1080×1920 to 2160×3840.
- Images up to 500 MB. Video 15 to 30 s.

### Safe area and text rules
Sources: https://developer.apple.com/app-store/asset-best-practices/ and the 2.3.3 help page.
- **Safe area:** Apple gives no numeric safe area. It is "defined in each template" (Figma Universal; Photoshop/Pixelmator for Header, Search results and Universal; Sketch Universal). Apple says to keep key art inside the safe area and the focal point in the centre, because the crop varies by device and orientation. **Gap: Arrangia would need the template geometry, which is not published as numbers.**
- **Text:** "a short phrase that enhances your visual rather than describes it". Localize it for all supported languages.
- **Not allowed:**
  - prices or discounts, URLs, the © symbol, unverifiable awards
  - other platforms or marketplaces
  - Apple recognitions (Editors' Choice, App of the Day, ADA)
- **Age rating:** must suit 4+, even if the app is rated higher.
- **Localization:** per localization. Placements hang on localizations (see API below).
- **Counts per App Store version:** 1 header per placement group. Screenshots up to 10, previews up to 3. Search shows up to 3 screenshots or previews depending on orientation. Custom product pages and PPO have their own limits, which you read from the reference data.

### API: App Asset Library (ASC API 4.5 introduced the types; 4.5.1 release notes)
- 4.5.1 release notes: https://developer.apple.com/documentation/appstoreconnectapi/app-store-connect-api-4-5-1-release-notes. No explicit date; the enum types say "introducedAt 4.5", and docs examples use 2026-08 dates.
- Release 4.5.1 says:
  - **Added:** App Asset Library images, videos, placements, placement ordering requests and ref data. Also the `IPHONE_DUO` display class.
  - **Deprecated:** `appScreenshotSets`, `appScreenshots`, `appPreviewSets`, `appPreviews`, `appEventScreenshots`, `appEventVideoClips` and the custom product page screenshot/preview sets. Migration guide: https://developer.apple.com/documentation/appstoreconnectapi/migrating-to-the-app-asset-library
- **Flow** (https://developer.apple.com/documentation/appstoreconnectapi/understanding-the-app-asset-library):
  1. `GET /v1/apps/{id}/assetLibrary` returns the library id (stable for the life of the app).
  2. `GET /v1/appAssetLibraryRefData` returns the catalog:
     - six arrays: `features`, `placementProfileGroups`, `imageSpecs`, `videoSpecs`, `placementTypes`, `displayClasses`
     - filters: `filter[features|placementTypes|placementProfileGroups|specs]`
  3. `POST /v1/appAssetLibraryImages` (or `…Videos`) reserves the asset with `fileName`, `fileSize`, `category` (`APP_SCREENSHOTS_AND_PREVIEWS` | `CREATIVE_ASSETS`, cannot be changed later), an optional `referenceName`, and a relationship to `assetLibrary`.
  4. Upload the parts with `uploadOperations`, as before.
  5. `PATCH` with `{uploaded: true}` only. **No MD5 / `sourceFileChecksum` any more.** The commit is validated against `fileSize`.
  6. Poll the state: `AWAITING_UPLOAD` → `UPLOAD_COMPLETE` → `PREPARE_FOR_SUBMISSION` (or `FAILED` with `stateDetails`). ASC then sets `specId` (the matched specification) and `imageAsset.templateUrl`.
  7. `POST /v1/appAssetLibraryPlacements` with `placementType`, `placementGroup`, exactly one of `image`/`video`, and exactly one of `appStoreVersionLocalization`, `appCustomProductPageLocalization`, `appEventLocalization` or `appStoreVersionExperimentTreatmentLocalization`. The parent surface must be editable. Placements cannot be modified: delete and re-create them.
  8. `POST /v1/appAssetLibraryPlacementOrderingRequests` with `placementGroup` and `orderedPlacements`, related to the localization. Read back with `GET …Localizations/{id}/placements?sort=placementGroupPosition`.
  9. Delete order: placements first, then the asset (`STATE_ERROR.ASSET_HAS_PLACEMENTS`). Archive works only on approved assets (409 otherwise).
- **`AppAssetLibraryPlacementType`:** `APP_SCREENSHOT`, `IMESSAGE_APP_SCREENSHOT`, `APP_PREVIEW`, **`PRODUCT_PAGE_HEADER_ASSET`**, **`APP_STORE_SEARCH_RESULTS_ASSET`**, `SEARCH_RESULTS_ADS_ASSET`, `TODAY_TAB_ADS_ASSET`, `EVENT_CARD_ASSET`, `EVENT_DETAILS_PAGE_ASSET`, `RETENTION_MESSAGE_ASSET`. So **header and search results assets can be uploaded via the API, only through the Asset Library flow.** There is no legacy endpoint for them.
- **Universal assets:** a spec with `universalAsset: true` lists its `compatiblePlacementTypes`. Upload once, then create one placement per type.
- **Placement groups** are runtime strings, e.g. `IPHONE_DYNAMIC_ISLAND_LARGE_PROFILE`, `MAC_PROFILE`, `IMESSAGE_IPHONE_DYNAMIC_ISLAND_LARGE_PROFILE`. Apple: "don't hard-code"; new device families arrive without an API version bump. **Uncertain:** the group ids for header and search assets (probably per platform, e.g. iPhone/iPad) are not in the public docs. You have to read them from refdata with a real key.
- **`AppAssetLibraryDisplayClass`:** `DEFAULT`, `WATCH_ULTRA`, `WATCH_SERIES_10`, `WATCH_SERIES_7`, `WATCH_SERIES_4`, `WATCH_SERIES_3`, **`IPHONE_DUO`**, `IPHONE_DYNAMIC_ISLAND_LARGE_DISPLAY`/`_MEDIUM_DISPLAY`, `IPHONE_FACE_ID_LARGE`/`_MEDIUM_DISPLAY`, `IPHONE_HOME_BUTTON_LARGE`/`MEDIUM`/`40`/`35_DISPLAY`, `IPAD_13`/`11`/`129`/`105`/`97_DISPLAY`.
- **`AppAssetLibraryPlacementPlatform`:** `IPHONE_APP_STORE`, `IPAD_APP_STORE`, `WATCH_APP_STORE`, `IMESSAGE_APP_STORE`, `VISION_PRO_APP_STORE`, `MAC_APP_STORE`, `TV_APP_STORE`, `ANY`.
- The legacy enum `ScreenshotDisplayType` still has **no Duo value** (checked today; still `APP_IPHONE_67` … `APP_APPLE_VISION_PRO`). **Consequence for ADR 0018: Duo screenshots, headers and search assets can only be uploaded through the App Asset Library.** `ScreenshotSetFlow` cannot do it.

---

## 2. iPhone Duo

Sources:
- news https://developer.apple.com/news/?id=kkphp5qo (2026-10-05)
- https://developer.apple.com/iphone-duo/ and /iphone-duo/prepare/
- HIG https://developer.apple.com/design/human-interface-guidelines/designing-for-iphone-duo
- tech specs https://www.apple.com/iphone-duo/specs/

- **Device:** a book-style foldable with a vertical centre hinge.
  - Outer display: compact, "wider and shorter" than other iPhones. Size class compact width.
  - Inner display: large. Size class regular width.
  - Both displays have a front camera. The outer camera sits "in the corner" and is always visible. The inner camera is under the display and hidden unless active.
  - Poses: closed, open, partially folded like a book, laptop-style on a table, standing on its edges. There is a reserved folding region in the centre when partially open.
  - With the iOS 27.1 SDK, toolbars and tab bars run **vertically on the side**: on the outer display, and on the inner display in landscape.
- **Availability:** customers from **2026-10-23**. Dev support from Xcode 27.1. Uploads in ASC opened 2026-10-05.
- **Hardware** (apple.com specs):

  | | Display | Pixels | Density |
  |---|---|---|---|
  | Inner | 7.6" OLED folding | 1878 × 2670 | 430 ppi |
  | Outer | 5.4" | 1398 × 2034 | 460 ppi |

  - Size open: 164.6 mm wide × 117.8 mm high × 5.2 mm deep.
  - Size closed: 84.1 × 117.8 × 11.3 mm.
  - Weight: 254 g. Finishes: Night Sky, Star White. Apple says the displays have rounded corners within a rectangle.
  - Corner radii, bezel widths, camera and hinge positions are **not published as numbers**. A parametric frame has to estimate them; the dimensions above give the aspect ratios.
  - Open size 164.6 × 117.8 is landscape, so the inner display's natural orientation is landscape.
- **Screenshot sizes** (ASC Help, screenshot specifications, verified verbatim today):

  | Display | Portrait | Landscape | Note |
  |---|---|---|---|
  | Outer | 1398 × 2034 | 2034 × 1398 | equals native resolution |
  | Inner | 2007 × 2853 | 2853 × 2007 | 951 × 669 pt @3x; **larger than the native 1878 × 2670**, same aspect ≈ 1.4216 |

  - Notes:
    - "Starting in April 2027, screenshots will be required for any app using the iOS 27.1 SDK or later."
    - The news post says more broadly: "Starting April 2027, any apps or games submitted will need to include screenshots for iPhone Duo."
    - **Wording conflict:** all apps or only 27.1-SDK apps. The help page is more specific.
  - Submitting Duo screenshots shows a "Made for iPhone Duo" badge on the product page.
  - **Uncertain:** whether outer and inner screenshots are separate placement groups (probably), whether both are required, and how the API distinguishes them. Only the one display class `IPHONE_DUO` is documented. Read from refdata.
- In ASC the Duo is a group within "iPhone". A Figma forum report says Figma's preset (890 × 626) differs from Apple's kit (951 × 669).
- **HIG for marketing:** no Duo-specific marketing rules. The best-practices page only says "assets optimized for iPhone Duo". The preview tool in ASC shows Duo.
- Apple **does publish an "iPhone Duo" Product Bezel** (Photoshop/PNG, `Bezel-iPhone-Duo.dmg`) on https://developer.apple.com/design/resources/. Relevant for the user import (ADR 0012), not for the repo.

---

## 3. Apple Watch and Wear OS

### Apple (ASC screenshot specifications, verbatim)
"Required for Apple Watch apps". Use **one** of the sizes below, and the **same size across all localizations**:

| Size | Devices |
|---|---|
| 422 × 514 | Ultra 4, Ultra 3 |
| 410 × 502 | Ultra 2, Ultra |
| 416 × 496 | Series 12, 11, 10 |
| 396 × 484 | Series 9, 8, 7 |
| 368 × 448 | Series 6, 5, 4, SE 3, SE 2, SE |
| 312 × 390 | Series 3, 2, 1 |

- The API display classes are `WATCH_ULTRA` (covers both Ultra sizes), `WATCH_SERIES_10`, `WATCH_SERIES_7`, `WATCH_SERIES_4` and `WATCH_SERIES_3`. The legacy enum has `APP_WATCH_*` equivalents.
- The default design resources now include Product Bezels for Ultra 4 (2026) and Series 11.
- **Framing:** Apple publishes **no official rule** on watch bezels. The exact screen-pixel sizes and community or DTS-adjacent forum answers (thread 815829 and others) suggest that watch screenshots should be **unframed screen captures**, because the store renders them in its own watch mask. Text or marketing on watch shots is risky. Guideline 2.3.3 (more than half of screenshots must show the app in use; overlays allowed) applies generally. **Recommendation:** export watch targets unframed, as the full screen at the exact size.

### Google Play, Wear OS (Play Console Help 9866151, verified today)
Source: https://support.google.com/googleplay/android-developer/answer/9866151
- **1:1, at least 384 × 384 px**. General maximum 3840 px per side. JPEG or 24-bit PNG without alpha. At least 1, up to 8 per device type.
- "Provide screenshots showing only the app interface". **No device frames**, no extra text, graphics or backgrounds, **no transparency or masking**. Must show the current Wear OS version. A Tiles screenshot is recommended.
- Watch faces: same rules. If customizable, show several permutations.
