# Vitrine Studio API (`api.vitrinestudio.app`)

Server: `crates/api-server` (crate `vitrine-api-server`, binary
`vitrine-api`), ADR 0023 (templates) and ADR 0025 (API server, GitHub
login). Today it hosts the template gallery; further areas are added as
modules under `crates/api-server/src/routes/`. Tests:
`crates/api-server/tests/{api,auth}.rs`. Clients:
`vitrine_publish::gallery` (used by `vitrine gallery …` and the app
commands `gallery_list|gallery_preview|gallery_download`).

Conventions: JSON (`application/json`), errors as
`{"error": "<message>", "code": "<code>"}` (plus fields such as
`interval`) with the HTTP status. **Browsing, previews and downloads need
no account** (rate limited per client); only uploading needs a login.
Every template is CC0-1.0; fonts inside keep the OFL.

## Public

| Method | Path | Description |
|---|---|---|
| GET | `/health` | `{"status": "ok", "version": "…"}` |
| GET | `/v1/templates?q=&tag=&limit=&offset=` | approved templates, newest first; `q` searches name, description, tags; `limit` 1–100 (default 50) |
| GET | `/v1/templates/{id}` | one approved template |
| GET | `/v1/templates/{id}/download` | the `.vitrine-template` (`application/zip`, `Content-Disposition: attachment`); counts a download |
| GET | `/v1/templates/{id}/previews/{canvas}` | preview PNG of a canvas (rendered by the server) |
| POST | `/v1/templates/{id}/report` | abuse report `{"reason": "copyright|trademark|offensive|malware|other", "message": "…"}` → 202; rate limited |

The four GET routes under `/v1/templates` share a read limit per client
(`VITRINE_API_READ_RATE_LIMIT_PER_HOUR`, default 3000); reports and uploads
share `VITRINE_API_RATE_LIMIT_PER_HOUR` (default 20); login starts
(`VITRINE_API_LOGIN_RATE_LIMIT_PER_HOUR`, 10) and device-flow polls
(`VITRINE_API_POLL_RATE_LIMIT_PER_HOUR`, 300) have their own. Over a
limit: 429 `rate_limited`.

Template object:

```json
{
  "id": "bold-gradient", "name": "Bold gradient", "description": "…",
  "author": "…", "tags": ["gradient"], "version": "1.0.0",
  "license": "CC0-1.0", "uploader": "octocat", "official": false,
  "status": "approved",
  "canvases": [{"id": "phone", "aspect": "1320:2868", "pages": 5,
                "targets": ["ios-iphone-69", "play-phone"],
                "preview": "previews/phone.png",
                "preview_url": "https://api.vitrinestudio.app/v1/templates/bold-gradient/previews/phone"}],
  "fonts": ["Lora"], "size": 412345, "downloads": 12,
  "created_at": 1762850000, "updated_at": 1762850000,
  "download_url": "https://api.vitrinestudio.app/v1/templates/bold-gradient/download"
}
```

`official` is `true` when the uploader is one of
`VITRINE_API_OFFICIAL_LOGINS` (default `vitrinestudio-app`, the org that
publishes the official templates; compared case-insensitively).

## Upload (login required)

| Method | Path | Description |
|---|---|---|
| POST | `/v1/templates` | body: the `.vitrine-template` (`application/zip`, ≤ `VITRINE_API_MAX_UPLOAD_MB`); `Authorization: Bearer <token>` → 201 `{"id", "version", "status": "pending"}` |

The bearer token is a **session token** (`vss_…`, from the GitHub login
below) or a **static upload token** from `VITRINE_API_UPLOAD_TOKENS`
(owner-issued; for the org `vitrinestudio-app`, which cannot sign in with
OAuth, and testers).

Server-side checks (same code as the app, `vitrine_core::template`): size
limits, safe paths, no symlinks, only the allowed file types (checked by
content), SVG without scripts/external references, fonts must declare the
OFL (name ID 13/14) with an OFL licence file, `license: CC0-1.0`, the
project subset loads and validates, no store settings. Then the server
**replaces all screenshots by generated placeholders** and **renders the
previews itself** (uploaded previews are discarded). Errors: 401 no/invalid
token, 413 too large, 422 `invalid_template` (message says why), 409
`id_taken` (id of another uploader) or `version_exists`, 429
`rate_limited`.

**Ownership**: the server stores the uploader's GitHub login and numeric
GitHub user id. A new version (same id, different `version`) is accepted
from the owner: when both sides have a GitHub id, the ids decide (a renamed
GitHub account keeps its templates, the shown uploader name follows the
new login; someone who later takes the old login cannot update them);
otherwise (static token without id) the logins decide, case-insensitively.
A new version replaces the files and goes back to `pending`.

Upload terms the client shows before uploading (text to be finalised by
the owner): the uploader owns the rights to everything in the template,
grants CC0 1.0 for it, and confirms that it contains no app screenshots of
others, no vendor device images and only OFL fonts.

## Login with GitHub (ADR 0025)

OAuth App of the org `vitrinestudio-app`, client id
`Ov23linU2WEHOoBNEFpq` (public; `VITRINE_API_GITHUB_CLIENT_ID`), device
flow enabled, callback `https://api.vitrinestudio.app/v1/auth/github/callback`.
No scope is requested (login and id of `GET /user` are public). The server
reads the user once, **discards the GitHub access token** and issues its
own session token: `vss_` + 64 hex chars (32 random bytes), stored only as
a SHA-256 hash with login, GitHub id and expiry
(`VITRINE_API_SESSION_DAYS`, default 90); expired sessions are refused and
cleaned up.

