# 0018 – Upload plan and executor (incl. MCP confirmation policy)

- Status: accepted (defaults for the open user questions (a)/(b) in
  `docs/plan/STATUS.md` implemented; real run pending, Phase 5.6)
- Date: 2026-11-10

## Context

Phase 5.2/5.5 (KONZEPT §7, `docs/plan/phase-5.md`): never upload without
a plan and an explicit confirmation – also not via MCP. Accounts and the
project's store mapping exist (ADR 0016), the REST clients too (ADR
0017). Two decisions are still open with the user: (a) Apple's App Asset
Library flow vs. the deprecated set-based flow, (b) how MCP uploads are
confirmed. Both had to be built so that changing them later is cheap.

## Decision

Everything lives in `arrangia_app::upload` (it needs rendering,
validation, settings, the keychain and the store clients; `core` stays
network-free, `publish` stays a pair of clients):

```text
plan     dry run + confirm token        log      .cache/uploads.sqlite
remote   clients, AppleUploadFlow       exec     executor thread
policy   who may execute (MCP rules)
```

Commands (GUI/dev bridge, CLI, MCP all go through them): `upload_plan`,
`upload_execute`, `upload_status`, `upload_cancel`, `upload_log`,
`upload_confirm`.

### Plan (`upload_plan {store?, targets?, langs?, remote?, force?}`)

- **Sets**: every target with a slot of a mapped store × its languages
  (filters), language → store locale (project map → defaults, ADR 0016).
  Key `<store>:<locale>:<slot>`. Apple slots of the other platform
  (`APP_DESKTOP` vs iOS) and languages without a store locale become
  `skip` with a reason.
