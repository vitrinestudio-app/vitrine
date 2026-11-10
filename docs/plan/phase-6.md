# Phase 6 – Polish → release 1.0: task list

Scope: KONZEPT.md §12, Phase 6 ("Vorlagen, eigene Frame-Pakete,
Farbmanagement P3 → sRGB, Feature Graphic, Installer/Signierung, Website
arrangia.app, Doku"). Done when release 1.0 ships.

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
search-results creatives (banners from a strip region).
- [ ] Check end to end: example fixture target, export, store rules (24-bit
      PNG/JPEG, no alpha), upload plan + Play upload as single image
- [ ] Template / quick action "banner from this strip" shared with the Apple
      creatives (region + fit), GUI wording

## 6.3 Templates (+ web gallery)
KONZEPT §9 "Vorlagen": duplicate a canvas, save as template, new project
from template. User 2026-11-10: templates should be uploadable to the web
and downloadable from a gallery.
- [ ] Template format: a project folder subset (strip(s), styles, fonts,
      placeholder screenshots) + `template.yaml` (name, author, licence,
      preview images, required fonts, tags); packed as `.arrangia-template`
      (zip); no vendor graphics, fonts only with redistributable licences
- [ ] Save canvas / project as template (GUI + CLI + MCP), new project from
      template (built-in + local + downloaded), insert canvas from template
- [ ] App-wide template store (`<app data>/templates/`), list/remove
- [ ] Web gallery on arrangia.app (see 6.6): browse, preview, download;
      upload flow. User 2026-11-10: **own backend** (e.g. `api.arrangia.app`
      on the user's Docker server; user also runs PHP and Node.js
      elsewhere). Stack (user OK 2026-11-10): Rust (axum) in
      this workspace (`crates/gallery-server`), Docker image, SQLite, files
      on disk, behind the user's reverse proxy – reuses `core` validation and
      `render` to build trusted preview images server-side. Uploader login
      via GitHub OAuth (no passwords), moderation queue, licence per
      template fixed to **CC0** (user 2026-11-10: no licence choice; attribution/NC make no sense for store screenshots) – uploader confirms own rights + CC0 grant in the upload terms; fonts keep their own licence (OFL only). Licence file per font (user OK 2026-11-10): kept if present; otherwise generated when saving a template – copyright line (name ID 0, plus Reserved Font Name if stated) from the font + the standard OFL 1.1 text – only if the font itself declares OFL (name IDs 13/14); fonts without an OFL declaration are refused ("may not be redistributed / licence unclear"); the server re-checks on upload; size limits, safe zip handling, rate limits, abuse report,
      GDPR/Impressum; downloads without an account; in-app "browse gallery" opens the site or
      fetches an index (network → `publish`-like crate, not core)
- [ ] Licence/attribution per template; checks on import (size limits, no
      executable content, path traversal)

## 6.4 Own frame packages
KONZEPT §4.2 (frames, point 3): any PNG or SVG frame plus a small
`frame.yaml` (screen rectangle, mask / corner radius, model mapping) in the
project's `frames/` or app-wide. Arrangia ships no vendor images.
- [ ] `frame.yaml` format (reuse the imported-bezel store and screen
      detection from `core::frame_import`, ADR 0012): images per
      orientation/colour, screen rect or auto-detect, mask/corner radius,
      maps to catalog model(s) or defines a new device class/size
- [ ] Sources: project `frames/` (travels with the project, Git) and
      app-wide; `frame: pack:<name>` (or `imported:<vendor>`) resolution
- [ ] SVG frames via `resvg`
- [x] User 2026-11-10: agreed as described (PNG/SVG + `frame.yaml`, project
      `frames/` or app-wide, reuse of the bezel import machinery)
- [ ] Use cases: vendor kits the user downloads (Samsung, Google Pixel,
      Figma exports), clay/flat mockups, branded frames; could later be
      shared through the same web gallery as templates
- [ ] GUI: frames manager lists packs; import pack folder/zip; inspector

## 6.5 Installer & signing (planned for 2026-11-11)
- [ ] Bundles via Tauri bundler: macOS `.app` + `.dmg` (universal or per
      arch), Windows `.msi`/NSIS, Linux AppImage/`.deb`(/rpm)
- [ ] Ship the CLI with the GUI (Phase 4 TODO: CLI path for bundled builds –
      `arrangia` inside the bundle, PATH install option / symlink, Claude
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
- [ ] App icon set, version scheme, licence decision (still open – no
      LICENSE file yet), third-party licence notices (fonts, hyphenation,
      crates: `cargo about`/`cargo deny`)

## 6.6 Website arrangia.app (own step)
- [ ] Static site (generator choice, hosting – open question), landing
      page, screenshots made with Arrangia itself, download links per OS,
      feature overview, fastlane frameit migration page
- [ ] Hosts the template (and frame pack) gallery from 6.3/6.4

## 6.7 Documentation (own step)
- [ ] User guide from `docs/guide/NOTES.md`: getting started, editor,
      languages, import (incl. fastlane), export, store upload setup (Apple
      key, Google service account), MCP/Claude, CLI reference, project file
      format reference
- [ ] Published on the website; screenshots of the real app
- [ ] `docs_get` MCP tool content stays in sync