| Method | Path | Description |
|---|---|---|
| POST | `/v1/auth/github/device` | starts the device flow (app/CLI) → `{"device_code", "user_code", "verification_uri", "expires_in", "interval"}` (GitHub's answer); rate limited |
| POST | `/v1/auth/github/token` | body `{"device_code"}`; one poll of GitHub → 200 `{"token", "login", "github_id", "expires_at", "official"}`, or 400 `authorization_pending` (poll again after `interval` s), 400 `slow_down` (+ `interval`: the new minimum), 400 `expired_token` / `incorrect_device_code` (start again), 403 `access_denied` (cancelled on GitHub), 503 `device_flow_disabled`, 502 `github_error` |
| GET | `/v1/auth/github/login` | web flow (website): 302 to GitHub's authorize page with `state` and a PKCE S256 challenge; sets the cookie `vitrine_oauth_state` (HttpOnly, SameSite=Lax, Secure on https, path `/v1/auth/github`, 10 min); 503 `web_login_disabled` without `VITRINE_API_GITHUB_CLIENT_SECRET` |
| GET | `/v1/auth/github/callback` | `?code&state`: the state must match the cookie and a stored, unused, unexpired state (else 400 `invalid_state`); exchanges the code (client secret + PKCE verifier), issues the session. Answers the same JSON as the device flow, or – with `VITRINE_API_WEB_LOGIN_RETURN_URL` – redirects there with `#token=…&login=…&expires_at=…` in the fragment |
| GET | `/v1/auth/me` | bearer → `{"login", "github_id", "kind": "session|static", "expires_at", "official"}` |
| POST | `/v1/auth/logout` | bearer session token → `{"revoked": true}` (deleted); static tokens → 400 `static_token`; unknown → 401 |

The client polls `/v1/auth/github/token` at most every `interval` seconds
(RFC 8628). The server polls GitHub once per call and enforces GitHub's
interval per device code (in memory, the code kept as a hash): a poll
sooner than the last interval (from the device start, raised by GitHub's
`slow_down`; 5 s for codes it did not start) is answered 400 `slow_down`
with `interval` locally, without contacting GitHub.

GitHub endpoints used (GitHub docs "Authorizing OAuth apps",
<https://docs.github.com/en/apps/oauth-apps/building-oauth-apps/authorizing-oauth-apps>,
checked 2026-11-11):

- `POST https://github.com/login/device/code` (`client_id`; `Accept:
  application/json`) → `device_code` (40 chars), `user_code` (8 chars with
  a hyphen), `verification_uri` (`https://github.com/login/device`),
  `expires_in` (default 900), `interval`.
- `POST https://github.com/login/oauth/access_token` with `client_id`,
  `device_code`, `grant_type=urn:ietf:params:oauth:grant-type:device_code`
  → `access_token`, `token_type`, `scope`, or `error`:
  `authorization_pending`, `slow_down` (+ `interval`, 5 s more),
  `expired_token`, `unsupported_grant_type`, `incorrect_client_credentials`,
  `incorrect_device_code`, `access_denied`, `device_flow_disabled`. No
  client secret in the device flow.
- `GET https://github.com/login/oauth/authorize` with `client_id`,
  `redirect_uri`, `state`, `code_challenge`, `code_challenge_method=S256`
  (`plain` is not supported), `allow_signup`.
- `POST https://github.com/login/oauth/access_token` with `client_id`,
  `client_secret`, `code` (valid 10 minutes), `redirect_uri`,
  `code_verifier`.
- `GET https://api.github.com/user` with `Authorization: Bearer …`,
  `Accept: application/vnd.github+json`, `X-GitHub-Api-Version: 2022-11-28`
  and a `User-Agent` → `login`, `id`.

## Moderation (admin)

`Authorization: Bearer $VITRINE_API_ADMIN_TOKEN` (unset = moderation off, 403).

| Method | Path | Description |
|---|---|---|
| GET | `/v1/admin/queue` | `{"pending": [templates], "reports": [open reports]}` |
| POST | `/v1/admin/templates/{id}/approve` | publish; optional body `{"note": "…"}`; resolves its reports |
| POST | `/v1/admin/templates/{id}/reject` | hide (also for taking down a reported template) |

Not yet: deleting files of rejected templates (manual for now), uploader
self-service (list/delete own templates), notification of the uploader,
an admin CLI.

## Clients

- CLI: `vitrine gallery login|logout|list|upload|download` (`--api URL`,
  default `https://api.vitrinestudio.app`; `VITRINE_API_URL` overrides it
  with a loopback URL only). The session token lives in the system
  keychain: service `app.vitrinestudio`, account `gallery/<api host>`.
- App commands (for the GUI's "new project" dialog, GUI not built yet):
  `gallery_list {query, tag, offset, limit}` (adds `installed_version`),
  image command `gallery_preview {id, canvas?}`, `gallery_download {id,
  replace?}` (checked with the same safe-import code as template files,
  installed into the app-wide template store). An unreachable server is
  the error code `unreachable` ("Gallery not reachable"); server errors
  are `remote` (`details.code`), unknown ids `not_found`.
- Network code lives in `vitrine-publish` (reqwest + rustls), never in
  `core`.
