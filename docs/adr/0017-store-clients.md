# 0017 – Store clients (App Store Connect, Google Play)

- Status: accepted (network clients only; credentials, plan, executor follow
  in Phase 5.1/5.2/5.5)
- Date: 2026-11-10

## Context

Phase 5 (KONZEPT §7) uploads rendered screenshots to App Store Connect and
Google Play. Tasks 5.3/5.4 are the two REST clients in `arrangia-publish`.
The concept asks to verify every endpoint, enum and limit against the
current official docs before implementing. This ADR records what was
verified (all sources fetched **2026-11-10**) and how the clients are built.

## Verified API facts – App Store Connect

Sources (fetched 2026-11-10):

- [Generating tokens for API requests](https://developer.apple.com/documentation/appstoreconnectapi/generating-tokens-for-api-requests)
- [Uploading assets to App Store Connect](https://developer.apple.com/documentation/appstoreconnectapi/uploading-assets-to-app-store-connect)
- [Identifying rate limits](https://developer.apple.com/documentation/appstoreconnectapi/identifying-rate-limits)
- [App Screenshots](https://developer.apple.com/documentation/appstoreconnectapi/app-screenshots),
  [Replace all app screenshots for an app screenshot set](https://developer.apple.com/documentation/appstoreconnectapi/patch-v1-appscreenshotsets-_id_-relationships-appscreenshots),
  [Create an app screenshot set](https://developer.apple.com/documentation/appstoreconnectapi/post-v1-appscreenshotsets)
- [List all App Store versions for an app](https://developer.apple.com/documentation/appstoreconnectapi/get-v1-apps-_id_-appstoreversions),
  [AppVersionState](https://developer.apple.com/documentation/appstoreconnectapi/appversionstate),
  [AppMediaAssetState](https://developer.apple.com/documentation/appstoreconnectapi/appmediaassetstate),
  [ErrorResponse.Errors](https://developer.apple.com/documentation/appstoreconnectapi/errorresponse/errors-data.dictionary)
- [App Store Connect API 4.5.1 release notes](https://developer.apple.com/documentation/appstoreconnectapi/app-store-connect-api-4-5-1-release-notes),
  [Migrating to the App Asset Library](https://developer.apple.com/documentation/appstoreconnectapi/migrating-to-the-app-asset-library)
- [Creating API keys](https://developer.apple.com/documentation/appstoreconnectapi/creating-api-keys-for-app-store-connect-api)

**JWT.** Header `alg: ES256`, `kid: <key id>`, `typ: JWT`. Claims: `iss`
(issuer id, team keys), `iat`, `exp`, `aud: "appstoreconnect-v1"`,
optional `scope`. Individual keys have no issuer id and use `sub: "user"`
instead of `iss`. Tokens with a lifetime > 20 min are rejected (except
GET-only scoped tokens for a few unrelated resources). Apple recommends
reusing a token until it expires. → Client signs 15-minute tokens, caches
them, renews 1 min before expiry and re-signs once on HTTP 401.

**Resources / endpoints used** (base `https://api.appstoreconnect.apple.com`):

| Step | Call |
|---|---|
| App by bundle id | `GET /v1/apps?filter[bundleId]=…` (exact match checked client-side) |
| App by id | `GET /v1/apps/{id}` |
| Versions | `GET /v1/apps/{id}/appStoreVersions?filter[platform]=IOS\|MAC_OS\|TV_OS\|VISION_OS&limit=200` (max `limit` 200) |
| Localizations | `GET /v1/appStoreVersions/{id}/appStoreVersionLocalizations` |
| Sets | `GET /v1/appStoreVersionLocalizations/{id}/appScreenshotSets` |
| Create set | `POST /v1/appScreenshotSets` `{data:{type:"appScreenshotSets", attributes:{screenshotDisplayType}, relationships:{appStoreVersionLocalization:{data:{type,id}}}}}` |
| Screenshots of set (ordered) | `GET /v1/appScreenshotSets/{id}/appScreenshots` |
| Reserve | `POST /v1/appScreenshots` `{data:{type:"appScreenshots", attributes:{fileName, fileSize}, relationships:{appScreenshotSet:{data:{type,id}}}}}` → `uploadOperations[]`, state `AWAITING_UPLOAD` |
| Upload parts | per operation: `method` (PUT), `url` (pre-signed, no JWT, secret, `Expires` query), `length`, `offset`, `requestHeaders[{name,value}]`; body = bytes `offset..offset+length`; parts may go **concurrently and in any order**; a failed part may be resent until commit |
| Commit | `PATCH /v1/appScreenshots/{id}` `{data:{type,id,attributes:{uploaded:true, sourceFileChecksum:<MD5 hex of the whole file>}}}`; byte count must match `fileSize` |
| Poll | `GET /v1/appScreenshots/{id}` → `assetDeliveryState{state, errors[], warnings[]}`; states `AWAITING_UPLOAD`, `UPLOAD_COMPLETE`, `COMPLETE` (final), `FAILED` (final: delete and restart). No interval documented; processing takes seconds to minutes |
| Delete | `DELETE /v1/appScreenshots/{id}` |
| Order | `PATCH /v1/appScreenshotSets/{id}/relationships/appScreenshots` `{data:[{type:"appScreenshots",id}…]}` → 204 |

Reservations expire after about a week. Version states: the
`appStoreState` attribute/filter (`AppStoreVersionState`) is deprecated
since API 3.7; use `appVersionState` (`AppVersionState`: ACCEPTED,
DEVELOPER_REJECTED, IN_REVIEW, INVALID_BINARY, METADATA_REJECTED,
PENDING_APPLE_RELEASE, PENDING_DEVELOPER_RELEASE, PREPARE_FOR_SUBMISSION,
PROCESSING_FOR_DISTRIBUTION, READY_FOR_DISTRIBUTION, READY_FOR_REVIEW,
REJECTED, REPLACED_WITH_NEW_VERSION, WAITING_FOR_EXPORT_COMPLIANCE,
WAITING_FOR_REVIEW). Apple documents **no** list of "editable" states; the
client treats `PREPARE_FOR_SUBMISSION`, `DEVELOPER_REJECTED`, `REJECTED`,
`METADATA_REJECTED`, `INVALID_BINARY` as editable (the states in which the
version page is editable in the web UI; to be confirmed in the real run).

**Errors.** `{errors:[{id, status, code, title, detail, source, meta}]}`;
`code` is machine-readable (`STATE_ERROR…`, `ENTITY_ERROR…`,
`PARAMETER_ERROR…`, `NOT_AUTHORIZED`, `RATE_LIMIT_EXCEEDED`), `title`/
`detail` are for humans only. Statuses: 400, 401, 403, 404, 409 ("provided
resource data is not valid" – Apple uses 409 for both state and entity
errors), 422, 429.

**Rate limits.** Every response carries `X-Rate-Limit:
user-hour-lim:3500;user-hour-rem:500;` (rolling hour, per API key; actual
limits vary). Exceeding → HTTP 429 `RATE_LIMIT_EXCEEDED`. Apple documents
**no** `Retry-After` header; advice is to queue the job and retry later.

### Deprecation (differs from KONZEPT §7)

API **4.5.1** deprecated the set-based media resources: *App Screenshot
Sets, App Screenshots, App Preview Sets, App Previews* (availability now
"1.2.0 – 4.5.0"). The replacement is the **App Asset Library**:
`GET /v1/apps/{id}/assetLibrary` → `POST /v1/appAssetLibraryImages`
(reserve, same `uploadOperations`, commit with `uploaded: true` only – **no
checksum**) → `POST /v1/appAssetLibraryPlacements` (`placementType`,
`placementGroup` such as `IPHONE_DYNAMIC_ISLAND_LARGE_PROFILE`, asset,
target localization) → `POST /v1/appAssetLibraryPlacementOrderingRequests`
for the order. Display types become runtime lookups from
`GET /v1/appAssetLibraryRefData` (placement groups, dimensions; Apple adds
device families such as `IPHONE_DUO` there without API version changes).
Assets with placements cannot be deleted (`STATE_ERROR.ASSET_HAS_PLACEMENTS`);
placements first. No removal date for the old endpoints is published.

Decision for now: implement the **set-based flow** as planned (it is still
documented, matches the display types verified in the ASC web app on
2026-11-09 in `targets.yaml`, and is what the existing tooling uses), and
keep the client API (`find_app`/`editable_version`/`localizations`/upload/
reorder) narrow enough that an asset-library backend can be added behind
the same executor steps. Open question for the user/coordinator: switch to
(or add) the asset-library flow before the real run? It is required for
`IPHONE_DUO` and avoids the deprecated endpoints.

## Verified API facts – Google Play (Android Publisher v3)

Sources (fetched 2026-11-10):

- [Using OAuth 2.0 for server to server applications](https://developers.google.com/identity/protocols/oauth2/service-account)
- [edits.insert](https://developers.google.com/android-publisher/api-ref/rest/v3/edits/insert),
  [edits.commit](https://developers.google.com/android-publisher/api-ref/rest/v3/edits/commit),
  [edits.images](https://developers.google.com/android-publisher/api-ref/rest/v3/edits.images)
  (`list`, `deleteall`, `upload`), [AppImageType](https://developers.google.com/android-publisher/api-ref/rest/v3/AppImageType)
- [Upload media](https://developers.google.com/android-publisher/upload)
- Discovery document `https://androidpublisher.googleapis.com/$discovery/rest?version=v3`
  (revision 20261110) for `mediaUpload` limits and response schemas
- [Getting started](https://developers.google.com/android-publisher/getting_started)

**Token.** JWT header `alg: RS256`, `typ: JWT`, optional `kid` (service
account key id). Claims `iss` (service account e-mail), `scope`
(`https://www.googleapis.com/auth/androidpublisher`), `aud`
(`https://oauth2.googleapis.com/token`, always), `iat`, `exp` (≤ 1 h).
`POST https://oauth2.googleapis.com/token`, form
`grant_type=urn:ietf:params:oauth:grant-type:jwt-bearer&assertion=<jwt>` →
`{access_token, expires_in (3600), token_type: "Bearer"}`; errors
`{error: "invalid_grant", error_description}`. → Client caches the access
token until 60 s before `expires_in`, refetches once on HTTP 401, and
ignores `token_uri` from the JSON file (the configured token URL is used, so
a pasted file cannot redirect the signed assertion).

**Endpoints** (base `https://androidpublisher.googleapis.com`,
`P = /androidpublisher/v3/applications/{packageName}`):

| Call | Request | Response |
|---|---|---|
| `edits.insert` | `POST P/edits` (body `AppEdit`, `{}`) | `AppEdit{id, expiryTimeSeconds}` |
| `edits.images.list` | `GET P/edits/{editId}/listings/{language}/{imageType}` | `{images:[Image]}` |
| `edits.images.deleteall` | `DELETE P/edits/{editId}/listings/{language}/{imageType}` | `{deleted:[Image]}` (empty response when nothing matched) |
| `edits.images.upload` | `POST /upload` + `P/edits/{editId}/listings/{language}/{imageType}?uploadType=media`, body = image bytes, `Content-Type` = image MIME | `{image: Image}` |
| `edits.commit` | `POST P/edits/{editId}:commit[?changesNotSentForReview=true]` (also `changesInReviewBehavior`: `CANCEL_IN_REVIEW_AND_SUBMIT`/`ERROR_IF_IN_REVIEW`, not used yet) | `AppEdit` |
| `edits.delete` | `DELETE P/edits/{editId}` | empty |

`Image{id, url, sha1, sha256}`. Upload: `mediaUpload.accept: ["image/*"]`,
`maxSize: 15728640` (15 MiB); `uploadType=media` is meant for files of
about ≤ 5 MB (`multipart`/`resumable` exist for larger files – screenshots
are far smaller). Unsupported languages are a no-op for upload.
`AppImageType`: `phoneScreenshots`, `sevenInchScreenshots`,
`tenInchScreenshots`, `tvScreenshots`, `wearScreenshots`, `icon`,
`featureGraphic`, `tvBanner` (KONZEPT §7.3 lists a subset; matches). Images
appear in upload order. `changesNotSentForReview`: "when a rejection
happens, the parameter will make sure that the changes in this edit won't
be reviewed" (Play rejects a commit without it for apps whose changes are
not sent for review automatically). Errors: `{error:{code, message,
status, errors:[{reason, message}]}}`.

## Decision – client design

- `arrangia_publish::apple::Client` / `google::Client`, async (tokio),
  plain `reqwest` (rustls, features `query`, `form`, `json`) and
  `jsonwebtoken` with the `aws_lc_rs` backend (the same crypto library
  rustls already uses). Configs hold key material only in memory
  (`Zeroizing<String>`, redacted `Debug`); the caller loads it from the
  keychain (5.1). Base URLs (and Google's token URL) are overridable for
  tests. Bearer tokens are only sent to the configured API origin
  (pagination links to other hosts are refused); upload parts go to Apple's
  pre-signed URLs without a token.
- **Errors** (`Error`, `ErrorKind`): `Auth` (401/403, invalid grant),
  `NotFound`, `Conflict` (Apple `STATE_ERROR*`, no editable version, Google
  `FAILED_PRECONDITION`/409), `RateLimited{retry_after}`, `Validation`
  (with `ApiErrorDetail{code,title,detail}`), `Processing` (asset `FAILED`),
  `Transport` (network, 5xx), `Timeout`, `Cancelled`, `Config`.
  `is_transient()` tells the executor which steps can be rescheduled.
- **Retries** (`RetryPolicy`, default 5 attempts, 0.5 s doubling to 30 s,
  equal jitter): idempotent calls (GET/PUT/PATCH/DELETE, upload parts, token)
  retry 429, 500/502/503/504 and network errors; creating POSTs (set,
  reservation, edit, Google image upload, commit) retry only 429, 503 and
  connection failures – a lost response must not create duplicates. A
  `Retry-After` (delta seconds) is honoured; longer than
  `max_retry_after` (default 120 s) → `RateLimited` immediately so the
  executor can queue the job (Apple's advice).
- **Cancellation**: `ClientOptions.cancel` (`tokio_util`
  `CancellationToken`) aborts in-flight requests and waits.
- **Logging hook**: `HttpObserver` receives `HttpEvent{method, url,
  attempt, kind}` (request, response with status/elapsed, network error,
  retry scheduled). URLs are redacted to scheme/host/path (Apple's upload
  URLs carry signatures in the query); headers and bodies are never passed.
- **Progress**: `Progress{step: UploadStep(Reserve|Transfer|Commit|
  Processing|Done), asset_id, bytes_sent, bytes_total, parts, state}` via a
  `&dyn Fn(&Progress)` callback, reported per part (Apple) / per image
  (Google). The executor reuses these types.
- **Apple upload**: reserve → parts with `part_concurrency` (default 4,
  `JoinSet`, `Bytes` slices, `requestHeaders` applied verbatim) → commit
  with MD5 → poll with `PollPolicy` (2 s doubling to 15 s, timeout 10 min).
  On a failed transfer/commit or `FAILED` processing the reservation is
  deleted (best effort) as Apple recommends; on cancel/timeout it is left
  (visible in `list_screenshots` with its state; resume cleans up).
  `delete_screenshot`/`delete_edit` treat 404 as success (resumable).
- Tests: `wiremock` only (no real network); throwaway EC P-256/RSA keys are
  generated at test time with aws-lc-rs, and mock matchers verify the JWT
  signatures and claims.

## Consequences

- No keychain, config, plan or executor code here (5.1/5.2/5.5).
- Apple's set-based endpoints are deprecated; see the open question above
  (also in `docs/plan/STATUS.md`, "Open questions for the user").
- Real behaviour (editable states, processing times, Play review flag)
  is confirmed only in the user's real run (5.6).

## Amendment 2026-11-10 – connection test

Two read-only calls were added for "Test connection" (ADR 0016,
`arrangia_app::store_test`): `apple::Client::apps_overview`
(`GET /v1/apps?fields[apps]=bundleId,name&limit=1` → `meta.paging.total`
+ first app) and `google::Client::authenticate` (token fetch only). The
Google package name is now checked when an edit URL is built instead of in
`Client::new`, so a token-only client needs none. The asset-library
question is listed in `docs/plan/STATUS.md` → "Open questions for the user".
