# Phase 6 – Polish → release 1.0: task list

Scope: KONZEPT (`docs/archive/KONZEPT-de.md`, superseded by `ARCHITECTURE.md`) §12, Phase 6 ("Vorlagen, eigene Frame-Pakete,
Farbmanagement P3 → sRGB, Feature Graphic, Installer/Signierung, Website
vitrinestudio.app, Doku"). Done when release 1.0 ships.

State 2026-11-13: the features of this phase are built and merged; what remains are
mostly owner steps (signing, publishing, legal, real-app checks) and the
release itself. Those are collected, with every other remaining step to
the first public version, in **`docs/plan/phase-7.md`** (release). Open
items below point there.

User decisions 2026-11-10: work on Phase 6 in parallel with the Phase 5
real-run tests; split **website** and **documentation** into separate
steps; colour management is started right away; installer/signing is
tackled on 2026-11-11 (prepared here); templates come with a web gallery to
upload to and download from.

Status markers: `[ ]` open · `[~]` in progress · `[x]` done

## 6.1 Colour management P3 → sRGB (done 2026-11-10, ADR 0021)
- [x] Read embedded colour information of imported images: PNG `iCCP`,
      `sRGB`, `cICP` chunks; JPEG APP2 ICC (`core::colorspace::sniff`);
      HEIC/WebP are not accepted as inputs, so nothing to do there
- [x] Convert Display P3 (and any RGB/gray ICC profile, SDR cICP) to sRGB
      once when an asset is decoded (asset cache) – `moxcms` 0.8.1 (already
      in the tree via `image`; compared with `qcms`/`lcms2` in ADR 0021);
      untagged images = sRGB; `gAMA`/`cHRM` ignored
- [x] Output: PNG keeps the `sRGB` chunk, JPEG untagged (= sRGB); "Display
      P3 output" not now (reasoning in ADR 0021)
- [x] Same pipeline for GUI preview, export, upload, MCP and thumbnails
      (`raster::SourceImage::decode`); imported bezels too (also the
      swatch measurement in `frame_import`)
- [x] Colours in the model (`#RRGGBB`) stay sRGB; documented (NOTES, ADR)
- [x] Tests: P3/gray profiles generated with moxcms (no vendor material):
      P3 red clips to sRGB red, gray stays gray, mid colour vs. matrix
      reference, 16-bit, JPEG ICC, cICP, broken profiles; golden images
      unchanged (fixture screenshots are `sRGB`-chunk only)
- [x] Validation info `color_profile_unsupported` when a screenshot carries
      a profile we cannot use (unreadable, CMYK, HDR cICP …)
- [ ] Look at a real iPhone P3 screenshot and the imported iPhone 18 bezels
      in the GUI (user; image preview shows the colour space since
      2026-11-12) → Phase 7

## 6.2 Feature graphic (Google Play)
Largely there: target preset `play-feature-graphic` (1024×500, banner,
`slot: google.image_type featureGraphic`, rules `google-feature-graphic`)
and the Play client knows `featureGraphic`. Close to Apple's header /
search-results creatives (banners from a canvas region).
- [x] Check end to end (2026-11-11): example target `play-feature-graphic`
      (phone canvas, `banner` variant, Android frames) + golden; exports are
      always opaque (PNG RGB 8-bit / JPEG), validation checks size/format;
      upload plan + Play upload as one `featureGraphic` image per language
      (test `feature_graphic_uploads_as_one_opaque_png`, fake Play); specs
      re-checked against Google's and Apple's pages (targets.yaml G1/G4/AC1)
- [x] Quick action "banner from this canvas" (2026-11-11): app commands
      `canvas_banner_options` / `target_add_banner` (page range fitted to
      the banner aspect, cover, no gaps, screenshots/device map/variant of
      the same store's target), GUI canvas pane, MCP `banner_add`; no CLI
      (the CLI does not edit projects)
- [~] Owner: look at the canvas pane in the real app (open → Phase 7);
      decide whether the example's feature graphic should drop the phones
      (Google: "avoid device imagery") – decided 2026-11-11: Fernwood keeps
      its phones for now

## 6.3 Templates (+ web gallery) – app side done 2026-11-11, gallery backend skeleton (ADR 0023)
KONZEPT §9 "Vorlagen": duplicate a canvas, save as template, new project
from template. User 2026-11-10: templates should be uploadable to the web
and downloadable from a gallery.
- [x] User 2026-11-11: a template may contain **several canvases** (e.g.
      Phone + Tablet + feature graphic as one coordinated set), each with
      its targets and variants, sharing styles/texts; a single canvas is
      just the smallest case. "New project from template" takes all;
      "insert from template" lets the user pick which canvases.
- [x] Template format: a project folder subset (canvas(es), styles, fonts,
      placeholder screenshots) + `template.yaml` (name, author, licence,
      preview images, required fonts, tags); packed as `.vitrine-template`
      (zip); no vendor graphics, fonts only with redistributable licences
- [x] Save canvas / project as template (GUI + CLI + MCP), new project from
      template (built-in + local + downloaded), insert canvas from template
- [x] App-wide template store (`<app data>/templates/`), list/remove; built-in `phone`/`minimal` are templates of the same format
- [~] Web gallery on vitrinestudio.app (see 6.6): browse, preview, download;
      upload flow. User 2026-11-10: **own backend** (e.g. `api.vitrinestudio.app`
      on the user's Docker server; user also runs PHP and Node.js
      elsewhere). Stack (user OK 2026-11-10): Rust (axum) in
      this workspace (`crates/api-server`, formerly `gallery-server`), Docker image, SQLite, files
      on disk, behind the user's reverse proxy – reuses `core` validation and
      `render` to build trusted preview images server-side. Uploader login
      via GitHub OAuth (no passwords), moderation queue, licence per
      template fixed to **CC0** (user 2026-11-10: no licence choice; attribution/NC make no sense for store screenshots) – uploader confirms own rights + CC0 grant in the upload terms; fonts keep their own licence (OFL only). Licence file per font (user OK 2026-11-10): kept if present; otherwise generated when saving a template – copyright line (name ID 0, plus Reserved Font Name if stated) from the font + the standard OFL 1.1 text – only if the font itself declares OFL (name IDs 13/14); fonts without an OFL declaration are refused ("may not be redistributed / licence unclear"); the server re-checks on upload; size limits, safe zip handling, rate limits, abuse report,
      GDPR/Impressum; downloads without an account; in-app "browse gallery" opens the site or
      fetches an index (network → `publish`-like crate, not core)
- [x] User 2026-11-11: browsing and downloading the gallery need **no
      account**; only publishing needs the GitHub login. The "New project"
      dialog shows the server's templates next to the local ones and
      downloads/installs on click (offline → quiet "gallery not reachable").
      Done 2026-11-13: New project → tabs "Local templates" | "Template
      gallery" (facet filters, previews, badges, unreachable state; a
      downloaded template stays in the app store – agent's choice, to
      confirm); not yet looked at in the real Tauri window.
- [x] Licence/attribution per template; checks on import (size limits, no
      executable content, path traversal)

- Done 2026-11-11 (ADR 0023): `core::template` (package, safe import,
      OFL check + licence generation, placeholders, extract, create
      project, insert with merge rules, store, built-ins),
      `render::template` previews, app commands `template_*`, CLI
      `vitrine template save|list|import|export|remove|insert|new` + `new
      --template <name|file>`, MCP `template_list|save|insert|import`, GUI
      (template cards in New project, "Save as template…", canvases dialog
      "From template"; not yet looked at in the real app).
- [x] Gallery backend `crates/api-server` (renamed from `gallery-server`
      2026-11-11; axum, SQLite, Docker): list/search/get/download/previews
      (public, rate limited), upload with validation (server regenerates
      placeholders and previews), abuse reports, moderation queue, rate
      limits; GitHub login (device + web flow, session tokens, ownership
      by GitHub id, official flag; ADR 0025); client `publish::gallery`,
      CLI `vitrine gallery`, app commands `gallery_*` – tested in-process.
      TODO: ~~in-app "browse gallery" (GUI)~~ (done 2026-11-13),
      uploader self-service, deletion of rejected files, ~~the website
      pages~~ (done 2026-11-12). Deployment by the owner:
      `docs/api/DEPLOY.md` – running on api.vitrinestudio.app (GitHub
      login verified 2026-11-12). Remaining open items (self-service,
      rejected-file deletion, upload terms text) → Phase 7.
- [x] Owner 2026-11-12: **sample screenshots** in templates (save option,
      marker `screenshots/samples.yaml`, info `sample_screenshot`, upload
      plan blocks, server keeps them only when confirmed), **fixed tag
      facets** style/color/use + derived devices
      (`data/template-tags.yaml`; CLI, GUI chips, MCP enums, server filters,
      `GET /v1/tags`), **WebP gallery sprites** per canvas (API for the
      website gallery), **bulk upload + dry run + admin CLI**, DB migration
      (schema 1) + backfill + admin `rerender` (ADR 0023/0025 amendments).
      [x] owner: server upgrade, upload the ~50 templates (55 templates
      live in the gallery on 2026-11-13); [x] website gallery on the
      sprite API (2026-11-12). Template size (2026-11-12): JPEG q60
      samples, 250 MB client limit, 413 `too_large`.

## 6.4 Own frame packages
KONZEPT §4.2 (frames, point 3): any PNG or SVG frame plus a small
`frame.yaml` (screen rectangle, mask / corner radius, model mapping) in the
project's `frames/` or app-wide. Vitrine Studio ships no vendor images.
- [x] `frame.yaml` format `vitrine-frame/1` (ADR 0024, reference
      `docs/guide/frame-packs.md`): images per orientation/colour/bezel,
      detected screen or `screen` + `radius` or `mask`, `cut_screen` for
      painted screens, `models` (empty = any device: new devices/sizes)
- [x] Sources: project `frames/<id>/` and app-wide `<app data>/frames/packs/<id>/`
      (project wins), compiled caches with stamps; `frame: pack:<id>`
- [x] SVG frames via `resvg` (`vitrine_render::frame_packs::ResvgRasterizer`)
- [x] Synthetic test pack (`synthetic::write_pack`, example
      `synthetic_frame_pack`), tests in core/render/app/CLI
- [x] Vendor research `docs/research/2026-11-11-vendor-frames.md`
- [ ] Follow-ups: recompile on file changes in `frames/` while a project is
      open (watcher); rotate portrait frames for landscape devices; share
      packs through the gallery (6.3/6.6) – after 1.0
- [x] User 2026-11-10: agreed as described (PNG/SVG + `frame.yaml`, project
      `frames/` or app-wide, reuse of the bezel import machinery)
- [x] Use cases (documented in `docs/guide/frame-packs.md` and the
      vendor research): vendor kits the user downloads (Samsung, Google Pixel,
      Figma exports), clay/flat mockups, branded frames; could later be
      shared through the same web gallery as templates
- [x] GUI: frames manager lists packs; import pack folder/zip (this
      computer / this project); inspector + project default frame pickers
      (not looked at in the real app yet)
- [x] CLI `frames import <pack> [--to] [--id]`, `frames list --packs`,
      `frames remove --pack`; MCP `frames_list` returns packs

## 6.5 Installer & signing – prepared 2026-11-11 (ADR 0026, `docs/dev/releasing.md`)
- [x] Bundles via Tauri bundler: macOS universal `.app` + `.dmg` (min.
      macOS 11), Windows NSIS setup (per user; no MSI), Linux AppImage +
      deb + rpm (Ubuntu 22.04); stable asset names without version
- [x] Ship the CLI with the GUI: Tauri sidecar (`externalBin`, release-only
      config `tauri.bundle.conf.json`, `scripts/release/build-cli-sidecar.sh`);
      `claude_desktop::cli_path()` finds it next to the GUI binary (AppImage:
      `vitrine` on PATH); "Install command line tool" (app settings,
      `vitrine install-cli`): macOS symlink `/usr/local/bin/vitrine` (admin
      prompt when needed), Windows user PATH (+ NSIS installer prompt),
      deb/rpm `/usr/bin`; standalone CLI archives in the release
- [x] macOS: Developer ID signing + notarization via Tauri (hardened
      runtime, entitlements file – empty on purpose, app not sandboxed,
      see ADR 0026; App Store Connect API key; stapling), sidecar signed
      too; unsigned builds still succeed – [x] owner: certificate + key as
      secrets in the GitHub environment `release` (2026-11-11); tag run
      `v0.1.0`: macOS signed + notarized OK, Windows OK (unsigned), Linux
      AppImage bundling fails (linuxdeploy) – see STATUS hand-off; the
      tag exists, no draft release yet → Phase 7
- [x] Windows: SignPath Foundation researched
      (`docs/research/2026-11-11-windows-signing.md`), SignPath path in the
      workflow (two requests, re-signed updater artifact), switched on by
      the `SIGNPATH_*` variables; until then unsigned + labelled – owner
      decision 2026-11-13: **no Windows certificate for now**, Windows
      ships unsigned (updater unaffected), SignPath/Azure deferred
      (Phase 7.1)
- [~] Linux: desktop file + icons from the Tauri bundler; packages unsigned
      (checksums); AppImage self-updates, deb/rpm only notify – AppImage
      build fails on CI (linuxdeploy), deb/rpm untested → Phase 7
- [x] Release workflow `.github/workflows/release.yml` (tag `v*` → draft
      release with checksums, notices, `latest.json`; `workflow_dispatch` →
      artifacts only, optional signing); pinned tools (Tauri CLI 2.12.4,
      cargo-about 0.9.3, cargo-deny 0.20.2, SHA-256 checked)
- [x] Updater: `tauri-plugin-updater` 2.14.1 from Rust, GitHub Releases
      `latest.json`, `requireSignedVersion`; "Check for updates…" (menus +
      app settings), daily automatic check (setting, default on), user
      confirms install + restart; off while the public key is the
      placeholder – [x] owner: key generated, public key committed
      (5389895), private key + password as secrets (2026-11-11); backup in
      the owner's password manager advised
- [x] Version scheme: workspace version is the single source (0.1.0);
      release job checks tag = `v<version>`; release steps in
      `docs/dev/releasing.md`. App icon set exists (`crates/gui/icons/`).
- [x] Licence decision: GPL-3.0-or-later + CLA (2026-11-11, ADR 0022;
      `LICENSE`, `CLA.md` 1.0 (reviewed by the owner 2026-11-11), `CONTRIBUTING.md`, manifests, Tauri bundle
      `publisher`/`copyright`/`license`, CLI help + About panel notice)
- [x] Third-party licence notices with every installer: `about.toml` +
      `scripts/release/third-party-notices.sh` (cargo-about →
      `THIRD-PARTY-NOTICES.html`, `LICENSE.txt`, fonts' `OFL.txt`,
      hyphenation `LICENSES.md`) bundled under `licenses/`, in every CLI
      archive and attached to the release; `deny.toml` + CI job `licenses`
      (`cargo deny check licenses`); release notes name the tag as the
      corresponding source. [ ] Later: link the notices from the About
      panel / `vitrine --help`.
- [x] CI hardening: job timeouts, apt retries + step timeout (ci.yml)
- [x] Owner: review of `CLA.md` (2026-11-11)
- [ ] Owner: set up the CLA bot (CONTRIBUTING.md) → Phase 7
- [x] Added 2026-11-12: Linux aarch64 + static musl CLI archives, CLI
      container image `ghcr.io/vitrinestudio-app/vitrine-cli` (release
      jobs `cli`/`image`), CI integration drafts in `integrations/`
      (GitHub Action, GitLab components, fastlane plugin; MIT) – not yet
      run on GitHub / not published → Phase 7

## 6.6 Website vitrinestudio.app (own step) – first version 2026-11-11
- [x] Static site: Hugo (extended ≥ 0.154, the owner's generator) in
      `website/`, no theme, own layouts/CSS/JS, no npm; en (default) + de
      (`/de/`); light/dark; self-hosted font subsets; no cookies, trackers
      or third-party requests (`website/README.md`)
- [x] Landing page: animated "continuous canvas" hero, how it works,
      language × device matrix, editor, features, example showcase, AI/MCP,
      fastlane, store upload, templates teaser, open source + CLA, FAQ
- [x] Download page ("coming soon" until 6.5; `downloads_available` +
      URLs in `hugo.toml`), fastlane migration page (docs)
- [x] Impressum + Datenschutz complete (2026-11-13, owner answers
      filled; German binding, English translation); gallery terms of use
      drafted (`/de/nutzungsbedingungen/`, `/terms/`; owner fills the
      `[OFFEN: …]` items); domain/hosting/deployment (SFTP pattern of the
      other sites, see `website/README.md`) and review of texts and design
      open → Phase 7
- [x] Screenshots made with Vitrine Studio itself (2026-11-13): all app
      screenshots retaken with the current UI (light + dark, following the
      site theme), home matrix rendered from the Warm Stay template;
      dedicated Open Graph image still open → Phase 7
- [x] Template gallery page (2026-11-12): `/gallery/` + `/de/galerie/`,
      build-time snapshot + live refresh from the API (CORS allow list
      `VITRINE_API_CORS_ORIGINS`, list limit 200, per-canvas `devices`),
      store-gallery cards from the sprites (floating pages, pointer tilt/pan,
      lazy), local filters style/colour/use/device + search with FLIP
      animation and URL query, detail view `#<id>` (canvases, pages,
      download, how to use), home page live cards (`website/README.md`)
- [~] Owner: set `VITRINE_API_CORS_ORIGINS` on the server, upload/approve
      templates (done, 55 live), rebuild + deploy the site (→ Phase 7)
- [x] 2026-11-13: home page update (hero callouts, AI + CI/CD sections,
      fonts tile, "how development is funded" with sponsor placeholders,
      `sponsors_available = false`), docs section **Automation**
      (GitHub Actions, GitLab CI, fastlane, other systems; old URLs
      redirect), owner review of the German docs applied; header "Home"
      link and a section index on the home page, narrow screenshots float
      beside the text
- [ ] Frame pack gallery (6.4) on the website – after 1.0

## 6.7 Documentation (own step)
- [x] User guide from `docs/guide/NOTES.md` and the code (2026-11-11):
      17 English pages in `website/content/en/docs/` – how it works,
      install & update, getting started walkthrough, editor, devices &
      frames, targets & store formats, styles & fonts, languages &
      translation, screenshots, export, fastlane migration, store upload,
      templates & gallery, AI assistants, CLI reference, project file
      format, troubleshooting & FAQ, keyboard & mouse; full German
      translation in `website/content/de/docs/` (German UI labels, "du",
      English anchor ids). Pre-release state marked with `status` callouts.
- [x] CLI reference generated from the real `--help` output
      (`scripts/docs/cli-help.sh` → `website/data/cli_help.json`,
      shortcode `cli-help`); link checker `website/scripts/check-links.py`
- [x] Screenshots of the real app: the 11 placeholders per language
      replaced by light/dark screenshots (2026-11-13); owner reviewed the
      German texts 2026-11-13 (fixes merged)
- [ ] `docs_get` MCP tool content stays in sync (it serves the model
      reference for assistants – elements, ops, project, markup, targets,
      scopes – not the user guide; no change needed now, a link to the
      website guide in `overview` could be added once it is online) –
      `docs_get` was extended with the MCP findings (layers, groups,
      anchors, `content_scale`, devices, group `scale`); the link → Phase 7

## 6.8 Added during Phase 6 (not in the original plan)
Owner requests and MCP test-run findings, built 2026-11-10 to 2026-11-13:
- [x] fastlane compatibility: whole `fastlane/` folder import, export
      layout `--layout fastlane`, `index.html` overview, stale-file cleanup
      (`docs/research/2026-11-10-fastlane-conventions.md`)
- [x] Canvas and variant management in "Canvases & targets", rename
      strip → canvas (also the YAML key), banner from a canvas
- [x] GUI interaction: drag pans, Shift+drag marquee, scrollbars, context
      menu, element clipboard, `locked`; in-app help (keyboard & mouse
      sheet, quick guide, Help submenu with the website docs link);
      resizable inspector, image preview, page backgrounds selectable;
      "Save as template" as its own menu entry
- [x] Fonts: nine bundled OFL families (lazy), installed system fonts
      first-class, "Store used system fonts in the project", bundled Noto
      script fallbacks, CJK system preferences, RTL alignment fix
      (ADR 0028)
- [x] Apple bezels stored in a private project for CI (ADR 0027)
- [x] CI/CD: `validate --format github|gitlab|junit`, store credentials
      from the environment, CLI image + integration drafts
      (`docs/research/2026-11-12-ci-integrations.md`)
- [x] Design checks in `validate` (pixel inspector, contrast, seams,
      device distance …; ADR 0029) – round 2 (WP-C, 2026-11-13):
      `device_cut`, `screen_covered`, clearer messages, corner occluders
- [x] MCP test round 1 + 2 fixes: preview sizes, store background,
      background exports, frame metrics, image cache, group `scale`
      (ADR 0030), `apply_ops {file}`, dropped-override warning,
      `styles_update {fonts}`, own screenshot variant folders, no-op edits
      not saved
- [x] Done 2026-11-13 (agents): `clip_to` for images + span highlight
      padding/radius (F2, ADR 0031), fastlane captures in their own folder
      (import skips own exports, export reports foreign images; ADR 0032),
      gallery terms of use, legal pages completion, template language fix
      (`visible_in` remapped, never `{}`) – see Phase 7.10
