# 0025 – API server and GitHub login

- Status: accepted
- Date: 2026-11-11

## Context

ADR 0023 introduced the gallery backend as `crates/gallery-server` with
GitHub OAuth only as 501 stubs and owner-issued upload tokens. The owner
deployed it at `api.vitrinestudio.app` and registered a GitHub OAuth App
under the org `vitrinestudio-app` (device flow enabled, callback
`https://api.vitrinestudio.app/v1/auth/github/callback`, client id
`Ov23linU2WEHOoBNEFpq`). User decisions 2026-11-11: the server will host
more than the gallery, so it is renamed; browsing and downloading stay
open without login, only publishing needs GitHub; the app's "new
project" dialog will later show the server's templates.

## Decision

### Name and structure

Crate `vitrine-api-server` in `crates/api-server`, binary `vitrine-api`,
Docker image `vitrine-api`, compose service `api`, environment
`VITRINE_API_*` (mapping and upgrade steps: `docs/api/DEPLOY.md`), docs in
`docs/api/`. HTTP routes unchanged. One module per API area under
`src/routes/` (`templates`, `admin`, `auth`), each with a `routes()`
router merged in `routes::router`; shared `AppState`, `Config`, `db`,
`error` (`ApiError` with optional extra body fields), `ratelimit`;
authentication in `src/auth/` (`mod` = bearer, uploader identity, admin;
`session`; `github`). SQLite file `api.sqlite` (the deployed
`gallery.sqlite` was empty).

### Login

- **Device flow** for the app and CLI: the server proxies GitHub's device
  code and token endpoints (`POST /v1/auth/github/device`, `POST
  /v1/auth/github/token`), so the client needs only the API and the
  server keeps the client id in one place. It enforces GitHub's interval
  per device code (in-memory gate; too-early polls get `slow_down` without
  a GitHub call) and has own per-client limits for login starts (10/h)
  and polls (300/h), so it cannot be used to hammer GitHub. It maps GitHub's OAuth errors to its own codes
  (`authorization_pending`, `slow_down` + `interval`, `expired_token`,
  `access_denied`, …).
- **Web flow** for a future website: `state` (random, stored hashed,
  single use, 10 minutes) bound to the browser by an HttpOnly SameSite=Lax
  cookie (login CSRF), plus PKCE S256 (GitHub recommends it). Needs the
  client secret (`VITRINE_API_GITHUB_CLIENT_SECRET`); without it the web
  login answers 503 and the device flow still works. The token reaches the
  website as JSON or in the fragment of `VITRINE_API_WEB_LOGIN_RETURN_URL`.
- **No scope**; identity = `GET /user` → login + numeric id. The GitHub
  access token is dropped right after that call.
- **Own session tokens**: `vss_` + 32 random bytes hex, stored as SHA-256
  hash with login, GitHub id, expiry (90 days default); revoked by `POST
  /v1/auth/logout`; `GET /v1/auth/me` tells who a token is.
- **Static upload tokens** (`token:login[:github_id]`) stay for accounts
  that cannot use OAuth – the org `vitrinestudio-app` publishes the
  official templates this way – and for testers.
- **Ownership** by numeric GitHub id when both sides have one (renamed
  accounts keep their templates, the displayed login follows), else by
  login (case-insensitive).
- **Official**: templates of `VITRINE_API_OFFICIAL_LOGINS` (default
  `vitrinestudio-app`) carry `official: true`.

### Public reads

List, detail, previews and downloads need no login and get their own,
generous per-client limit (`VITRINE_API_READ_RATE_LIMIT_PER_HOUR`, 3000);
writes (uploads, reports, login starts) keep the strict one.

### Clients

`vitrine_publish::gallery` (reqwest + rustls, like the store clients; core
stays network-free): `api_url` (explicit `https`/loopback URL, else
`VITRINE_API_URL` when it is a loopback URL, else production),
`GalleryClient` (list, detail, preview, download with size caps, upload,
device start/poll, me, logout), `GalleryError::Unreachable` for
offline/DNS/timeout/proxy 502–504 so front ends can stay quiet. Session
tokens go to the keychain through `SecretStore::{set,get,delete}_named`
(service `app.vitrinestudio`, account `gallery/<api host>`).

- CLI `vitrine gallery login|logout|list|upload|download` (`--api`;
  `upload --token-env VAR` for static tokens – never a token argument).
- App commands `gallery_list`, image command `gallery_preview`,
  `gallery_download` (same safe import as template files, installs into
  the app-wide store, refuses a package whose id differs from the one
  asked for); new error codes `unreachable` and `remote`. GUI wiring of
  the commands exists; the dialog is a separate step.

## Consequences

- One login for app, CLI and website; the server never stores GitHub
  tokens, only hashes of its own.
- A stolen database does not reveal usable session tokens.
- Tests run against a fake GitHub (wiremock) and an in-process server; no
  network.
- Open: uploader self-service (list/delete own templates, revoke all
  sessions), deleting rejected files, admin CLI, the GUI dialog, a
  website upload page, a session-cleanup job independent of new logins.

## Amendment 2026-11-12: gallery data for the website, bulk upload, admin CLI

- **Schema version 1** (`PRAGMA user_version`, idempotent `db::migrate`):
  columns `devices`, `samples`, `sprites`, `backfilled`; tags stored as the
  facet map. `backfill` (spawned after start) fills derived fields from
  the stored packages and renders missing sprites of approved templates.
- **Sprites**: per canvas `<data>/templates/<id>/sprites/<canvas>[@2x].webp`
  (lossy WebP via the `webp` crate = bundled libwebp, BSD-3-Clause; only
  this crate links it), metadata in the `sprites` column; rendered on
  upload, on approve when missing and by `POST
  /v1/admin/templates/{id}/rerender`; at most
  `VITRINE_API_RENDER_CONCURRENCY` renders at once (tokio semaphore +
  `spawn_blocking`). Public route with `ETag`/304 and a long cache for
  `?v=<etag>` URLs.
- **Filters and taxonomy**: `style|color|use|devices` list filters
  (SQLite `json_each`), `GET /v1/tags`; uploads need complete tags (422
  `invalid_tags`).
- **Clients**: `publish::gallery` gains facet filters, `upload(…,
  confirm_own_screenshots)`, `admin_queue`, `admin_action`
  (approve/reject/rerender), `tags`. CLI `gallery upload <files…>
  [--dry-run] [--confirm-own-screenshots]` (all local checks, summary,
  continue on error) and `gallery admin …` (token only from an
  environment variable). This closes the "admin CLI" open point.

## Amendment 2026-11-12: upload limit errors

- An upload body over `VITRINE_API_MAX_UPLOAD_MB` (default stays 50) is
  answered `413` `{"code": "too_large", "max_upload_mb": N}`; `GET /health`
  includes `max_upload_mb`. The package inside is read with
  `Limits::for_package(max_upload)`; stored packages are re-read with the
  larger of that and the client limit (ADR 0023 amendment).
- The CLI tells the API's `too_large` (names the limit and the template's
  size) apart from a reverse proxy's `413` without JSON (nginx
  `client_max_body_size`, see `docs/api/DEPLOY.md`). Template uploads and
  downloads get a 15-minute request timeout in `publish::gallery`.
