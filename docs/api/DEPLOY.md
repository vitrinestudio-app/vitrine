# Deploying the API server (`api.vitrinestudio.app`)

The owner deploys; agents never do. Backend: `crates/api-server` (crate
`vitrine-api-server`, binary `vitrine-api`, ADR 0023 + 0025, API:
`API.md`).

## Steps (new installation)

1. **DNS**: `api.vitrinestudio.app` → the Docker server (A/AAAA record).
2. **Build** on the server (or in CI and push to a registry) from a
   checkout of the repository:
   `docker build -f crates/api-server/Dockerfile -t vitrine-api .`
3. **Secrets** in a `.env` next to the compose file (or as Portainer stack
   environment variables; never in Git):
   - `VITRINE_API_ADMIN_TOKEN=<long random>` (e.g. `openssl rand -hex 32`);
   - `VITRINE_API_UPLOAD_TOKENS=<random>:vitrinestudio-app` – the static
     token for uploading the official templates (the org cannot sign in
     with OAuth); more entries comma-separated as `token:login` or
     `token:login:github_id` for testers;
   - `VITRINE_API_GITHUB_CLIENT_SECRET=<secret of the OAuth App>` – only
     for the web login (website); the app/CLI device flow needs no secret.
4. **Run**: `docker compose -f crates/api-server/docker-compose.example.yml up -d`
   (service `api`, binds `127.0.0.1:8080`, data in the volume
   `gallery-data` = `/data`: `api.sqlite` + `templates/<id>/`).
5. **Reverse proxy** (TLS, HTTP/2, limits): forward `api.vitrinestudio.app`
   to `127.0.0.1:8080`, set `X-Forwarded-For` (trusted with
   `VITRINE_API_TRUST_PROXY=1`), request body limit ≥
   `VITRINE_API_MAX_UPLOAD_MB`, pass cookies (default) for the web login.
   nginx:
   ```
   client_max_body_size 55m;
   proxy_set_header Host $host;
   proxy_set_header X-Forwarded-For $remote_addr;
   proxy_pass http://127.0.0.1:8080;
   ```
   (Caddy: `request_body { max_size 55MB }` + `reverse_proxy 127.0.0.1:8080`.)
   Keep `client_max_body_size` (nginx default: only 1 MB!) a few MB above
   `VITRINE_API_MAX_UPLOAD_MB` and raise both together. Over the API's
   limit, the API answers `413` with `{"code": "too_large",
   "max_upload_mb": N}` and the CLI names that limit; a `413` from nginx
   itself has no JSON body, so the CLI reports "a proxy in front of the API
   limits the request size". Templates with sample screenshots are packed
   as JPEG (quality 60) and mostly stay well below 50 MB; the app/CLI read
   packages up to 250 MB, so the server limit can be raised up to that
   (nginx: also `proxy_read_timeout`/`client_body_timeout` for slow
   uploads; the CLI waits 15 min per upload). `GET /health` shows the
   active limit (`max_upload_mb`).
6. **Check**: `curl https://api.vitrinestudio.app/health`; device login:
   `vitrine gallery login` (shows a code for github.com/login/device).
7. **Backups**: the volume (`api.sqlite` – use `sqlite3 .backup` or stop
   the container – and `templates/`).
8. **Updates**: pull, rebuild, `docker compose up -d`; the schema is created
   with `CREATE TABLE IF NOT EXISTS` (migrations come with the first change
   to existing tables).

## Environment variables

All optional; empty = default. List variables are comma-separated; empty
entries (trailing comma, spaces) are ignored; a bad entry is reported by
its position among the non-empty entries ("entry 2"), never by its value.

