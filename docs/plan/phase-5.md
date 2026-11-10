# Phase 5 – Store upload: task list

Scope: KONZEPT.md §7 and §12, Phase 5. Done when uploading into a test app
works for both stores (App Store Connect first, then Google Play), including
screenshot order.

Status markers: `[ ]` open · `[~]` in progress · `[x]` done

## Principles (from the concept, binding)
- Crate `arrangia-publish`: direct REST calls (`reqwest` + rustls), JWT via
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
Client part done 2026-11-10 (`arrangia_publish::apple`, ADR 0017). Apple
deprecated the set-based screenshot API in 4.5.1 (App Asset Library
replaces it) – open question in ADR 0017.
- [x] JWT ES256 (15 min, max 20; team + individual keys), apps → editable
      `appStoreVersion` → `appStoreVersionLocalizations` →
      `appScreenshotSets` per display type
- [x] Replace set: delete old screenshots, reserve (`appScreenshots` POST with
      file size/name), upload parts per `uploadOperations`, commit with MD5
      checksum, poll `assetDeliveryState`, set order
      (`appScreenshotSets/{id}/relationships/appScreenshots` PATCH)
- [~] Rate limits / retries / resume after abort; log every step – client
      side done (retry/backoff/Retry-After, cancel, observer hook, progress
      events, idempotent deletes); resume + upload log belong to 5.5
- [ ] Optional: App Asset Library backend (`appAssetLibraryImages` +
      placements + ordering requests; needed for `IPHONE_DUO`)

## 5.4 Google Play client
Client part done 2026-11-10 (`arrangia_publish::google`, ADR 0017).
- [x] OAuth2 JWT bearer (service account) → access token
- [x] `edits.insert` → per language & image type `edits.images.deleteall` →
      `edits.images.upload` (order = upload order) → `edits.commit`
      (with `changesNotSentForReview` option where required); abort = delete edit

## 5.5 Execute + UI (done 2026-11-10, ADR 0018; GUI not yet checked visually)
- [x] Executor with progress events, cancel, resume, upload log
- [x] CLI: `arrangia upload plan [--store apple|google] [--target] [--lang]`,
      `arrangia upload execute --confirm <token>`, `arrangia upload log`
- [x] GUI: "Upload…" dialog (plan table: store/target/language/changes/rule
      problems → confirm → progress → result/log) + MCP confirmation dialog
- [x] MCP: `upload_plan`, `upload_execute {confirm_token}`, `upload_status`,
      `upload_log`; GUI-hosted needs the user's click, headless off unless
      `mcp_allow_headless_upload` (recommended default implemented; user
      decision (b) still to confirm)

## 5.6 Tests & real run
- [~] wiremock tests for both clients (happy path, partial failure + resume,
      processing errors, rate limit) – client tests done
      (`crates/publish/tests/{apple,google}.rs`); executor end to end in
      `crates/app/tests/upload.rs` (fake stores: plan local/remote, Apple
      replace + order, Google edit/commit, partial failure + resume, cancel,
      tokens, blocking rules, MCP policy)
- [ ] User runs a real upload into a test app (Apple first, then Google)
- [~] Docs: ADR(s), NOTES (setup: creating API keys / service account, roles
      needed, locale mapping, how to upload), CLAUDE.md, STATUS.md – done
      except the findings of the real run
