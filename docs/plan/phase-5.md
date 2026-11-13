# Phase 5 – Store upload: task list

Scope: KONZEPT.md §7 and §12, Phase 5. Done when uploading into a test app
works for both stores (App Store Connect first, then Google Play), including
screenshot order.

Status markers: `[ ]` open · `[~]` in progress · `[x]` done

## Principles (from the concept, binding)
- Crate `vitrine-publish`: direct REST calls (`reqwest` + rustls), JWT via
  `jsonwebtoken`, no vendor SDKs; `core` stays free of network code.
- **Plan → show → confirm → execute.** Never upload without a plan and an
  explicit confirmation – also not via MCP (confirm token from the plan).
- Secrets only in the system keychain (`keyring`); configs store references.
  The user imports keys themselves (file picker / paste in the GUI); Claude
  never handles key material.
- Upload log + resume in `.cache/uploads.sqlite` (per project); hashes decide
  what changed since the last upload.
- Tests against recorded responses (`wiremock`); the real run is done by the
  user against a test app.
- Verify every endpoint, enum and limit against the current official API
  docs before implementing; note source + date in the ADR.

## 5.1 Credentials & config (done 2026-11-10, ADR 0016)
- [x] Store accounts in app settings (app-wide, not in the project):
      Apple (issuer id, key id, `.p8` → keychain), Google (service account JSON →
      keychain); list/test/remove; "test connection" call (seam
      `ConnectionTester`, wired 2026-11-10 with the clients: Apple
      `GET /v1/apps?limit=1`, Google token fetch; read-only; GUI + dev bridge)
- [x] Project ↔ store mapping in `project.yaml`: Apple app id / bundle id,
      Google package name; language → store locale map (`de` → `de-DE`,
      configurable, defaults per store in `data/store-locales.yaml`)
- [x] Target ↔ store slot already in targets (`store: {apple: {display_type}}`,
      `google: {image_type}`) – validate completeness (`store_no_slot`,
      `store_slot_duplicate`, locale findings)

## 5.2 Plan (dry run) (done 2026-11-10, ADR 0018)
- [x] For each target × language: which store set gets replaced, which images
      changed (hash vs last upload / vs store state), store-rule checks
      (count, size, format), editable app version / edit state check
- [x] Plan object with a confirm token (hash of the plan + expiry); shown in
      CLI/GUI/MCP

## 5.3 App Store Connect client
Client part done 2026-11-10 (`vitrine_publish::apple`, ADR 0017). Apple
deprecated the set-based screenshot API in 4.5.1 (App Asset Library
replaces it) – open question in ADR 0017.
- [x] JWT ES256 (15 min, max 20; team + individual keys), apps → editable
      `appStoreVersion` → `appStoreVersionLocalizations` →
      `appScreenshotSets` per display type
- [x] Replace set: delete old screenshots, reserve (`appScreenshots` POST with
      file size/name), upload parts per `uploadOperations`, commit with MD5
      checksum, poll `assetDeliveryState`, set order
      (`appScreenshotSets/{id}/relationships/appScreenshots` PATCH)
- [x] Rate limits / retries / resume after abort; log every step – client
      side done (retry/backoff/Retry-After, cancel, observer hook, progress
      events, idempotent deletes); resume + upload log done in 5.5
- [x] App Asset Library backend (ADR 0019, 2026-11-10; `auto` uses it
      only for Duo/creative slots, `upload_flow: asset_library` for all):
      library, ref data, images (commit without checksum), placements,
      ordering requests; `store.apple.upload_flow` auto/asset_library/
      screenshot_sets; header/search results placements
      (catalog presets `ios-header`, `ios-search-results`,
      `ios-creative-universal`: slot `placement`); Duo; the set-based
      flow never gets `placement` slots or `APP_IPHONE_DUO`; reuse by content marker;
      `upload refdata` (CLI + GUI "Show App Store asset types")
- [ ] User checks the real ref data once (`vitrine upload refdata`) and the
      mapping in `data/asset-library.yaml` (display classes, header/search
      groups, Duo inner/outer groups) – still open 2026-11-13 (Phase 7)