- **Images**: the incremental export renders the slotted targets into
  `<project>/.cache/upload/images` (no gallery previews; a second plan
  renders nothing). Store order = image index. SHA-256 per image, plus MD5
  (Apple's `sourceFileChecksum`). File name sent to the store:
  `<target>-<lang>-<nn>_<pages>.png`.
- **Compare**: the log's last `done` set with the same store/app/locale/
  slot and the same hashes in the same order → `unchanged`; a different
  one → `replace`; none → `new`. `remote: true` additionally reads the
  store (Apple: app → editable version → localizations → sets →
  screenshots with checksum and state; Google: a throw-away edit, `list`
  per language and image type, `sha256`, edit deleted) and refines the
  action (`unchanged` when the store holds exactly these images in this
  order) and the generic steps (`create_localization`, `create_set`,
  `delete_existing`, `upload`, `set_order`). `force` uploads unchanged
  sets too.
- **Checks** (`problems`, `blocking`): validation errors of the target ×
  language (export mode: missing screenshots, store sizes/formats/count),
  the store-mapping warnings `store_locale_duplicate`,
  `store_slot_duplicate`, `store_locale_unknown` (ADR 0016 said the plan
  refuses them), two targets filling one set, render failures, no images,
  file size over the catalog's `max_file_size` / Google's 15 MB, missing
  or wrong account, missing app id/package; remote: no editable version
  (`no_editable_version`), app/package not reachable. One blocking
  problem blocks the whole plan (`blocked: true`); filter with
  `--target`/`--lang` to upload the rest.
- **Flow field**: `StorePlan.flow` (`screenshot_sets`, later
  `asset_library`; Google `edit`) is the only flow-specific output.
- **Confirm token** `upl_<32 hex>` = SHA-256 over the fingerprint, the
  expiry and a random nonce; stored with the fingerprint, expiry (30 min),
  a used flag and the plan JSON in the log (CLI `plan` and `execute` are
  separate processes, so the session cannot hold it). The fingerprint is
  SHA-256 over the canonical JSON of project folder, revision, `store:`
  section, selection and per set key/target/lang/locale/image hashes.
  `upload_execute` re-builds the local plan and compares: any saved
  change (revision) or changed image → `not_confirmed` (`reason:
  changed`); also `unknown`, `expired`, `used` (a token starts one run),
  `blocked`. The stored plan (not a client-supplied one) is executed.

### Executor (`upload_execute {token, wait?, changes_not_sent_for_review?}`)

Runs on its own thread with a current-thread tokio runtime; one run at a
time (`busy`). `wait` blocks (CLI, tests); the GUI and MCP return at once
and follow `upload-progress` events / `upload_status`.

- **Apple** (`AppleUploadFlow`, today `ScreenshotSetFlow`): `open` (app +
  editable version, precondition), then per set `replace`: localization
  (created when missing), set (`ensure_set`), delete all screenshots of the
  set (also leftovers of an aborted run), upload in order
  (reserve/parts/commit MD5/poll), `set_order`, verify the order. A set
  is `done` only after verification. Per-set errors continue with the
  next set; `Auth` errors stop the store.
- **Google**: one edit for all languages and image types: per set
  `deleteall` → upload in order; then `commit`
  (`changesNotSentForReview` only on request; a commit error mentioning it
  gets a hint). Any failure or cancel deletes the edit (a cleanup client
  with its own cancel token) – nothing changes in Play; sets are `done`
  only after the commit.
- **Progress**: `upload-progress {run_id, status, step, set, file,
  images_done/total, bytes_done/total, paused_until}` (steps `prepare`,
  `set_started`, `delete_existing`, `file_started`, `transfer`,
  `file_done`, `set_order`, `verify`, `set_uploaded`, `commit`,
  `set_finished`, `paused`, `resumed`); `upload-finished` with the final
  `RunState`.
- **Cancel** (`upload_cancel`): the clients' cancellation token aborts
  in-flight requests; the current Apple set is `cancelled` (incomplete in
  the store; the next run replaces it), remaining sets `cancelled`.
- **Resume**: no explicit resume command. A re-run is a new plan: sets
  done with the same hashes are `unchanged` and skipped; everything else
  is replaced as a whole set (simple and idempotent; per-file state is
  logged for diagnosis only).
- **Rate limits**: `RateLimited` (retries exhausted or `Retry-After` >
  120 s in the client) pauses the set (`paused`, `paused_until`; wait =
  `Retry-After` or 1 min, at most 15 min, 2 pauses per set) and retries
  the set; then it fails with the store's message.
- **Log** (`<project>/.cache/uploads.sqlite`, rusqlite): `plans`, `runs`
  (status, report JSON), `sets` (store, app, locale, slot, target, lang,
  hashes, status, error), `files` (index, name, SHA-256, bytes, store id,
  status), `events` (redacted HTTP events: method, scheme/host/path,
  status, ms, retries; never headers, bodies, tokens, keys or signed
  URLs). `upload_log {limit, events}`.

### MCP policy (`upload::policy`, the user's open question b)

| caller | requirement |
|---|---|
| user (GUI button, CLI `--confirm` + prompt / `--yes`) | confirm token |
| assistant via the GUI-hosted MCP | token + click "Confirm upload" in the app: `upload-confirm-requested {id, token, client, summary, plan, expires_at}` → GUI dialog → `upload_confirm {id, approved}`; the tool call waits up to 5 min, then `not_confirmed` (`reason: timeout`; declined: `denied`); `upload-confirm-resolved {id, outcome}` |
| assistant via `arrangia mcp --headless` | refused (`forbidden`) unless the app setting `mcp_allow_headless_upload` is on (default off); then the token is enough |

Implementation: the MCP tool `upload_execute {confirm_token}` calls
`check_mcp_allowed` (headless refusal before anything else),
`upload_verify` (token valid?), `authorize_mcp` (asks the GUI and waits)
and then the command. An approval is a one-time **grant** bound to the
token; `upload_execute` refuses assistant actors without a grant
(`reason: no_confirmation`), so no tool path can skip the dialog.
`upload_confirm` refuses assistant actors. Changing the policy = editing
`requirement()` (one table) in `crates/app/src/upload/policy.rs`.
MCP tool descriptions stress that uploads are outward-facing and need
the user's explicit request and confirmation.

### Apple flow abstraction (the user's open question a)

`AppleUploadFlow` (`open`, `inspect`, `replace`) + `AppleFlowKind`. The
App Asset Library flow (`appAssetLibraryImages` + placements + ordering
requests, display types from `appAssetLibraryRefData`) would be a second
implementation and a second kind; plan, executor, CLI, GUI and MCP do
not change (they only show `flow`). New endpoint used by the set flow:
`POST /v1/appStoreVersionLocalizations` (`data.type
appStoreVersionLocalizations`, attribute `locale`, relationship
`appStoreVersion`; 201; checked 2026-11-10 against Apple's JSON docs).
Apple notes that the same locale must exist in the app info
localizations before submission – not created by Arrangia (App Store
Connect shows it).

### Endpoints in tests

`UploadConfig` (on `AppState::upload()`) holds endpoints, client
options and timings. CLI and dev bridge read
`ARRANGIA_ASC_BASE_URL`/`ARRANGIA_PLAY_BASE_URL`/`ARRANGIA_PLAY_TOKEN_URL`
(loopback URLs only, like the connection test); the real GUI always
uses the production endpoints. Tests set the config directly.

## Consequences

- Tests: `crates/app/tests/upload.rs` (stateful fake ASC + Play servers on
  wiremock: local/remote plan, Apple replace + order, Google edit/commit,
  partial failure + resume, Google abort, cancel, token
  unknown/changed/used/expired, blocking rules, missing editable version,
  MCP approve/deny/timeout, headless off by default),
  `crates/cli/tests/upload.rs`, `crates/cli/tests/mcp.rs` (tools, headless
  refusal).
- To confirm in the real run (5.6): editable states, processing times,
  whether replacing whole sets is acceptable, the Play review flag, the
  app info localization note.
- The plan renders full-size images into `.cache/upload/images` (disk
  space ≈ one export of the store targets).

## Amendment 2026-11-10: readiness checklist

`upload::readiness` computes, without rendering, keychain or network, what
is missing before an upload, in the order to fix it, each step with
`{code, store?, message, fix, blocking}` and `fix` ∈ `app_settings_stores`
| `project_settings_stores` | `targets`:

1. App settings: `no_account` (no account of that store on this machine),
   `account_missing_for_project` (the project names an account id that is
   not set up here);
2. Project settings: `project_not_mapped` (no store mapped, or no account /
   app identity chosen), `store_locale_unknown`, `store_locale_conflict`
   (two languages → one store locale), `no_store_locale` (warning: skipped);
3. Targets: `no_store_slots`, `store_slot_conflict`.

The command `upload_readiness {store?}` returns it (the upload dialog shows
it first, with a button per step that opens the right dialog and re-checks
when it closes); `upload_plan` carries it as `readiness` (serde default,
older stored plans still load) and the MCP `upload_plan` text lists the
steps as numbered hints. The plan's own store problem for a missing
account choice was renamed `no_account` → `account_not_selected` so
`no_account` means only "nothing in the app settings".
