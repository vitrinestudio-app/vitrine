# Template gallery API (`api.vitrinestudio.app`) – sketch

Server: `crates/gallery-server` (binary `vitrine-gallery`), ADR 0023.
Status 2026-11-11: skeleton – everything below is implemented and tested
(`crates/gallery-server/tests/api.rs`) except the routes marked **TODO**.

Conventions: JSON (`application/json`), errors as
`{"error": "<message>", "code": "<code>"}` with the HTTP status. Downloads
and browsing need no account. Every template is CC0-1.0; fonts inside keep
the OFL.

## Public

| Method | Path | Description |
|---|---|---|
| GET | `/health` | `{"status": "ok", "version": "…"}` |
| GET | `/v1/templates?q=&tag=&limit=&offset=` | approved templates, newest first; `q` searches name, description, tags; `limit` 1–100 (default 50) |
| GET | `/v1/templates/{id}` | one approved template |
| GET | `/v1/templates/{id}/download` | the `.vitrine-template` (`application/zip`, `Content-Disposition: attachment`); counts a download |
| GET | `/v1/templates/{id}/previews/{canvas}` | preview PNG of a canvas (rendered by the server) |
| POST | `/v1/templates/{id}/report` | abuse report `{"reason": "copyright|trademark|offensive|malware|other", "message": "…"}` → 202; rate limited |

Template object:

```json
{
  "id": "bold-gradient", "name": "Bold gradient", "description": "…",
  "author": "…", "tags": ["gradient"], "version": "1.0.0",
  "license": "CC0-1.0", "uploader": "octocat", "status": "approved",
  "canvases": [{"id": "phone", "aspect": "1320:2868", "pages": 5,
                "targets": ["ios-iphone-69", "play-phone"],
                "preview": "previews/phone.png",
                "preview_url": "https://api.vitrinestudio.app/v1/templates/bold-gradient/previews/phone"}],
  "fonts": ["Lora"], "size": 412345, "downloads": 12,
  "created_at": 1762850000, "updated_at": 1762850000,
  "download_url": "https://api.vitrinestudio.app/v1/templates/bold-gradient/download"
}
```

## Upload (uploader login)

| Method | Path | Description |
|---|---|---|
| POST | `/v1/templates` | body: the `.vitrine-template` (`application/zip`, ≤ `GALLERY_MAX_UPLOAD_MB`); `Authorization: Bearer <token>` → 201 `{"id", "version", "status": "pending"}` |

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

A new version (same id, same uploader, higher/different `version`)
replaces the files and goes back to `pending`.

Upload terms the client shows before uploading (text to be finalised by
the owner): the uploader owns the rights to everything in the template,
grants CC0 1.0 for it, and confirms that it contains no app screenshots of
others, no vendor device images and only OFL fonts.

### Login – GitHub OAuth (**TODO**, feature `github-oauth`, routes answer 501)

| Method | Path | Description |
|---|---|---|
| POST | `/v1/auth/github/device` | start the device flow (app): `{"user_code", "verification_uri", "device_code", "interval"}` |
| POST | `/v1/auth/github/token` | poll with `device_code` → `{"token", "login"}` once the user confirmed |
| GET | `/v1/auth/github/login` | web flow (website): redirect to GitHub (state cookie) |
| GET | `/v1/auth/github/callback` | exchange the code, issue the session token |

The server stores only the GitHub login and numeric id plus a hash of its
own session token (no GitHub access token kept; scope: none / `read:user`).
Until this exists the owner issues interim tokens
(`GALLERY_UPLOAD_TOKENS=token:login,…`).

## Moderation (admin)

`Authorization: Bearer $GALLERY_ADMIN_TOKEN` (unset = moderation off, 403).

| Method | Path | Description |
|---|---|---|
| GET | `/v1/admin/queue` | `{"pending": [templates], "reports": [open reports]}` |
| POST | `/v1/admin/templates/{id}/approve` | publish; optional body `{"note": "…"}`; resolves its reports |
| POST | `/v1/admin/templates/{id}/reject` | hide (also for taking down a reported template) |

Not yet: deleting files of rejected templates (manual for now), uploader
self-service (list/delete own templates), notification of the uploader.

## In the app (planned)

"Browse gallery" in the start screen / insert dialog: fetch
`/v1/templates`, show previews, download → `template_import` (the app
checks the package again). The network client belongs in a
`publish`-like crate, never in `core`.