## 5.4 Google Play client
Client part done 2026-11-10 (`vitrine_publish::google`, ADR 0017).
- [x] OAuth2 JWT bearer (service account) → access token
- [x] `edits.insert` → per language & image type `edits.images.deleteall` →
      `edits.images.upload` (order = upload order) → `edits.commit`
      (with `changesNotSentForReview` option where required); abort = delete edit

## 5.5 Execute + UI (done 2026-11-10, ADR 0018; GUI not yet checked visually)
- [x] Executor with progress events, cancel, resume, upload log
- [x] CLI: `vitrine upload plan [--store apple|google] [--target] [--lang]`,
      `vitrine upload execute --confirm <token>`, `vitrine upload log`
- [x] GUI: "Upload…" dialog (plan table: store/target/language/changes/rule
      problems → confirm → progress → result/log) + MCP confirmation dialog
- [x] MCP: `upload_plan`, `upload_execute {confirm_token}`, `upload_status`,
      `upload_log`; GUI-hosted needs the user's click, headless off unless
      `mcp_allow_headless_upload` (recommended default implemented; user
      decision (b) still to confirm)

## 5.5b Store setup UX (done 2026-11-10, user requests; ADR 0016/0018 amendments)
- [x] "How do I get this?" help panels (Apple team key, Google service
      account) in app settings → Stores and the import dialog; help links via
      `open_url` (allowlist only)
- [x] Upload dialog: setup checklist before the plan (`upload_readiness`,
      `plan.readiness`, codes + `fix`), buttons open app settings → Stores /
      project settings → Stores / targets and re-check; MCP hints
- [x] App picker (App Store): `apple::Client::list_apps` (paginated) +
      `store_apps_list` (session cache) → fills Apple ID + bundle id
- [x] Google Play: no list endpoint (verified) → typed package name +
      "Check" (`store_package_check`: temporary edit, deleted, explicit click)
- [ ] Look at it in the real app (dev bridge checked with curl only) –
      Phase 7 checklist

## 5.5c First real Apple upload fixes (2026-11-10, ADR 0018 amendment b)
- [x] Run hung for 10 min per slow image (per-image poll, timeout failed the
      set), no progress while polling, events with one flush timestamp
      → upload all images of a set, then poll together (budget 15 min);
      still processing = `uploaded_processing`, not a failure; `FAILED` with
      Apple's errors; reuse of images already in the set (checksum)
- [x] Watchdog (heartbeat, stall → cancel/fail, panic safety net),
      stale `running` runs → `interrupted` on log open
- [x] Event time per event, poll requests logged as state transitions only
- [x] Plan: `uploaded_processing` → unchanged (info), remote: `processing` /
      `reusable` counts; GUI "Check processing", per-image step text
- [x] "No editable App Store version": short message with suggestion,
      once per store (plan problem + readiness step + link; run:
      `store_failures`, sets skipped)
- [ ] User re-runs the real upload with the fixes (owner testing,
      Phase 7 checklist)

## 5.6 Tests & real run
- [~] wiremock tests for both clients (happy path, partial failure + resume,
      processing errors, rate limit) – client tests done
      (`crates/publish/tests/{apple,google}.rs`); executor end to end in
      `crates/app/tests/upload.rs` (fake stores: plan local/remote, Apple
      replace + order, Google edit/commit, partial failure + resume, cancel,
      tokens, blocking rules, MCP policy, Apple processing budget,
      FAILED processing, cancel while polling, reuse, stale runs, watchdog,
      no editable version once)
- [~] User runs a real upload into a test app (Apple first, then Google) –
      first real Apple run 2026-11-10 (→ 5.5c fixes); re-run, asset-library
      run and Google still open (Phase 7 checklist)
- [x] Added 2026-11-12 for CI: store credentials from `VITRINE_ASC_*` /
      `VITRINE_PLAY_*` (only `vitrine upload`, ADR 0016 amendment), CI
      upload = `plan` + `execute --confirm <token> --yes` in one job
      (ADR 0018 amendment); sample screenshots from templates block an
      upload plan (ADR 0023 amendment)
- [~] Docs: ADR(s), NOTES (setup: creating API keys / service account, roles
      needed, locale mapping, how to upload), CLAUDE.md, STATUS.md – done
      except the findings of the real run
