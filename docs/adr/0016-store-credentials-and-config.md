# 0016 – Store credentials and store config

- Status: accepted
- Date: 2026-11-10

## Context

Phase 5 uploads screenshots to App Store Connect and Google Play
(KONZEPT §7, `docs/plan/phase-5.md`). Task 5.1 needs: where the API keys
live, how projects say which app and account they upload to, and how
project languages map to store locales. Binding principles: secrets only
in the system keychain, configs hold references, the user imports keys
themselves, Claude (MCP) never handles key material, store-specific values
are data with source and date.

## Decision

### Accounts are app-wide (`settings.yaml`)

`Settings.store_accounts: Vec<StoreAccount>` (`vitrine_app::stores`):
`{id, kind: apple|google, label, apple: {issuer_id, key_id}, google:
{client_email, project_id?}}` – public facts only. Ids are generated from
the label (`apple-my-team`, `-2` on collision). `settings_set` refuses to
touch `store_accounts`; only `store_account_import` / `store_account_remove`
change them, so a settings entry always has its keychain entry (import
deletes the fresh keychain entry again if saving the settings fails).

Accounts are not stored in projects: a project folder is shared through
Git, keys and the people who may use them are per machine. A project only
references an account id; a missing reference is reported
(`project_store_get.missing_accounts`), not an error.

### Secrets in the system keychain (`vitrine_publish::credentials`)

- `keyring` 4.2 (`v1` feature: macOS Keychain, Windows Credential Manager,
  Secret Service on Linux) plus `keyring-core` 1.0 for the store API and
  its in-memory mock. Entry: service `app.vitrinestudio`
  (`VITRINE_KEYRING_SERVICE` overrides), user `<account id>/<kind>`,
  password = the whole file content (Apple `.p8` PEM, Google service
  account JSON).
- `VITRINE_KEYRING=mock` selects a process-wide in-memory store; the MCP
  integration tests set it, and the dev bridge uses a mock unless
  `--system-keychain` is given. App unit tests always get
  `SecretStore::mock()` (`AppState::with_secret_store`), so CI never
  touches a real keychain.
- Validation on import, without logging or echoing secrets: Apple – a PEM
  `PRIVATE KEY` (PKCS#8) or `EC PRIVATE KEY` block whose DER contains the
  OIDs `id-ecPublicKey` and `prime256v1` (ES256 needs P-256); key id from
  `AuthKey_<KEYID>.p8` unless given, issuer id must be a UUID. Google –
  JSON with `type: service_account`, a `client_email` and a PEM
  `private_key`; JSON syntax errors are reported by line/column only
  (serde messages can quote values). Keychain errors that carry stored
  bytes are mapped to generic messages. `Secret` has a redacted `Debug`.
- Remove deletes the keychain entry (a missing entry is fine), then the
  settings entry.

### Connection test seam

`store_account_test` reads the secret, validates it again and calls
`AppState::connection_tester()` (`trait ConnectionTester` in
`vitrine_app::stores`). Without a tester (CLI, headless MCP) the command
answers `not_implemented` with `details: {secret: "ok"}`.

Wired 2026-11-10 (after the clients, ADR 0017, were merged):
`vitrine_app::store_test::StoreClientTester`, installed by the GUI
(`main.rs`) and the dev bridge. Read-only on purpose:

- Apple: signs an ES256 token and calls `GET /v1/apps?limit=1`
  (`apple::Client::apps_overview`) → `apps_visible` (`meta.paging.total`)
  and `first_app`; summary e.g. "key accepted, 3 apps visible (first: …)",
  0 apps → hint to check the key's role / app access.
- Google: only fetches an OAuth access token (`google::Client::authenticate`,
  no package name needed) → `client_email`. No edit is opened, so Play
  Console permissions are only checked at upload time.
- Failures are `io` errors with a user-facing message and
  `details.reason`: `key_rejected` (401 / OAuth 400–403), `no_permission`
  (Apple 403), `offline` (network error), `store_error` (5xx, unexpected
  answers), `rate_limited`, `timeout`, `invalid_key`, `other`.
- Short timeout (20 s) and one retry. The test runs on its own thread with
  a current-thread tokio runtime (callers may already be inside a runtime).
- Dev bridge only: `VITRINE_ASC_BASE_URL`, `VITRINE_PLAY_BASE_URL`,
  `VITRINE_PLAY_TOKEN_URL` redirect it to a mock server; only loopback
  `http://127.0.0.1:` / `localhost:` / `[::1]:` URLs are accepted. Tests:
  `crates/app/tests/store_connection.rs` (wiremock, in-memory keychain).

### Project mapping (`store:` in `project.yaml`, `core::model::store`)

```yaml
store:
  apple: {account: apple-team, app_id: "1234567890", bundle_id: com.example.app, platform: macos}
  google: {account: google-play, package_name: com.example.app}
  locales:
    apple: {en: en-GB}
    google: {de-AT: de-DE}
```

All fields optional; `platform` defaults to `ios` (not written);
`app_id` accepts a hand-written number. `store.<kind>` present means "this
project uploads there". One op `set_store` (whole section, inverse = old
section); it checks formats (numeric app id, reverse-DNS bundle id /
package name, account id syntax) and rejects explicit locale codes the
store does not know. Commands: `project_store_get` (mapping, effective
locales, code lists, account choices, missing accounts) and
`project_store_patch` (store object replaced or `null` removed;
`locales.<kind>` map replaced, empty values = back to default).

