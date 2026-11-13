# Phase 7 – Release: first public version → 1.0

Scope: KONZEPT.md §12, Phase 7 (split out of Phase 6 on 2026-11-13; 3D moved
to Phase 8). Every remaining step from today's state to the first published
version (`v0.1.0` public) and on to 1.0: signing, publishing, legal, servers,
integrations, store test runs, real-app checks and open owner decisions.
Phase 6 holds the feature work (`docs/plan/phase-6.md`); this list is about
getting it out. Done when release 1.0 is published, the website with its
legal pages is live and the integrations point at a real release.

Status markers: `[ ]` open · `[~]` in progress · `[x]` done
Who: **Owner** = only the owner can do it (accounts, keys, money, legal,
publishing, his machines) · **Agent** = a Claude session can do it in the
repo · **Owner + Agent** = agent prepares, owner applies/approves.
"Needs" = prerequisite; "Doc" = where the step is explained.

Release order in short: fix Linux bundling → re-tag `v0.1.0` → test the draft
on all three OS → publish → ghcr image public → website live with downloads
and legal pages → integrations published → 0.1.x updates (tests the
updater) → 1.0. Windows ships unsigned (owner decision 2026-11-13, 7.1);
SignPath / Azure signing is deferred.

## 7.1 Code signing & notarization

### macOS (Developer ID + notarization)
- [x] **Owner** – Apple Developer account (Looking4Cache UG), Developer ID
      Application certificate as `.p12` → secrets `APPLE_CERTIFICATE`,
      `APPLE_CERTIFICATE_PASSWORD`, `APPLE_SIGNING_IDENTITY` (2026-11-11).
      Doc: `docs/dev/releasing.md` §1.
- [x] **Owner** – notarization key (App Store Connect API, role Developer)
      → secrets `APPLE_API_ISSUER`, `APPLE_API_KEY`, `APPLE_API_KEY_P8`
      (2026-11-11). Doc: `releasing.md` §2.
- [x] Tag run `v0.1.0`: universal app signed, notarized, stapled.
- [ ] **Owner** – keep the `.p12` export password and the `.p8` in the
      password manager; note the certificate's expiry date (Developer ID
      certificates run 5 years) in the calendar.

### Updater signing (Tauri)
- [x] **Owner** – `cargo tauri signer generate`; public key committed to
      `crates/gui/tauri.conf.json` → `plugins.updater.pubkey` (5389895);
      private key + password as secrets `TAURI_SIGNING_PRIVATE_KEY`,
      `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` (2026-11-11). Doc: `releasing.md` §3.
- [ ] **Owner** – offline backup of the private updater key (password
      manager + offline copy). Losing it means installed apps can never
      update again.

### Windows – unsigned for now (owner decision 2026-11-13)
- [x] **Owner** – decision: no Windows code-signing certificate for now.
      The Windows setup and `vitrine.exe` ship **unsigned** (no
      Authenticode signature) in the first releases; SignPath Foundation
      and Azure Artifact Signing are deferred (list below).
- [x] What it means for users: SmartScreen shows "Windows protected your
      PC" with "Unknown publisher" when the downloaded setup is started;
      **More info → Run anyway** installs it (per user, no admin rights).
      The release notes say "Windows: unsigned (SmartScreen warns on first
      start)" (`release.yml`), the website install page explains the steps
      (`website/content/{en,de}/docs/install.md`). Some antivirus tools may
      be stricter with unsigned files; the checksums in `SHA256SUMS.txt`
      let users verify the download.
- [x] Updater on Windows: unaffected. The Tauri updater checks its **own**
      minisign signature (`TAURI_SIGNING_PRIVATE_KEY` → `…-setup.exe.sig`,
      entry `windows-x86_64` in `latest.json`), not Authenticode; the
      unsigned build path creates that `.sig` like on macOS/Linux, so
      0.1.x updates install from the app. The update is downloaded by the
      app itself (no browser "downloaded from the internet" mark), so
      SmartScreen is not expected to ask again – to confirm in the updater
      test (7.3). Switching to a signed build later needs no change for
      installed apps (the updater key stays the same).
