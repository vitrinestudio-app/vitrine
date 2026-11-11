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
| `hugo.toml` | site config: languages (en default at `/`, de at `/de/`), params (GitHub URL, `downloads_available` + download URLs) |
| `content/en/`, `content/de/` | pages per language (`contentDir`); same file name = translation |
| `content/*/_index.md` | the home page: **all landing-page texts live in its front matter** (hero, features, FAQ, …) |
| `content/*/docs/` | documentation (`weight` = order, `icon` = sidebar icon, `untranslated: true` shows the "translation follows" note) |
| `content/*/impressum.md`, `datenschutz.md` | legal pages – **placeholders, see TODO** |
| `i18n/en.yaml`, `de.yaml` | UI strings |
| `layouts/` | `baseof.html`, `home.html`, `page.html`, `docs/{section,page}.html`, `404.html`, `_partials/`, `_shortcodes/` (Hugo ≥ 0.146 template layout) |
| `assets/css/` | `base.css` (tokens, fonts, light/dark), `layout.css`, `components.css`, `home.css`, `docs.css` – concatenated, minified and fingerprinted by Hugo Pipes |
| `assets/js/site.js` | nav toggle, theme switch, scroll reveal, tabs, hero tilt |
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
- Privacy: no third-party requests at all (fonts and images are local), no
  analytics, no cookies.

### Regenerating the font subsets

The WOFF2 files are subsets of `/assets/fonts/` made with fontTools:

```sh
pip install fonttools brotli
U="U+0000-00FF,U+0131,U+0152-0153,U+02C6,U+02DA,U+02DC,U+2000-206F,U+20AC,U+2122,U+2190-2199,U+21E7,U+2212,U+2248,U+2260,U+2264-2265,U+2318,U+2325,U+232B,U+2713,U+2715,U+00D7,U+2026,U+2033"
for f in Inter-Regular Inter-SemiBold InterDisplay-Bold InterDisplay-Black; do
  pyftsubset ../assets/fonts/$f.ttf --unicodes="$U" --layout-features='kern,liga,calt,tnum,ss01,cv11,case' --flavor=woff2 --output-file=static/fonts/$f.woff2
done
pyftsubset "../assets/fonts/jetbrainsmono/JetBrainsMono[wght].ttf" --unicodes="$U" --flavor=woff2 --output-file=static/fonts/JetBrainsMono.woff2
```

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

## Images to replace later

New screenshots will be generated once templates exist. Today's images:

| File | Used on | Replace with |
|---|---|---|
| `assets/images/editor-dark.jpg` | home "The editor" | a current editor screenshot (this one still says "Strip" instead of "Canvas"), ideally 2× (≥ 2400 px wide), light and dark versions |
| `assets/images/inspector-text.jpg` | home, floating over the editor | a 2× crop of the inspector with a selected text (today cut from `/docs/gui/text-editing.jpg`, only 359 px wide) |
| `assets/images/iphone-gallery-en.jpg`, `iphone-gallery-fr.jpg`, `ipad-gallery-de.jpg`, `mac-gallery-en.jpg`, `web-gallery-en.jpg`, `blog-header-de.jpg` | home showcase tabs; `iphone-gallery-en.jpg` is also the Open Graph image (cropped to 1200×630) | galleries rendered from the new templates (`vitrine render --gallery`), 2× width; a dedicated 1200×630 OG image |
| CSS/SVG mock-ups: hero canvas, language × device matrix, AI chat, terminal, upload plan, template cards | home | may stay as illustrations; template cards → real template previews once the gallery exists |

### Docs screenshots (placeholders today)

The user guide marks missing screenshots with the `placeholder` shortcode
(a hatched box "Screenshot coming soon" + caption). Replace each with
`{{< shot src="images/<file>.jpg" alt="…" caption="…" >}}` in **both**
languages (German pages: German UI, `?lang=de` in the dev bridge). Ideally 2×,
light theme (or both), cropped to the dialog:

| Page (`content/*/docs/`) | Shows |
|---|---|
| `getting-started.md` | start screen with the *New project* dialog (name, location, template cards, languages) |
| `getting-started.md` | the export dialog (targets × languages grid, folder, layout, options) |
| `editor.md` | the canvas context menu on a selected phone |
| `devices-frames.md` | *Frame packages* dialog after analysing an Apple bezel package |
| `targets.md` | *Canvases & targets* → *Add target* list (presets, badges, sizes) |
| `languages.md` | Translations tab: a missing German text shown from the fallback, an overflowing French cell |
| `screenshots.md` | *Import screenshots* dialog with a fastlane folder |
| `export.md` | the export's `index.html` overview with the enlarged view |
| `store-upload.md` | App settings → Stores (two accounts, Test, the Apple key help) |
| `store-upload.md` | the upload plan table with *Confirm & upload* |
| `ai-mcp.md` | App settings → *AI assistants (MCP)* (status, Add to Claude Desktop, Claude Code command) |

Used today: `editor-dark.jpg` (getting started, outdated: says "Strip"),
`inspector-text.jpg` (editor), `ipad-gallery-de.jpg`, `blog-header-de.jpg`
(concepts, targets) and `canvas-phone-en.jpg` (concepts; rendered with
`vitrine --project tests/fixtures/example render --target ios-iphone-69
--canvas --width 2400`, converted to JPEG).

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

- **Legal pages:** `content/de/impressum.md`, `content/de/datenschutz.md` (and
  the English pointers) are placeholders – the site must not go live with them.
- **Downloads:** when installers exist, set `downloads_available = true` and
  the URLs under `[params.downloads]` in `hugo.toml`.
- **Docs:** review the user guide (en + de), replace the screenshot
  placeholders (list above); once installers exist, update the "pre-release"
  callouts (`install.md`, docs `_index.md`).
- **Gallery pages:** browsing templates from `api.vitrinestudio.app` on the
  website (phase 6.3/6.6) – not built yet.
- Domain, hosting and deployment (see above).
