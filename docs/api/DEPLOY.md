# Deploying the template gallery (`api.vitrinestudio.app`)

The owner deploys; agents never do. Backend: `crates/gallery-server`
(ADR 0023, API: `API.md`).

## Steps

1. **DNS**: `api.vitrinestudio.app` → the Docker server (A/AAAA record).
2. **Build** on the server (or in CI and push to a registry) from a
   checkout of the repository:
   `docker build -f crates/gallery-server/Dockerfile -t vitrine-gallery .`
3. **Secrets** in a `.env` next to the compose file (not in Git):
   `GALLERY_ADMIN_TOKEN=<long random>` (e.g. `openssl rand -hex 32`),
   optionally `GALLERY_UPLOAD_TOKENS=<random>:<github-login>,…` for testers
   until GitHub login exists.
4. **Run**: `docker compose -f crates/gallery-server/docker-compose.example.yml up -d`
   (binds `127.0.0.1:8080`, data in the volume `gallery-data` = `/data`:
   `gallery.sqlite` + `templates/<id>/`).
5. **Reverse proxy** (TLS, HTTP/2, limits): forward `api.vitrinestudio.app`
   to `127.0.0.1:8080`, set `X-Forwarded-For` (the app trusts it with
   `GALLERY_TRUST_PROXY=1`), request body limit ≥ `GALLERY_MAX_UPLOAD_MB`,
   and optionally an extra rate limit. Example (Caddy):
   ```
   api.vitrinestudio.app {
       request_body { max_size 55MB }
       reverse_proxy 127.0.0.1:8080
   }
   ```
   (nginx: `client_max_body_size 55m; proxy_set_header X-Forwarded-For $remote_addr;`)
6. **Check**: `curl https://api.vitrinestudio.app/health`.
7. **Backups**: the volume (`gallery.sqlite` – use `sqlite3 .backup` or stop
   the container – and `templates/`).
8. **Updates**: pull, rebuild, `docker compose up -d`; the schema is created
   with `CREATE TABLE IF NOT EXISTS` (migrations come with the first change).

Environment variables (all optional): `GALLERY_BIND` (`0.0.0.0:8080`),
`GALLERY_DATA_DIR` (`/data`), `GALLERY_MAX_UPLOAD_MB` (50),
`GALLERY_ADMIN_TOKEN`, `GALLERY_UPLOAD_TOKENS`, `GALLERY_AUTO_APPROVE`
(0; testing only), `GALLERY_TRUST_PROXY` (0), `GALLERY_RATE_LIMIT_PER_HOUR`
(20 uploads/reports per client), `GALLERY_PUBLIC_URL`
(`https://api.vitrinestudio.app`).

## GDPR / Impressum (to be done by the owner – notes, not legal advice)

- **Impressum** (§ 5 DDG): the gallery is a public service of Looking4Cache
  UG (haftungsbeschränkt) → link the website's Impressum from the gallery
  pages and from the API's responses/documentation.
- **Privacy policy** (Art. 13 GDPR) covering the gallery:
  - personal data processed: IP addresses (reverse proxy logs; the server
    keeps them only in memory for rate limiting, max. 1 hour), the
    uploader's GitHub login/id (public, shown as uploader), the text of
    abuse reports (may contain contact data), download counters (no
    personal data);
  - legal bases: Art. 6(1)(b) for uploaders (providing the gallery),
    Art. 6(1)(f) for logs/rate limiting/abuse handling;
  - recipients: hosting provider; GitHub (login, once implemented; data
    transfer to the US under the EU-US Data Privacy Framework);
  - retention: proxy logs (set e.g. 7–14 days), reports until resolved +
    a short period, uploader data until the account/templates are deleted;
  - rights: access, erasure (delete an uploader's templates and login on
    request – today by hand in SQLite + `templates/<id>/`), objection.
- **Terms of upload**: rights confirmation, CC0 grant, no third-party
  screenshots/vendor graphics, OFL fonts only; notice-and-takedown via the
  report endpoint and an e-mail address (DSA: point of contact, statement
  of reasons when removing content).
- **No cookies / tracking** in the API; the website decides its own.
- The app sends nothing to the gallery unless the user browses or uploads.