### Locale map defaults (`data/store-locales.yaml`)

Code lists from the official pages, read 2026-11-10: Apple's
"Managing metadata in your app by using locale shortcodes" table
(<https://developer.apple.com/documentation/appstoreconnectapi/managing-metadata-in-your-app-by-using-locale-shortcodes>,
50 codes, e.g. `de-DE`, `ja`, `zh-Hans`, `he`, `no`) and Google Play's
"View list of available languages"
(<https://support.google.com/googleplay/android-developer/answer/9844778?hl=en>,
86 codes, e.g. `de-DE`, `ja-JP`, `zh-CN`, `iw-IL`, `es-419`). Resolution:
explicit project map → the language itself when it is a store code
(case-insensitive) → `defaults` for the full tag → `defaults` for the
primary subtag (`de-AT` → `de` → `de-DE`). The catalog check (unit test)
requires every default to be a listed code.

### Validation (`core::validate`, project level)

Only for stores the project is mapped to, except the explicit-map checks:
`store_locale_unknown` (error, hand-edited unknown code),
`store_locale_unused` (info, map key is not a project language),
`store_locale_missing` (warn, an uploaded language has no locale),
`store_locale_duplicate` (warn, e.g. `de` and `de-AT` both → `de-DE`),
`store_no_slot` (warn, mapped store without any target slot),
`store_slot_duplicate` (warn, two targets fill the same display type /
image type for a shared language). Warnings, not errors, so exports keep
working; the upload plan (5.2) refuses them.

### MCP

Read-only `store_config_get` (accounts without secrets, the project's
mapping and effective locales, no code lists). There is no tool to import
or remove accounts, and `apply_ops` refuses `set_store`: the user sets
the mapping in the GUI.

## Consequences

- `vitrine-app` depends on `vitrine-publish` (credentials only so far;
  the clients will join it). `core` stays network- and keychain-free.
- On macOS the keychain item belongs to the binary that created it; a
  rebuilt (differently signed) app may ask the user once to allow access.
- Re-check `store-locales.yaml` when Apple or Google add languages.

## Amendment 2026-11-10: store setup UX

- **Help panels.** App settings → Stores and the import dialog show
  collapsible "How do I get this?" steps for the App Store Connect team key
  and the Google service account (texts in the GUI i18n files, summary in
  NOTES). Links open in the system browser through the command
  `open_url {url}`, which only accepts an exact allowlist of https URLs
  (`commands::os::HELP_URLS`) and goes through the `Dialogs::open_url`
  seam (Tauri: platform opener; dev bridge/tests: `FixedDialogs` records,
  `NoDialogs` answers `not_implemented`).
- **Apple app picker.** `store_apps_list {account, refresh?}` lists every
  app the key can see (`apple::Client::list_apps`: `GET /v1/apps`,
  `fields[apps]=name,bundleId,sku,primaryLocale`, `limit=200`, follows
  `links.next` on the API host only), sorted by name, cached per account
  for the app session (`refresh` asks again; removing the account drops
  it). Errors are mapped like the connection test (`io` +
  `details.reason`). The project settings fill Apple ID + bundle id from
  the choice; manual entry stays. The apps resource has no platform
  attribute, so the platform is not suggested.
- **Google package check.** The Android Publisher API has no "list my
  apps" call (checked against the v3 REST reference 2026-11-10), and its
  edit-free reads (reviews, monetization, orders, …) need other Play
  permissions than "Manage store presence". So the package name stays a
  typed field, and `store_package_check {account, package_name}` runs
  only on the user's "Check" click: `edits.insert` → `edits.details.get`
  (+ the default listing's title) → `edits.delete` – never committed,
  nothing changes in Play (`google::Client::check_package`). 404 →
  `details.reason: not_found`, 403 → `no_permission`.
- Endpoints for both come from `UploadConfig` (production; loopback
  overrides via `VITRINE_ASC_BASE_URL`/`VITRINE_PLAY_*` in CLI/dev
  bridge, set directly in tests). Tests: `crates/publish/tests/{apple,
  google}.rs`, `crates/app/tests/store_setup.rs`.
