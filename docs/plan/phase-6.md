# Phase 6 – Polish → release 1.0: task list

Scope: KONZEPT.md §12, Phase 6 ("Vorlagen, eigene Frame-Pakete,
Farbmanagement P3 → sRGB, Feature Graphic, Installer/Signierung, Website
vitrinestudio.app, Doku"). Done when release 1.0 ships.

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
      in the GUI (user)

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
- [ ] Owner: look at the canvas pane in the real app; decide whether the
      example's feature graphic should drop the phones (Google: "avoid
      device imagery")

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
      this workspace (`crates/gallery-server`), Docker image, SQLite, files
      on disk, behind the user's reverse proxy – reuses `core` validation and
      `render` to build trusted preview images server-side. Uploader login
      via GitHub OAuth (no passwords), moderation queue, licence per
      template fixed to **CC0** (user 2026-11-10: no licence choice; attribution/NC make no sense for store screenshots) – uploader confirms own rights + CC0 grant in the upload terms; fonts keep their own licence (OFL only). Licence file per font (user OK 2026-11-10): kept if present; otherwise generated when saving a template – copyright line (name ID 0, plus Reserved Font Name if stated) from the font + the standard OFL 1.1 text – only if the font itself declares OFL (name IDs 13/14); fonts without an OFL declaration are refused ("may not be redistributed / licence unclear"); the server re-checks on upload; size limits, safe zip handling, rate limits, abuse report,
      GDPR/Impressum; downloads without an account; in-app "browse gallery" opens the site or
      fetches an index (network → `publish`-like crate, not core)
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
- [~] Gallery backend `crates/gallery-server` (axum, SQLite, Docker):
      list/search/get/download/previews, upload with validation (server
      regenerates placeholders and previews), abuse reports, moderation
      queue, rate limits – tested in-process. TODO: GitHub OAuth (stubs,
      feature `github-oauth`), uploader self-service, deletion of rejected
      files, the website pages, in-app "browse gallery". Deployment by the
      owner: `docs/gallery/DEPLOY.md`.

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
      packs through the gallery (6.3/6.6)
- [x] User 2026-11-10: agreed as described (PNG/SVG + `frame.yaml`, project
      `frames/` or app-wide, reuse of the bezel import machinery)
- [ ] Use cases: vendor kits the user downloads (Samsung, Google Pixel,
      Figma exports), clay/flat mockups, branded frames; could later be
      shared through the same web gallery as templates
- [x] GUI: frames manager lists packs; import pack folder/zip (this
      computer / this project); inspector + project default frame pickers
      (not looked at in the real app yet)
- [x] CLI `frames import <pack> [--to] [--id]`, `frames list --packs`,
      `frames remove --pack`; MCP `frames_list` returns packs

## 6.5 Installer & signing (planned for 2026-11-11)
- [ ] Bundles via Tauri bundler: macOS `.app` + `.dmg` (universal or per
      arch), Windows `.msi`/NSIS, Linux AppImage/`.deb`(/rpm)
- [ ] Ship the CLI with the GUI (Phase 4 TODO: CLI path for bundled builds –
      `vitrine` inside the bundle, PATH install option / symlink, Claude
      Desktop config pointing at it)
- [ ] macOS: Developer ID signing + notarization (`notarytool`), hardened
      runtime, entitlements (network client, user-selected files, keychain);
      credentials only in CI secrets / the user's keychain – the user does
      the Apple account steps
- [ ] Windows: code signing (certificate type / Azure Trusted Signing –
      open question), SmartScreen
- [ ] Linux: AppImage signing optional; desktop file, icons
- [ ] Release workflow (GitHub Actions on tag): build, sign, notarize,
      checksums, GitHub release; updater (Tauri updater with signed
      manifests) – decide
- [ ] App icon set, version scheme
- [x] Licence decision: GPL-3.0-or-later + CLA (2026-11-11, ADR 0022;
      `LICENSE`, `CLA.md` 1.0 (reviewed by the owner 2026-11-11), `CONTRIBUTING.md`, manifests, Tauri bundle
      `publisher`/`copyright`/`license`, CLI help + About panel notice)
- [ ] Third-party licence notices shipped with every installer (required:
      MIT/BSD/Apache notices of the crates, `assets/fonts/OFL.txt`,
      `assets/hyphenation/LICENSES.md`): generate a `THIRD-PARTY-NOTICES`
      file with `cargo about` at release build time, bundle it as a Tauri
      resource and link it from the About dialog / `vitrine --help`; add
      `cargo deny check licenses` (+ `deny.toml` allow-list from ADR 0022)
      to CI; the release must offer the corresponding source (GitHub tag)
- [x] Owner: review of `CLA.md` (2026-11-11)
- [ ] Owner: set up the CLA bot (CONTRIBUTING.md)

## 6.6 Website vitrinestudio.app (own step)
- [ ] Static site (generator choice, hosting – open question), landing
      page, screenshots made with Vitrine Studio itself, download links per OS,
      feature overview, fastlane frameit migration page
- [ ] Hosts the template (and frame pack) gallery from 6.3/6.4

## 6.7 Documentation (own step)
- [ ] User guide from `docs/guide/NOTES.md`: getting started, editor,
      languages, import (incl. fastlane), export, store upload setup (Apple
      key, Google service account), MCP/Claude, CLI reference, project file
      format reference
- [ ] Published on the website; screenshots of the real app
- [ ] `docs_get` MCP tool content stays in sync
