# www.vitrinestudio.app

Marketing and documentation website of Vitrine Studio, built with
[Hugo](https://gohugo.io) (extended, v0.154+). No theme, no npm, no build
step besides Hugo: own layouts, plain CSS and one small script.

```sh
cd website
hugo server                 # dev server on http://localhost:1313 (drafts off)
hugo --minify               # production build into website/public/
```

`hugo --minify` must build without warnings. `public/` and `resources/` are
ignored by Git.

## Structure

| Path | Content |
|---|---|
| `hugo.toml` | site config: languages (en default at `/`, de at `/de/`), params (GitHub URL, `galleryAPI`, `downloads_available` + download URLs), no cache for remote resources |
| `content/en/`, `content/de/` | pages per language (`contentDir`); same file name = translation |
| `content/*/_index.md` | the home page: **all landing-page texts live in its front matter** (hero, features, FAQ, …) |
| `content/*/docs/` | documentation (`weight` = order, `icon` = sidebar icon, `untranslated: true` shows the "translation follows" note) |
| `content/*/docs/automation/` | a nested docs section (`_index.md` = overview with `{{< section-cards >}}`; the nav lists its pages while you are in it, the pager follows reading order). Moved pages keep their old URL with `aliases`; `aliasFragments` + `layouts/alias.html` send old `#fragments` to the sub-page that has them now |
| `content/*/impressum.md`, `datenschutz.md` | legal pages (`/de/impressum/`, `/de/datenschutz/`; English `/legal-notice/`, `/privacy/`): German binding, English translations say so at the top. The privacy policy states retention the API server enforces (rejected templates and resolved reports deleted at once, rate-limit counters dropped after their hour, expired sessions purged) – keep it in line with the code |
| `i18n/en.yaml`, `de.yaml` | UI strings |
| `layouts/` | `baseof.html`, `home.html`, `page.html`, `docs/{section,page}.html`, `404.html`, `_partials/`, `_shortcodes/` (Hugo ≥ 0.146 template layout) |
| `assets/css/` | `base.css` (tokens, fonts, light/dark), `layout.css`, `components.css`, `home.css`, `docs.css`, `gallery.css` – concatenated, minified and fingerprinted by Hugo Pipes |
| `assets/js/site.js` | nav toggle, theme switch, scroll reveal, home section index, tabs, hero tilt |
| `assets/js/gallery.js` | template gallery: live refresh, filters (FLIP), detail view, floating/tilting cards; also the live cards on the home page |
| `content/*/gallery.md`, `layouts/gallery.html`, `layouts/_partials/gallery/` | template gallery page (`/gallery/`, `/de/galerie/`): `data.html` (build-time snapshot), `labels.html` (tag labels), `card.html` (one card, mirrored by the script) |
| `assets/images/` | screenshots (resized to WebP/JPEG `srcset` by `_partials/picture.html`) |
| `assets/brand/` | logo files copied from `/assets/brand/` (© Looking4Cache UG, not GPL) |
| `static/fonts/` | self-hosted WOFF2 subsets (Latin) of Inter, Inter Display and JetBrains Mono (SIL OFL, licence files next to them) |

### Design notes

- Brand gradient from the logo: violet `#6a5bff` → magenta `#c04bf2` →
  coral `#ff7a59`. Light and dark mode via `prefers-color-scheme`, plus a
  toggle that stores the choice in `localStorage` (no cookie).
- Motion: load-in of the hero, scroll reveal (`.reveal`, IntersectionObserver),
  the animated "continuous canvas" SVG in the hero (`_partials/hero-canvas.html`:
  six pages with phones, a shared laptop and two tablets on one background,
  drawn by `_partials/hero-canvas-scene.html`; numbered callouts appear one
  after another via CSS delays, their texts come from `hero.notes` and move
  into a list below the canvas on small screens), card hover depth. Everything is off with `prefers-reduced-motion: reduce`;
  without JS all content is visible.
- Header: one "Home" / "Start" link instead of in-page links (they were
  mistaken for the way back to the top). The home page has a **section
  index** at the right edge (`_partials/section-index.html`, labels
  `sec*` in `i18n/`, styles at the end of `home.css`, script in its own
  scope in `site.js`): ticks on a rail filled up to the current section.
  The current section is the last one whose content top (section top +
  padding) is above a line 30 % down the viewport below the header; at
  the bottom of the page the last visible section wins. Its label is
  prominent, the others recede with distance (`data-d` 0–3: paler,
  smaller, slightly blurred). From 1420 px the labels sit in the margin
  all the time (after the hero); below, the index shows while scrolling
  over a frosted strip and fades out 1.4 s after scrolling stops. Visible
  labels are links right where they are; hover or keyboard focus shows
  all labels on a card, on touch a tap on the rail opens it. No index
  below 700 px (phones). Hidden without JS. A new home section needs an `id` and
  an entry there. The old anchors (`#features`, `#ai`, `#automation`,
  `#fastlane`) stay; the footer uses them.
- Privacy: no third-party requests at all (fonts and images are local), no
  analytics, no cookies.

### Screenshots

- Images live in `assets/images/` and are embedded with
  `_partials/picture.html` (home page) or the `shot` shortcode (docs);
  Hugo makes the WebP/JPEG `srcset`. Keep sources at most 2400 px wide
  (full editor shots, galleries) or 1520 px (dialogs; the docs column is
  760 px), JPEG quality ~84.
- **Light/dark pairs:** an app screenshot `x.jpg` may have a sibling
  `x-dark.jpg` (same view, dark app theme). `picture.html` then renders both
  `<picture>` elements (`theme-img--light` / `theme-img--dark`, both
  `loading="lazy"`), and `base.css` hides the one that does not match the
  site theme – via `prefers-color-scheme` and the toggle's `data-theme` on
  `<html>`, so it works without JS and switches without a flash. The hidden
  image is `display: none` and lazy, so the browser does not download it.
  Pass `"theme" false` to opt out. Store-gallery renders have one version.
- App screenshots are taken in the dev bridge (`?lang=en`, `?theme=light|dark`)
  at a 1600×1000 CSS px window with device scale 2, from projects made from
  the gallery templates *Pop-Art Panels*, *Synthwave Sunset* and *Park
  Poster*; galleries and banners are rendered with `vitrine render
  --gallery` / `--image`.
- **Matrix on the home page** ("Every language. Every device. Every
  store."): `assets/images/matrix/<target>-<lang>.jpg`, image 1 of each
  target rendered with `--image 1 --max-edge 400` from one project made
  from the *Warm Stay* template (`vitrine new … --lang en --lang de --lang
  fr`, French texts added by hand) plus two banner targets added to it:
  `web-banner` (1600×1000, `canvas: tablet-land`, region `{page: 2, x: 596,
  y: 220, width: 848, height: 530}` – the MacBook) and `play-feature-graphic`
  (1024×500, region `{page: 0, x: 0, y: 0, width: 1000, height: 488}`).
  Columns, targets and the image count (`count`, a real `vitrine export`
  of that project) live in the `matrix:` front matter of `_index.md`.
  `ipad-grouped-gallery-en.jpg` (targets page) is the gallery of a target
  `{canvas: tablet, group: 2, size: [2752, 2064]}` in the same project.
- Narrow panel screenshots float beside the text: `{{</* shot … float="right"
  width="360" */>}}` (CSS px, capped at half the image width; placed right
  after the heading, cleared by the next heading, not floated below 640 px).
- The docs still support `{{</* placeholder "…" */>}}` for screenshots that
  cannot be taken yet.

### Regenerating the font subsets

The WOFF2 files are subsets of `/assets/fonts/` made with fontTools:

```sh
pip install fonttools brotli
U="U+0000-00FF,U+0131,U+0152-0153,U+02C6,U+02DA,U+02DC,U+2000-206F,U+20AC,U+2122,U+2190-2199,U+21E7,U+2212,U+2248,U+2260,U+2264-2265,U+2318,U+2325,U+232B,U+2713,U+2715,U+00D7,U+2026,U+2033"
for f in Inter-Regular Inter-SemiBold InterDisplay-Bold InterDisplay-Black; do
  pyftsubset ../assets/fonts/inter/$f.ttf --unicodes="$U" --layout-features='kern,liga,calt,tnum,ss01,cv11,case' --flavor=woff2 --output-file=static/fonts/$f.woff2
done
pyftsubset "../assets/fonts/jetbrainsmono/JetBrainsMono[wght].ttf" --unicodes="$U" --flavor=woff2 --output-file=static/fonts/JetBrainsMono.woff2
```

## Template gallery (`/gallery/`, `/de/galerie/`)

Data source: the API server (`crates/api-server`, `docs/api/API.md`) at
`params.galleryAPI` (default `https://api.vitrinestudio.app`).

- **Build time:** `layouts/_partials/gallery/data.html` reads every
  approved template (`GET /v1/templates?limit=200&offset=…`, following the
  `limit` the server answers) and the tag taxonomy (`GET /v1/tags`) with
  `resources.GetRemote` (not cached: `[caches.getresource] maxAge = 0`),
  renders static cards (readable without JavaScript, indexable) and embeds
  the snapshot as JSON. **The build never fails because of the API:**
  unreachable → warning `gallery-api` and the "gallery opens soon" state;
  empty gallery → the same state without warning. Rebuild/deploy the site
  to refresh the snapshot (e.g. after approving templates).
- **In the browser:** `assets/js/gallery.js` refreshes the list from the
  API (all pages) and replaces the snapshot when it changed (new
  templates fly in). This needs CORS on the server:
  `VITRINE_API_CORS_ORIGINS=https://www.vitrinestudio.app,https://vitrinestudio.app`
  (`docs/api/DEPLOY.md`). Without it the page silently keeps the snapshot.
- Sprites (one WebP per canvas, the `?v=` URLs from the API, cached for
  good) load lazily from the API; there are no other third-party requests.
  No cookies; the filters live in the URL query, the detail view in the
  hash (`/gallery/#<id>`).
- Home page: the templates section shows the 3 most downloaded templates
  of the snapshot as live cards (and the illustrations while it is empty).

**Development against a local API** (the live API may have no templates):

```sh
# 1. API server with a scratch data folder, a static upload token and auto-approve
VITRINE_API_DATA_DIR=/tmp/vapi VITRINE_API_BIND=127.0.0.1:8787 \
VITRINE_API_PUBLIC_URL=http://127.0.0.1:8787 VITRINE_API_AUTO_APPROVE=1 \
VITRINE_API_UPLOAD_TOKENS=dev-token:vitrinestudio-app \
VITRINE_API_CORS_ORIGINS=http://localhost:1313 \
VITRINE_API_READ_RATE_LIMIT_PER_HOUR=100000 \
  cargo run --release -p vitrine-api-server

# 2. templates: save copies of projects with tags (+ own screenshots as samples)
vitrine --project <copy> template save --name "…" --style bold --color dark \
  --use games --sample-screenshots -o /tmp/t/<id>.vitrine-template
DEV=dev-token vitrine gallery upload /tmp/t/*.vitrine-template \
  --api http://127.0.0.1:8787 --token-env DEV --confirm-own-screenshots

# 3. the site against it (env var: Hugo lowercases the param `galleryAPI`)
HUGO_PARAMS_GALLERYAPI=http://127.0.0.1:8787 hugo server
```

Packages are limited to 50 MB (server and app): templates with many
full-size sample screenshots may be too large – check with `vitrine gallery
upload --dry-run`.

Build checks while `hugo server` runs: build into a temporary folder and
point the link checker at it: `hugo --minify -d /tmp/site && python3
scripts/check-links.py /tmp/site`.

## Deployment (not set up yet)

The owner's other Hugo sites are deployed as static files: `hugo --minify`
(or `hugo --gc`), then upload `public/` via SFTP to the web server
(`.vscode/sftp.json` with `"context": "public"` and
`"remotePath": "/var/www/www_<domain>"`), nginx serves the folder; redirects
live in a `deploy/nginx-*.conf` snippet. The same pattern fits here:

1. Point `www.vitrinestudio.app` (and a redirect from `vitrinestudio.app`)
   at the web server, TLS certificate.
2. `cd website && hugo --minify`, upload `public/` to e.g.
   `/var/www/www_vitrinestudio_app`.
3. nginx: serve `404.html` as the error page; long cache headers for
   `/css/`, `/js/` (fingerprinted), `/fonts/` and processed images.

## Current images

All in `assets/images/` (made 2026-11; retake them when the UI changes
visibly). No placeholders are left in the docs.

| Images | Used on | Made from |
|---|---|---|
| App screenshots, each a light/dark pair (`x.jpg` + `x-dark.jpg`): `editor`, `inspector-text`, `text-editing`, `gallery-view`, `all-languages`, `context-menu`, `app-menu`, `translations`, `validation`, `new-project`, `new-project-local`, `export-dialog`, `export-overview`, `canvas-dialog`, `targets-add`, `import-screenshots`, `frame-import`, `save-template`, `project-fonts`, `project-stores`, `store-accounts`, `upload-plan`, `mcp-settings` | home (editor, floating inspector) and the docs | dev bridge, see *Screenshots* above; projects made from the gallery templates *Pop-Art Panels*, *Synthwave Sunset* and *Park Poster* (the sample apps Swoonly, Glowtape and Ridgecast) |
| `iphone-gallery-en`, `iphone-gallery-fr`, `ipad-gallery-de`, `play-gallery-en`, `ipad-landscape-gallery-en`, `blog-header-en` | home showcase tabs (`showcase:` in `_index.md`); `play-gallery-en` and `blog-header-en` also in concepts / targets; `iphone-gallery-en` is the Open Graph image (cropped to 1200×630 in `head.html`) | `vitrine render --gallery` / `--image` of the same three template projects; one version (store renders, no dark pair) |
| `canvas-phone-en` | concepts | the *Synthwave Sunset* phone canvas, `vitrine render --canvas --width 2400` |
| `matrix/<target>-<lang>.jpg`, `ipad-grouped-gallery-en` | home matrix, targets | the *Warm Stay* project, see *Matrix on the home page* above |
| CSS/SVG illustrations: hero canvas, AI chat, terminal, upload plan, template cards | home | hand-made; the template cards give way to live gallery cards once the snapshot has templates |

Still open: a dedicated 1200×630 Open Graph image.

The repo README uses separate, smaller copies (not used by the site):
`docs/gui/` (`editor`, `gallery-view`, `text-editing`, `new-project`, each
with `-dark`) and `docs/example/` (`iphone-gallery-pop-art` and
`play-gallery-synthwave` from `iphone-gallery-en`/`play-gallery-en`,
`warm-stay-matrix` composed from the `matrix/` images); 1600 px wide
(dialogs 1200 px), JPEG quality ~80. Refresh them when the originals change.

## Docs

- `content/en/docs/` is the user guide (17 pages + overview); `content/de/docs/`
  the same pages in German (same file names, headings carry the English
  anchor ids `{#…}` so `#fragment` links work in both languages). Change
  both when a feature changes; the developer-side notes stay in
  `docs/guide/NOTES.md`.
- Shortcodes: `shot` (image from `assets/images` with caption), `placeholder`
  (missing screenshot), `callout` (`note`, `tip`, `status` = pre-release,
  `warn`), `cli-help "vitrine <cmd>"` (the `--help` text from
  `data/cli_help.json`).
- CLI reference data: `scripts/docs/cli-help.sh [path/to/vitrine]` regenerates
  `data/cli_help.json` from the real `--help` output – run it after changing
  a command or flag.
- Link check: `hugo --minify && python3 scripts/check-links.py` (internal links
  and `#fragments` of `public/`; exit 1 when something is broken).

## TODO (owner)

- **Legal pages:** complete (owner data 2026-11-13: info@grove.eco,
  Hetzner in Germany with AVV for website and API, logs with anonymised IPs
  for 30 days, full IPs only for security-relevant entries). Configure the
  web server and the API's nginx accordingly before go-live, and keep the
  privacy policy in line with the code when data flows change.
- **Downloads:** when installers exist, set `downloads_available = true` and
  the URLs under `[params.downloads]` in `hugo.toml`.
- **Docs:** review the user guide (en + de); once installers exist, update the "pre-release"
  callouts (`install.md`, docs `_index.md`).
- **Gallery:** set `VITRINE_API_CORS_ORIGINS` on the API server (above);
  rebuild the site after approving templates (snapshot for no-JS/SEO and
  the home page cards).
- Domain, hosting and deployment (see above).