- [x] Workflow checked (2026-11-13): no change needed. `release.yml`
      switches the SignPath path on only when `SIGNPATH_API_TOKEN` **and**
      `SIGNPATH_ORGANIZATION_ID` are set (step "Detect signing material");
      without them the Windows job takes "Build and bundle (… Windows
      without SignPath)", still signs the updater artifact, and labels the
      build unsigned. The `sign` input of `workflow_dispatch` only means
      "use the secrets that exist". The `v0.1.0` tag run already built
      Windows this way.
- [ ] **Owner** – MFA for every member of the GitHub org (good practice
      anyway; SignPath would require it).

Deferred (revisit after the first releases have some reputation, or when
users ask; doc: `docs/dev/releasing.md` §4,
`docs/research/2026-11-11-windows-signing.md`):
- SignPath Foundation: ask about the clause "without commercial
  dual-licensing" vs. the CLA's relicensing right (ADR 0022); "Code
  signing policy" page on the website (agent writes it, owner fills the
  names); Windows version resource for `vitrine.exe` (e.g. `winresource`)
  + product name/version rule in `.signpath/artifact-configurations/binaries.xml`;
  apply at signpath.org/apply; then trusted build system, artifact
  configurations, signing policy, CI token and the `SIGNPATH_*` secret and
  variables (table below). The workflow path is ready and stays dormant.
- Alternative: Azure Artifact Signing (~$10/month, company validation)
  via `bundle.windows.signCommand` (agent wires it).

### Linux
- [ ] **Agent** – fix AppImage bundling on CI (`linuxdeploy` fails even with
      `APPIMAGE_EXTRACT_AND_RUN=1 NO_STRIP=true` + libfuse2). Needs: the
      verbose log tail of "Build and bundle" from the owner (Actions →
      Release v0.1.0 → Build linux) – or **Owner** decides to ship 0.1.0
      with deb/rpm only (STATUS hand-off).
- [ ] Linux packages stay unsigned (checksums in `SHA256SUMS.txt`); signed
      deb/rpm repositories are not planned (optional later).