| Variable | Default | Meaning |
|---|---|---|
| `VITRINE_API_BIND` | `0.0.0.0:8080` | listen address |
| `VITRINE_API_DATA_DIR` | `/data` | `api.sqlite` + `templates/` |
| `VITRINE_API_PUBLIC_URL` | `https://api.vitrinestudio.app` | base of links in responses and of the OAuth callback (`…/v1/auth/github/callback`) |
| `VITRINE_API_TRUST_PROXY` | `0` | client address from `X-Forwarded-For` |
| `VITRINE_API_MAX_UPLOAD_MB` | `50` | upload body limit (MB); keep the proxy's body limit above it; at most 250 (the client limit) |
| `VITRINE_API_RATE_LIMIT_PER_HOUR` | `20` | uploads and reports per client |
| `VITRINE_API_READ_RATE_LIMIT_PER_HOUR` | `3000` | list/detail/preview/download per client |
| `VITRINE_API_LOGIN_RATE_LIMIT_PER_HOUR` | `10` | login starts (`POST /v1/auth/github/device`, `GET /v1/auth/github/login`) per client |
| `VITRINE_API_POLL_RATE_LIMIT_PER_HOUR` | `300` | device-flow polls (`POST /v1/auth/github/token`) per client (one flow every 5 s for 15 min = 180) |
| `VITRINE_API_AUTO_APPROVE` | `0` | uploads public at once (testing only) |
| `VITRINE_API_ADMIN_TOKEN` | – | moderation bearer token (unset = moderation off) |
| `VITRINE_API_UPLOAD_TOKENS` | – | static upload tokens `token:login[:github_id],…` |
| `VITRINE_API_OFFICIAL_LOGINS` | `vitrinestudio-app` | uploaders whose templates are `official` |
| `VITRINE_API_GITHUB_CLIENT_ID` | `Ov23linU2WEHOoBNEFpq` | OAuth App client id (public) |
| `VITRINE_API_GITHUB_CLIENT_SECRET` | – | OAuth App secret, web flow only (unset = web login 503) |
| `VITRINE_API_SESSION_DAYS` | `90` | session token lifetime |
| `VITRINE_API_WEB_LOGIN_RETURN_URL` | – | web flow: redirect here with the token in the URL fragment (unset = JSON answer) |
| `VITRINE_API_CORS_ORIGINS` | – | browser origins allowed to read the public GET routes, e.g. `https://www.vitrinestudio.app,https://vitrinestudio.app` (the website gallery's live refresh) |
| `VITRINE_API_RENDER_CONCURRENCY` | `2` | previews/sprites rendered at the same time (uploads, approvals, re-renders, start-up backfill) |

`VITRINE_API_URL` is *not* a server variable: it is the client-side
override of the API base URL (CLI/app, loopback URLs only).

## Upgrade 2026-11-12 (website gallery): CORS, list limit 200, per-canvas devices

- Add `VITRINE_API_CORS_ORIGINS: https://www.vitrinestudio.app,https://vitrinestudio.app`
  to the stack (without it the website still works from its build-time
  snapshot, but cannot refresh live or filter newer templates).
- Schema version 2: on start the server marks every template for the
  backfill again and stores `devices` per canvas from the stored package
  (no re-render; a few seconds for 50 templates). Nothing to do.
- `GET /v1/templates` accepts `limit` up to 200.
- If nginx adds its own CORS headers for the API, remove them (the
  server sends them; duplicates make browsers refuse the answer).

## Upgrade 2026-11-12: tags, sample screenshots, gallery sprites

What changes (ADR 0023 amendment, `docs/api/API.md`): fixed tag facets
(`style`, `color`, `use`, derived `devices`) with list filters and `GET
/v1/tags`; sample screenshots kept when the uploader confirms them
(`?confirm_own_screenshots=true`); WebP gallery sprites per canvas
(`/v1/templates/{id}/sprites/…`); admin `rerender`; uploads need complete
tags. New optional variable `VITRINE_API_RENDER_CONCURRENCY` (default 2:
renders at the same time – uploads, approvals, re-renders; lower it on a
small server). The image now also compiles libwebp (C, BSD-3-Clause; the
`rust` build image has the compiler).

Database: **migrated automatically and idempotently** on start (`PRAGMA
user_version` 0 → 1): the columns `devices`, `samples`, `sprites`,
`backfilled` are added when missing, and the old free tag lists are
sorted into the facets (unknown words such as `garden` are dropped). Then,
in the background, the server reads every stored package once to fill
devices, sample counts and tags, and renders the sprites of all approved
templates (log lines `rendered sprites of …`). Templates already in the
gallery stay online during this; their `sprite` is `null` until rendered.

Steps:

1. Back up the volume first (one file + the template folders):
   `docker exec <api container> sqlite3 /data/api.sqlite ".backup /data/api-before-tags.sqlite"`
   – or stop the stack and copy the volume.
2. Pull, build: `docker build -f crates/api-server/Dockerfile -t vitrine-api .`
3. Optional: add `VITRINE_API_RENDER_CONCURRENCY: 2` to the stack.
4. "Update the stack" (redeploy). Check `docker logs <api container>`:
   `vitrine-api listening …`, then `rendered sprites of <id>` per approved
   template.
5. Check: `curl https://api.vitrinestudio.app/v1/tags` and
   `curl 'https://api.vitrinestudio.app/v1/templates?devices=phone'`;
   each canvas has a `sprite` with URLs that answer `image/webp`.
6. Templates uploaded before have the tags their packages had (often
   none); re-upload them with tags (raise the version), or leave them.
   A sprite can be re-rendered any time:
   `VITRINE_API_ADMIN_TOKEN=… vitrine gallery admin rerender <id>`.
7. nginx: nothing to change (`/v1/…` is proxied already). If nginx sets
   its own cache headers for the API, let `Cache-Control`/`ETag` of the
   sprite responses through.

Rollback: the old image ignores the new columns, but reads the converted
`tags` (a JSON object instead of a list) as given – it keeps working;
restore the backup for an exact state.

### Uploading the ~50 official templates

On a workstation with the release CLI, from the project(s) holding the
templates (each saved with tags and, where wanted, samples):

```sh
# once per template (or loop over the projects):
vitrine --project <project> template save --name "<name>" --canvas <canvas> \
  --style <style> --color <color> --use <use> --sample-screenshots \
  --version 1.0.0 -o dist/<id>.vitrine-template
# all checks, nothing sent (summary table; fix what it reports):
vitrine gallery upload dist/*.vitrine-template --dry-run --confirm-own-screenshots
# upload with the official static token (VITRINE_API_UPLOAD_TOKENS
# `<token>:vitrinestudio-app`), continuing after errors:
export VITRINE_OFFICIAL_TOKEN=<token>
vitrine gallery upload dist/*.vitrine-template --confirm-own-screenshots \
  --token-env VITRINE_OFFICIAL_TOKEN
# review and publish:
export VITRINE_API_ADMIN_TOKEN=<admin token>
vitrine gallery admin queue
vitrine gallery admin approve <id> <id> …
```

The upload rate limit (`VITRINE_API_RATE_LIMIT_PER_HOUR`, default 20 per
client and hour) also applies to the owner: raise it for the bulk upload
(e.g. 100) and set it back afterwards, or upload in batches – failed
files are listed in the summary and can be uploaded again.

## Upgrade from the gallery-server deployment

The crate `gallery-server` (image `vitrine-gallery`, service `gallery`,
`GALLERY_*` variables) was renamed on 2026-11-11 to `api-server`. HTTP
routes are unchanged. The deployed database was empty, so the server now
uses a new file `api.sqlite` in the same volume; the old `gallery.sqlite`
is ignored.

| Old | New |
|---|---|
| image `vitrine-gallery` | image `vitrine-api` |
| Dockerfile `crates/gallery-server/Dockerfile` | `crates/api-server/Dockerfile` |
| binary `/usr/local/bin/vitrine-gallery` | `/usr/local/bin/vitrine-api` |
| compose service `gallery` | `api` |
| container user `gallery` (uid 10001) | `vitrine` (same uid 10001: files in the volume stay writable) |
| volume `gallery-data` → `/data` | unchanged |
| `/data/gallery.sqlite` | `/data/api.sqlite` (new, created on start) |
| `GALLERY_BIND` | `VITRINE_API_BIND` |
| `GALLERY_DATA_DIR` | `VITRINE_API_DATA_DIR` |
| `GALLERY_MAX_UPLOAD_MB` | `VITRINE_API_MAX_UPLOAD_MB` |
| `GALLERY_ADMIN_TOKEN` | `VITRINE_API_ADMIN_TOKEN` |
| `GALLERY_UPLOAD_TOKENS` | `VITRINE_API_UPLOAD_TOKENS` (now also `token:login:github_id`) |
| `GALLERY_AUTO_APPROVE` | `VITRINE_API_AUTO_APPROVE` |
| `GALLERY_TRUST_PROXY` | `VITRINE_API_TRUST_PROXY` |
| `GALLERY_RATE_LIMIT_PER_HOUR` | `VITRINE_API_RATE_LIMIT_PER_HOUR` |
| `GALLERY_PUBLIC_URL` | `VITRINE_API_PUBLIC_URL` |
| – | new: `VITRINE_API_READ_RATE_LIMIT_PER_HOUR`, `VITRINE_API_LOGIN_RATE_LIMIT_PER_HOUR`, `VITRINE_API_POLL_RATE_LIMIT_PER_HOUR`, `VITRINE_API_GITHUB_CLIENT_ID`, `VITRINE_API_GITHUB_CLIENT_SECRET`, `VITRINE_API_SESSION_DAYS`, `VITRINE_API_OFFICIAL_LOGINS`, `VITRINE_API_WEB_LOGIN_RETURN_URL` |

The old `GALLERY_*` variables are no longer read: a stack that still sets
only them starts with defaults (moderation off, no static tokens).

Steps (Portainer stack, compose):

1. Pull the repository and build the new image:
   `docker build -f crates/api-server/Dockerfile -t vitrine-api .`
2. Edit the stack (compare `crates/api-server/docker-compose.example.yml`):
   - rename the service `gallery:` → `api:`;
   - `image: vitrine-api:latest` (and, if the stack builds itself,
     `dockerfile: crates/api-server/Dockerfile`);
   - keep the volume line `gallery-data:/data` and the top-level
     `volumes: gallery-data:` exactly as they are (same stack = same Docker
     volume);
   - rename every `GALLERY_*` variable as in the table (values unchanged);
   - add `VITRINE_API_OFFICIAL_LOGINS: vitrinestudio-app` (the default, so
     optional) and, for the official templates,
     `VITRINE_API_UPLOAD_TOKENS: <openssl rand -hex 32>:vitrinestudio-app`;
   - optional now, needed for a website login later:
     `VITRINE_API_GITHUB_CLIENT_SECRET: <secret>` (GitHub → org
     `vitrinestudio-app` → Settings → Developer settings → OAuth Apps →
     the app → "Generate a new client secret"). The client id is built in.
3. Deploy the stack ("Update the stack"; Portainer replaces the
   `gallery` container by `api`).
4. nginx: unchanged when it proxies to `127.0.0.1:8080`. If it proxies by
   container/service name on a Docker network, change `gallery` to `api`.
5. Check: `curl https://api.vitrinestudio.app/health` →
   `{"status":"ok",…}`; `docker logs <api container>` shows
   `vitrine-api listening on 0.0.0.0:8080`; then on a workstation
   `vitrine gallery login` (code on github.com/login/device → "logged in
   as <you>") and `vitrine gallery list`.
6. Optional cleanup: remove the old image (`docker image rm
   vitrine-gallery`) and the empty old database
   (`docker exec <api container> rm -f /data/gallery.sqlite
   /data/gallery.sqlite-wal /data/gallery.sqlite-shm`).

Uploading the official templates afterwards: see "Uploading the ~50
official templates" above.

## GDPR / Impressum (to be done by the owner – notes, not legal advice)

- **Impressum** (§ 5 DDG): the API is a public service of Looking4Cache
  UG (haftungsbeschränkt) → link the website's Impressum from the gallery
  pages and from the API's responses/documentation.
- **Privacy policy** (Art. 13 GDPR) covering the gallery:
  - personal data processed: IP addresses (reverse proxy logs; the server
    keeps them only in memory for rate limiting, max. 1 hour), the
    uploader's GitHub login and numeric id (public, the login is shown as
    uploader), session records (hash of the session token, login, id,
    expiry; deleted on logout or expiry), the text of abuse reports (may
    contain contact data), download counters (no personal data);
  - legal bases: Art. 6(1)(b) for uploaders (providing the gallery),
    Art. 6(1)(f) for logs/rate limiting/abuse handling;
  - recipients: hosting provider; GitHub (sign-in; data transfer to the US
    under the EU-US Data Privacy Framework);
  - retention: proxy logs (set e.g. 7–14 days), reports until resolved +
    a short period, sessions up to `VITRINE_API_SESSION_DAYS`, uploader
    data until the account/templates are deleted;
  - rights: access, erasure (delete an uploader's templates, sessions and
    login on request – today by hand in SQLite + `templates/<id>/`),
    objection.
- **Terms of upload**: rights confirmation, CC0 grant, no third-party
  screenshots/vendor graphics, OFL fonts only; sample screenshots only
  when the uploader confirms they are their own (`--confirm-own-screenshots`); notice-and-takedown via the
  report endpoint and an e-mail address (DSA: point of contact, statement
  of reasons when removing content).
- **Cookies**: only the short-lived, strictly necessary login-state cookie
  of the web flow (`vitrine_oauth_state`, 10 minutes, deleted after the
  callback); no tracking. The website decides its own.
- The app sends nothing to the gallery unless the user browses, downloads
  or uploads.