### GitHub secrets/variables used by `release.yml` (environment `release`)
| name | kind | state |
|---|---|---|
| `APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`, `APPLE_SIGNING_IDENTITY` | secret | set |
| `APPLE_API_ISSUER`, `APPLE_API_KEY`, `APPLE_API_KEY_P8` | secret | set |
| `TAURI_SIGNING_PRIVATE_KEY`, `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | secret | set |
| `SIGNPATH_API_TOKEN` | secret | not set – Windows signing deferred (2026-11-13); leave empty |
| `SIGNPATH_ORGANIZATION_ID` (switches the SignPath path on), `SIGNPATH_PROJECT_SLUG`, `SIGNPATH_POLICY_SLUG`, `SIGNPATH_ARTIFACT_CONFIG_BINARIES`, `SIGNPATH_ARTIFACT_CONFIG_INSTALLER` | variable | not set – deferred; without them Windows builds unsigned |
| `GITHUB_TOKEN` (`packages: write`, ghcr image) | automatic | – |

- [x] **Owner** – environment `release` limited to `main` + tags `v*`
      (`releasing.md` "One-time setup").
- [ ] **Owner** – optional: "Required reviewers" on the environment.

## 7.2 GitHub organisation & repository
- [x] Repository `vitrinestudio-app/vitrine` with full history (2026-11-11).
- [ ] **Owner** – archive/delete the old repo `Looking4Cache/arrangia`
      (remote `old-arrangia`).
- [ ] **Owner** – check the repository is public (the Actions API answers
      without login, so it seems to be) and that the description, website
      link (www.vitrinestudio.app) and topics are set.
- [ ] **Owner** – branch protection for `main` (require CI incl. the
      `licenses` job, no force pushes); keep agents' merge workflow in mind
      (they push merges to `main`).
- [ ] **Owner** – CLA bot: CLA Assistant (cla-assistant.io: public gist with
      `CLA.md` 1.0, link the repo) **or** CLA Assistant Lite (copy
      `docs/dev/cla-workflow.yml` to `.github/workflows/cla.yml`, agent can
      do the copy). Doc: `CONTRIBUTING.md` "For the maintainer".
- [ ] **Owner** – GitHub Sponsors for the org (and/or Liberapay account);
      then **Agent**: `.github/FUNDING.yml`, real URLs in
      `website/hugo.toml` `[params.sponsors]`, `sponsors_available = true`.
- [ ] **Agent** – optional: issue templates (bug report with OS/version,
      feature request), `SECURITY.md` with a contact address – not planned
      anywhere yet; owner decides.
- [ ] **Owner** – after the first tag run with a push: ghcr package
      `vitrine-cli` → visibility **Public**, linked to the repo, Actions
      access Write; try `docker run --rm ghcr.io/vitrinestudio-app/vitrine-cli:0.1.0 vitrine --version`.
      Doc: `releasing.md` §5.

## 7.3 First release (v0.1.0) and the release process
- [x] Release workflow, draft releases, checksums, `latest.json`, licence
      notices, CLI archives, container image jobs (ADR 0026).
- [x] Tag `v0.1.0` pushed on 3152f19 (no draft release created: Linux failed).
- [ ] **Owner** – first `workflow_dispatch` run with `sign` on the current
      `main` (checks the new `cli`/`image` jobs, arm runners, musl smoke
      tests – never run on GitHub so far). Doc: `releasing.md` §6.
- [ ] **Owner + Agent** – re-tag: delete the `v0.1.0` tag (remote + local)
      and push it again on the current `main` (no release exists yet), or
      bump to `0.1.1`. Doc: `releasing.md` "Releasing a version".
- [ ] **Agent** – release highlights for the draft notes (no CHANGELOG file
      exists; decide whether to start one – **Owner**).
- [ ] **Owner** – smoke tests per OS on the draft's files:
  - macOS (Apple silicon + Intel if possible): DMG opens without Gatekeeper
    warning (`spctl -a -vvv -t install`, `xcrun stapler validate`), first
    start, start screen, open the example, export; keychain prompt on the
    first store-key import (only Vitrine's own entries); bundled CLI
    `…/Contents/MacOS/vitrine --version`; "Install command line tool";
    MCP: "Add to Claude Desktop" with the bundled CLI path; native menu.
  - Windows (unsigned, 7.1): SmartScreen "Unknown publisher" → More info
    → Run anyway, NSIS per-user install, PATH prompt, first start,
    keychain (Credential Manager), uninstall.
  - Linux: deb + rpm install (`/usr/bin/vitrine`), AppImage if built,
    WebKitGTK start, Secret Service keyring.
  - Everywhere: real file drop onto the window (asset import, folder →
    open project; never tested in Tauri), updates section in app settings.
- [ ] **Owner** – publish the draft; check `releases/latest/download/latest.json`.
- [ ] **Owner** – updater end to end: install 0.1.0, publish 0.1.1, the app
      offers the update, installs and restarts (macOS, Windows, AppImage;
      deb/rpm only notify). Windows: note whether SmartScreen asks again
      for the app-downloaded update (expected not to, 7.1).
- [ ] **Agent** – after publishing: website `downloads_available = true` +
      `[params.downloads]` URLs (`releasing.md` step 7), remove the
      "pre-release" `status` callouts (`install.md`, docs `_index.md`),
      README status.
- [ ] **Agent** – link the third-party notices from the About panel and
      `vitrine --help` (phase 6.5 "later").
- [ ] **Owner** – announcement (where: own channels, Mastodon/Bluesky,
      Hacker News / Product Hunt, fastlane community?) – decide.
- [ ] Later versions: version bump in `[workspace.package]`, tag, draft,
      publish (SignPath approvals only once Windows signing is set up); bump the GitLab component's default
      image tag on minor releases (`releasing.md`).

## 7.4 Integrations (MIT, separate homes)
All need a published release with CLI archives (7.3). Doc:
`integrations/README.md`, `releasing.md` "CI integrations",
`docs/research/2026-11-12-ci-integrations.md`.
- [x] GitHub Action pushed to `vitrinestudio-app/vitrine-github-action`
      (7d1645d; owner created the public repo).
- [ ] **Agent** – sync `integrations/github-action/` to its repo after the
      release, run its `Test` workflow against the real release.
- [ ] **Owner** – tag `v1.0.0` + move `v1`, publish the GitHub Marketplace
      listing from its release page (needs the owner's account to accept
      the Marketplace agreement, 2FA).
- [ ] **Owner** – create the gitlab.com group/project
      `vitrinestudio/vitrine-gitlab-ci`, mark it as CI/CD Catalog project;
      **Agent** pushes `integrations/gitlab/`; **Owner** tags `1.0.0`
      (catalog release). Needs: the public ghcr image.
- [ ] **Owner** – fastlane plugin: decide name/repo (proposed
      `vitrinestudio-app/fastlane-plugin-vitrine`), RubyGems account (MFA),
      **Agent** scaffolds (`fastlane new_plugin`, rspec, rubocop, CI) and
      runs the actions inside fastlane once; **Owner** `gem push
      fastlane-plugin-vitrine` (the plugin list picks it up by prefix).
      Optional decision: a `vitrine_upload` action.
- [x] **Owner** – repo `vitrinestudio-app/vitrine-agent-skill` created (2026-11-13) for the
      agent skill (`integrations/skill/`, MIT, open Agent Skills format; that folder is the
      repo root: `skills/vitrine-store-screenshots/SKILL.md`, README, LICENSE).
- [ ] **Agent** – before publishing: validate the skill against the spec (`skills-ref
      validate skills/vitrine-store-screenshots`, https://agentskills.io/specification:
      `name` = folder name, lowercase/hyphens, `description` ≤ 1024 chars, SKILL.md < 500
      lines); check `npx skills add <local path> --list` finds it; re-check the tool names
      against the released app; verify the per-client install notes in the README (marked
      "verify") and the licences of the sources in CREDITS.md.
- [ ] **Agent** – push `integrations/skill/` to the repo (folder = repo root; later syncs the
      same way as the GitHub Action).
- [ ] **Owner** – repo settings: public, description ("Agent skill: App Store & Google Play
      screenshots with Vitrine Studio over MCP"), topics (`agent-skills`, `mcp`,
      `app-store-screenshots`, `aso`, `vitrine-studio`), MIT licence detected.
- [ ] **Owner** – tag and release `v1.0.0` (GitHub release; attach a zip of
      `vitrine-store-screenshots/` for clients that take zip uploads, e.g. Claude Desktop /
      claude.ai). Users can pin it: `npx skills add vitrinestudio-app/vitrine-agent-skill@v1.0.0`.
- [ ] **Owner** – skills.sh: there is no submission form – a skill appears on the
      https://skills.sh leaderboard automatically through the anonymous telemetry of the
      `skills` CLI once people run `npx skills add vitrinestudio-app/vitrine-agent-skill`
      (skills.sh/docs/faq, checked 2026-11-13). So: run the install once yourself, then
      announce it. Optional: add the badge
      `[![skills.sh](https://skills.sh/b/vitrinestudio-app/vitrine-agent-skill)](https://skills.sh/vitrinestudio-app/vitrine-agent-skill)`
      to the README.
- [x] **Agent** – website: page "AI assistants" (en + de) gets an "Agent skill" section with
      the `npx skills add` command, the zip download for clients with skill upload, and a
      link to the repo; link it from the skill README.
- [ ] Optional (owner decision) – a Claude Code plugin marketplace entry
      (`.claude-plugin/marketplace.json` in a separate subfolder, so `/plugin marketplace add`
      works too; the `skills` CLI also discovers skills declared there) and a listing in
      other directories when they appear. Keep the skill itself vendor-neutral.
- [ ] **Owner** – crates.io: all crates have `publish = false`; decide
      whether to reserve/publish names (`vitrine-*` / `vitrine-cli`) or stay
      off crates.io (ADR 0022, KONZEPT §13).
- Optional, not planned anywhere (only on demand): Homebrew cask/tap,
      winget, Scoop, AUR, Flathub, Snap; Bitrise step, CircleCI orb, Azure
      DevOps extension (STATUS (g4)).

## 7.5 Website www.vitrinestudio.app
Doc: `website/README.md` (TODO section, deployment).
- [ ] **Owner** – hosting (Hetzner) and deploy: DNS for `www` and the apex
      `vitrinestudio.app` (redirect to `www`), TLS certificates, nginx
      (404 page, cache headers for fingerprinted CSS/JS/fonts/images),
      upload of `hugo --minify` output (SFTP pattern of the other sites) –
      or a CI deploy job (**Agent** can write one; owner provides the
      deploy key as a secret). Rebuild after approving templates (gallery
      snapshot).
- [x] **Owner + Agent** – legal pages: Impressum and Datenschutz complete
      (2026-11-13, merged a2e5b45; German binding, English translation;
      owner answers filled: info@grove.eco, Hetzner Germany + AVV,
      anonymised logs 30 days, full IP only for security events; no
      `[OFFEN]` left). The API server enforces the stated retention
      (rejected templates and dealt-with reports deleted at once,
      rate-limit counters and expired sessions pruned).
- [ ] **Owner** – configure the website's and the API's nginx log format
      and retention to match the privacy policy (anonymised IP, 30 days).
      Recommended: legal review (lawyer / IT-Recht-Kanzlei service).
- [x] **Agent** – terms of use for the template gallery
      (`/de/nutzungsbedingungen/` binding, `/terms/` translation: rights
      confirmation, CC0 grant, own sample screenshots only, OFL fonts only,
      notice-and-takedown, DSA contact; merged da3855c); CLI upload prints
      the link, `/health` has `terms_url`/`terms_version`.
- [ ] **Owner** – terms of use: fill the 9 `[OFFEN: …]` items per language
      (notice periods, session lifetime, age limit, how uploaders are
      notified – the gallery knows no e-mail addresses, Art. 19 DSA
      micro-enterprise check, consumer arbitration), remove the "Vor dem
      Livegang" callout, legal review. Notes: `docs/api/DEPLOY.md`
      "GDPR / Impressum".
- [ ] **Owner** – optional: an own contact mailbox (e.g.
      `info@`/`hello@vitrinestudio.app`); the legal pages use
      `info@grove.eco` today.
- [ ] **Owner** – review texts and design (en + de); German UI wording
      questions (see 7.9).
- [x] **Agent** – screenshots: all app screenshots retaken with the
      current UI as light/dark pairs (2026-11-13, 520b4b0; no `placeholder`
      shortcodes left in the docs, README editor shots as `<picture>`),
      home matrix with real renders from the Warm Stay template (9cfa24f),
      new page-grouping image.
- [ ] **Owner + Agent** – a dedicated 1200×630 Open Graph image (today a
      crop of `iphone-gallery-en.jpg`); optionally showcase tabs from the
      gallery templates; update the stale "Images to replace" table in
      `website/README.md`.
- [ ] **Agent** – the downloads switch (7.3). ("Code signing policy" page
      only with SignPath, deferred – 7.1.)
- [ ] **Agent** – link to the website guide in the MCP `docs_get`
      `overview` once the site is online (phase 6.7).
- Search console / analytics: not planned (site has no trackers).

## 7.6 API server api.vitrinestudio.app
Doc: `docs/api/DEPLOY.md`, `docs/api/API.md`, ADR 0023, 0025.
- [x] Deployed (Docker/Portainer, nginx), GitHub login verified
      (2026-11-12); upgrades 2026-11-12 applied; 55 templates live
      (2026-11-13).
- [ ] **Owner** – check the stack settings after the bulk upload:
      `VITRINE_API_RATE_LIMIT_PER_HOUR` back to the default 20 (raised for
      the bulk upload), `VITRINE_API_MAX_UPLOAD_MB=100` with nginx
      `client_max_body_size 110m` (+ `proxy_read_timeout` /
      `client_body_timeout` for slow uploads), `VITRINE_API_CORS_ORIGINS=https://www.vitrinestudio.app,https://vitrinestudio.app`,
      `VITRINE_API_TRUST_PROXY=1`, no duplicate CORS headers from nginx;
      `curl https://api.vitrinestudio.app/health` shows the limit.
- [ ] **Owner** – secrets: `VITRINE_API_ADMIN_TOKEN` and the official
      upload token only in the stack env / password manager; rotate the
      upload token if it was ever pasted somewhere; admin CLI only via
      `--admin-token-env`.
- [ ] **Owner** – GitHub OAuth App (org `vitrinestudio-app`): homepage URL,
      callback `https://api.vitrinestudio.app/v1/auth/github/callback`,
      logo; `VITRINE_API_GITHUB_CLIENT_SECRET` only if the web login is
      used (website upload/login – not built yet).
- [ ] **Owner** – backups of the volume (`sqlite3 .backup` + `templates/`),
      scheduled and tested once with a restore.
- [ ] **Owner** – monitoring: uptime check on `/health` (e.g. Uptime Kuma),
      disk space, container restart policy, log retention matching the
      privacy policy.
- [ ] **Owner** – moderation routine: `vitrine gallery admin queue|approve|reject`,
      answer abuse reports; delete data on request (by hand today).
- [x] Deletion of rejected templates (row, reports, files) and resolved
      reports at once, legacy rows purged at start-up (2026-11-13, 5690935).
- [ ] **Agent** – gallery follow-ups (after 1.0 unless needed): uploader
      self-service (own templates, delete), GUI upload/login (CLI only
      today), technical uploader ban, gallery rule for templates with
      "required, not included" fonts (7.9).
- [ ] **Owner** – rebuild/redeploy the server image with every release
      that changes `crates/api-server` or the template format.
- [ ] **Owner** – redeploy now for the 2026-11-13 server changes (terms
      link in `/health`, immediate deletion of rejected templates and
      reports, pruning of counters/sessions).

## 7.7 Store accounts & real upload runs (from Phase 5)
Doc: `docs/plan/phase-5.md`, ADR 0016–0019, website `docs/store-upload`.
- [ ] **Owner** – answer open questions (c) test apps (App Store Connect app,
      Play package) and (d) keys (ASC API key role App Manager; Play
      service account with "Manage store presence").
- [ ] **Owner** – `vitrine upload refdata` once with the real key; agent
      checks `crates/core/data/asset-library.yaml` against the output.
- [ ] **Owner** – re-run the real Apple upload with the 5.5c fixes
      (`auto`); then a test project with `upload_flow: asset_library`
      (iPhone Duo, header/search creatives).
- [ ] **Owner** – first real Google Play run (incl. feature graphic).
- [ ] **Owner** – decide (a): switch the Apple default to the App Asset
      Library after a verified run (keep screenshot sets as fallback).
- [ ] **Owner** – look at the store UI in the real app: app settings →
      Stores (help panels, Test connection with real keys), project
      settings → Stores, upload dialog + MCP confirmation dialog.
- [ ] **Owner** – optional CI check: `upload plan` + `execute --confirm … --yes`
      with `VITRINE_ASC_*` / `VITRINE_PLAY_*` in a private repo.

## 7.8 Real-app checks (built, only seen in the dev bridge)
**Owner** in the signed build (or `cargo run -p vitrine-gui`); findings go
back to agents.
- [ ] Apple bezels: re-check after the 2026-11-10 colour fixes; iPad / Mac
      packages (rules unverified); store the iPhone 18 bezels in Fernwood,
      re-import the package once (so `License.rtf` lands next to the
      frames), CI export with an empty `VITRINE_APP_DIR` (ADR 0027).
- [ ] Colour management: a real Display P3 iPhone screenshot (image
      preview shows the colour space).
- [ ] Frame packages dialog (synthetic pack:
      `cargo run -p vitrine-core --example synthetic_frame_pack -- /tmp/clay.zip`),
      inspector frame picker, canvas pane "banner from this canvas".
- [ ] Templates: New project → Local / Template gallery tabs (download +
      create), "Save as template…" (menu entry, tags, samples), canvases
      dialog "From template".
- [ ] Fonts: font pickers, project settings → Fonts (store system fonts).
- [ ] GUI polish: native menu order + Help submenu, menu button on icon/name,
      inspector drag handle in WebKit, image preview (Esc, arrows), page
      backgrounds, canvases & variants dialog, context menu + clipboard,
      quick guide on first open, export dialog options (fastlane layout,
      `index.html`).
- [ ] MCP: "Add to Claude Desktop" (re-add as `vitrine-studio`, remove the
      old `arrangia` entry), `claude mcp add` for Claude Code, Claude
      Desktop's real `clientInfo.name`, `mcp.json` removed on ⌘Q.
- [ ] Native store app gallery gaps measured on real devices (KONZEPT §13,
      `galleries.yaml` `not yet measured`) – optional before 1.0.

## 7.9 Open product decisions (owner, "to confirm")
Compact list; details in `docs/plan/STATUS.md` ("Open owner decisions",
"Open questions", "Decisions taken with the user") and the named ADRs.
- [ ] Release defaults: universal macOS build, no MSI, per-user NSIS,
      macOS ≥ 11, daily update check on, app not sandboxed (ADR 0026).
- [ ] Ship 0.1.0 without AppImage if the fix takes long (7.1).
- [ ] Apple upload default flow → asset library after a real run; when to
      drop the screenshot-set API (question (a), ADR 0019).
- [ ] MCP upload policy (question (b)): GUI click, headless off by default.
- [ ] Templates (question (f)): placeholders not copied into new projects;
      insert default "keep both"; no stored previews for built-ins;
      gallery template kept in the app store after download (2026-11-13).
- [ ] Gallery defaults: sample limits 10 MB / 6000 px, WebP q80, empty
      facets allowed locally, transparent samples stay PNG; rule for
      templates with "required, not included" fonts (question (h2):
      recommendation allow + note in the detail view).
- [ ] Fonts (question (h), ADR 0028): one "Use installed fonts" setting;
      RTL alignment physical vs. `start`/`end`; CJK font packs now / later
      / never (`docs/research/2026-11-12-cjk-font-packs.md`); keep parley
      `complex-scripts` (+3.4 MB) or drop Thai line breaking.
- [ ] Design checks (ADR 0029): on by default in validate, thresholds,
      `allow:` naming; group `scale` details (ADR 0030: origin = pivot,
      `at` unscaled, ungroup refused).
- [ ] Stored Apple frames (ADR 0027): layout, no `project.yaml` field,
      project copy wins, no MCP tool, names `embed`/`unembed`.
- [ ] fastlane defaults (2026-11-10): layout folder `export/fastlane/`,
      first claim wins on set conflicts, `index.html` on, stale cleanup;
      (g5) fastlane's `APP_STORE_CONNECT_API_KEY_*` not read implicitly.
- [ ] Proposed, not built: `{lang}` placeholder in image asset paths
      (P2.3); GUI in `--release`/`opt-level = 1` for long MCP sessions.
- [ ] German wording "Frame" vs. "Rahmen"; translate device/colour names?
- [x] Windows signing: none for now, ships unsigned (2026-11-13, 7.1);
      SignPath vs. the CLA's relicensing clause only matters if SignPath
      is taken up again.
- [ ] crates.io names (7.4); fastlane plugin name/repo (7.4); announcement (7.3).
- [ ] Later integrations (Bitrise, CircleCI, Azure) only on demand (g4).

## 7.10 Agent work packages of 2026-11-13 (all merged)
- [x] Design checks round 2 (WP-C): `device_cut`, `screen_covered`
      (`checks.screen_cover`), clearer contrast/header-cut messages,
      corner occluders (ADR 0029 amendment; c968e8b) – MCP test round 2,
      `docs/research/2026-11-13-mcp-test-results2.md`.
- [x] `clip_to` for images + span highlight padding/radius (F2, ADR 0031;
      198a602), scaled by group `scale` (ADR 0030, d454387).
- [x] MCP tool surface (WP-A): `apply_ops {file}`, dropped-override
      warning, `styles_update {fonts}`, gallery `pages` (c829456); quieter
      checks (WP-B): own screenshot variant folders, `page_not_covered`
      looks underneath, no-op edits not saved (80ca617).
- [x] Gallery terms of use (7.5; da3855c).
- [x] fastlane folder flow: captures in their own folder, import skips
      own exports, export reports foreign images, `--remove-foreign`
      (ADR 0032; 5c453e3).
- [x] Legal pages completion + API retention (7.5; a2e5b45).
- [x] Template language fix: language-only elements remapped, never
      `visible_in: {}` (d05f3d3).
- [x] Website: Automation docs section, home page update, new light/dark
      app screenshots (website + README), Warm Stay matrix, floating
      narrow screenshots, Home link + section index (ce43fba … aa1cfdb).

## 7.11 Before tagging 1.0
- [ ] All of 7.1–7.8 done or consciously moved after 1.0; 7.9 answered.
- [ ] `cargo deny check licenses` green, third-party notices regenerated,
      golden tests green on all three OS.
- [ ] Docs: README status "1.0", AGENTS.md "Current state", KONZEPT §12, STATUS.md,
      website "pre-release" callouts removed, release notes.
- [ ] Version `1.0.0` in `[workspace.package]`, tag `v1.0.0`, publish,
      announce.
