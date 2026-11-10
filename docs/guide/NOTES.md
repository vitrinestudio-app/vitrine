# User guide – collected notes

Raw notes for the future user guide / website (arrangia.app). **Not prose yet.**
Language: English (German translation later). Planned format: mdBook under
`docs/guide/` (Phase 6). Every feature adds its user-facing facts here as bullet
points under the matching chapter. Keep entries short, concrete, with commands.

## Outline (planned chapters)
1. Getting started (install, first project, tour of the editor)
2. Concepts (canvas/strip, home page, gaps, anchors, page grouping, variants, targets, languages & fallbacks)
3. How-to guides (screenshots import, translating, Apple frames, exporting, store sizes, banners)
4. Reference (CLI, project.yaml fields, validation findings, keyboard shortcuts, markup)
5. Limits & FAQ

---

## 1. Getting started
- CLI quick start: `arrangia new <dir> [--template phone|minimal] [--lang en --lang de]` → `arrangia validate` → `arrangia export`.
- GUI: `arrangia-gui [project-dir]` opens a project directly; otherwise start screen (new / open / recent).
- Project = folder: `project.yaml`, `i18n/<lang>.yaml`, `screenshots/<lang>/<variant>/<key>.png`, `assets/`, `fonts/`, `export/` and `.cache/` (both gitignored).
- Every edit is saved immediately (no "Save"); external edits (e.g. by an AI assistant via MCP, a text editor, git checkout) are reloaded live.

## 2. Concepts
- Page width is always 1000 lu (logical units); height follows the aspect. Sizes in lu → export resolution independent.
- Home page: each element's position is relative to its page; elements may reach into neighbouring pages (a phone spanning two screenshots exists once).
- Gaps: stores show screenshots with gaps; Arrangia builds the canvas with those gaps so continuous elements line up in the store; content inside a gap is not exported. Gap values per gallery profile; app-store web values measured from apps.apple.com CSS, native store apps still placeholders.
- Anchors (`top|bottom|center|stretch`) adapt one layout to other aspect ratios (e.g. iPhone 1:2.17 vs Play 1:2).
- Grouping `group: 2`: two portrait pages → one landscape image (iPad landscape from the phone canvas).
- Variants: per-format overrides (position, size, rotation…) while content stays shared. Inspector marks overrides purple.
- Language overrides: per-language text tweaks (font size, box, style, fit, effects); marked teal in the inspector; `de-AT` inherits `de`.
- Regional languages (`fr-CA`, `de-AT`, `pt-BR`): fall back automatically to their prefix language (`fr-CA → fr → default`); translate only the keys that differ (example: fr-CA has 2 Québec texts, "jardin communautaire", "cellulaire"). No `languages.fallback` entry needed for prefixes; use it only for non-prefix chains.
- Fallback severities: text from the default language = warning "not translated" (fails `validate --strict`); text from a prefix/configured fallback = info; screenshot from any fallback = info. A partial screenshot set per language is fine (missing ones come from the fallback chain).
- Rotation sign: positive = clockwise (CSS convention).

## 3. How-to notes

### Store sizes (verified in App Store Connect web app, 2026-11-10)
- Required iPhone set: "iPhone with Dynamic Island (medium display)" = 6.3" = 1206×2622 → `APP_IPHONE_61`. All other iPhone sizes optional (scaled by Apple).
- 6.9" 1320×2868 / 1290×2796 → `APP_IPHONE_67` (no `_69` type exists).
- 1260×2736 and 1512×2316 are listed on Apple's help page but rejected by the ASC uploader → Arrangia does not offer them.
- iPad 13" 2064×2752 / 2048×2732 → `APP_IPAD_PRO_3GEN_129` (required if the app runs on iPad).
- Mac: 16:10 only (1280×800, 1440×900, 2560×1600, 2880×1800).
- Google Play: long side ≤ 2× short side, 320–3840 px, 2–8 per device type, ≥ 4 at ≥ 1080 px for recommendations.
- iPhone Duo exists in ASC behind a feature flag; sizes not public yet.

### Screenshots import
- Supports Arrangia folder layout, fastlane snapshot output (`de-DE/iPhone 17 Pro Max-01_home.png`), flat folders.
- Locale `de-DE` → `de` automatically; device name → variant by mapping or pixel size; `01_home` → key `home`.
- `_framed.png` (frameit) and `screenshots.html` are skipped.
- Mapping can be saved to `project.yaml` (`import.fastlane`) so re-imports run unattended.
- CLI: `arrangia import screenshots <dir> [--dry-run] [--device-map "NAME=VARIANT"] [--save-mapping]`.

### Translating
- Markup: `**bold**`, `_italic_`, `[style]…[/]`, `\n`, soft hyphen `\-`.
- Soft hyphen: invisible unless the line breaks there; then a visible hyphen.
- Emergency break (word split without hyphen) is reported as a warning.
- Automatic hyphenation: opt-in (`hyphenate: true`), only kicks in when text doesn't fit even at `shrink_to`. Bundled languages: en-us, en-gb, de (1996), fr, es, nl, it, da, pt, nb/no, fi. Swedish missing (pattern licence LPPL-only, pending project licence decision).
- Exchange: CSV (Excel variants with BOM/`;` detected) and XLIFF 1.2/2.0; markup becomes protected inline tags so CAT tools don't break it; import shows a dry-run diff and post-import overflow check; one undo step.
- CSV context column gives element/page/style and an approximate max length.
- XLIFF/CSV export writes one file per non-default language; a sparse regional language (fr-CA) gets all keys it lacks with `--missing-only`.
- French typography: put a no-break space (U+00A0) before `:` `;` `?` `!` and inside « guillemets »; also to keep words together on one line ("le bon\u00a0moment"). Line breaking respects it.
- YAML: a translation containing `: ` (colon + space) or starting with `"`, `[`, `*` must be quoted (`"…"`); the GUI/strings import write correct YAML automatically.
- Hyphenation tip: auto-fit shrinks first and hyphenates only at `shrink_to`. If a long compound makes a headline shrink too much in one language, raise `shrink_to` for that language with a language override (`langs: {de: {fit: {shrink_to: 78, max_lines: 2}}}`) so hyphenation kicks in earlier; widen the box (`box`) if the break lands at an ugly syllable (it takes the last pattern point that fits: "Mischkul-turpartner" vs "Mischkultur-partner"). Soft hyphen `\-` forces the exact point. Example: `title-companions` in the example project.
- Validation info `hyphenated` lists every text that was hyphenated automatically per target × language – check those renders.

### Apple device frames (Apple Design Resources "Product Bezels")
- User downloads the bezels themselves from developer.apple.com/design/resources; Arrangia never ships or uploads them (Apple licence; License.rtf in the package).
- The DMGs contain a licence agreement: **open the DMG in Finder, accept the licence, then import the mounted volume** (`/Volumes/Bezel-iPhone-18`). Direct DMG import fails with "attach canceled".
- Package contents: PNGs per model/colour/orientation (`iPhone 18 Pro Max - Black - Portrait.png`), Photoshop files (ignored), License.rtf.
- iPhone 18 package (verified 2026-11-10): models Pro and Pro Max; colours Black, Burgundy, Glacier, Silver; screen exactly 1320×2868 (Pro Max) / 1206×2622 (Pro).
- Frames are stored app-wide (app data dir), never in the project; projects only say `frame: imported:apple`. Missing on another machine → parametric frame + info.
- CLI: `arrangia frames import <path> [--dry-run -v]`, `frames list`, `frames remove`.
- Both Pro and Pro Max bezels are kept; the bezel is picked per target size (else screenshot size), or pinned per device ("Bezel" in the inspector: "Auto · 18 Pro Max").
- Use them for the whole project: Project settings → General → Frame style → "Apple (imported)" (`styles.frame: imported:apple`); a hint shows when no Apple frames are installed. Per device: inspector → Frame.
- Colours come from the package (Black, Burgundy, Glacier, Silver for iPhone 18), not from Arrangia's palette. With an Apple frame the inspector shows only the installed colours, as swatches in the real housing colour (measured from each bezel at import: outer metal rim, black glass border excluded; stored as `swatch` in the store's `index.yaml`, filled in for older imports the first time the frame list is shown).
- Switching the frame source (device or project default) changes the colour too, in the same undo step: the closest colour of the new source by housing colour (Black titanium → Black, Natural titanium → Silver; Burgundy → Black titanium back). Round trips can change the colour (White titanium → Silver → Natural titanium).
- A colour that is not installed (e.g. project from another machine) → the default/first installed colour is drawn, info `imported_color_fallback`. No Apple frame installed at all → built-in detailed frame with the default colour, info `imported_frame_missing`.

### Text effects
- Outline (stroke), glow, shadow on text styles, span styles and elements. Precedence span > element > style. Disable inherited: `width: 0`, `opacity: 0`, `shadow: none`. Sizes in lu (don't shrink with auto-fit).

### Images
- PNG/JPEG/SVG. Shadows of transparent images follow the alpha (like CSS drop-shadow). Text inside SVGs is not rendered yet.

### Export
- Files: `export/<target>/<lang>/<nn>_<page-id>.png`, grouped `<nn>_<id1>+<id2>`, `banner.png`, `_gallery.png`.
- Always opaque (no alpha), sRGB. Incremental: unchanged images are skipped (`--force` to re-render).

### AI assistants / MCP (Phase 4)
- Arrangia is a plain MCP server: any MCP-capable client works. App settings → "AI assistants (MCP)": generic part first (switch, status, fixed port, start on demand, "Other MCP clients" with the stdio command and the HTTP URL + bearer header), then the client-specific setup ("Claude Desktop", "Claude Code").
- Generic config (any client): stdio server `arrangia` with command `<abs path>/arrangia`, args `["mcp"]` (recommended, always reaches the running app); or Streamable HTTP `http://127.0.0.1:<port>/mcp` with header `Authorization: Bearer <token>` (both from the settings, valid until the app restarts).
- Claude Desktop: App settings → "AI assistants (MCP)" → "Add to Claude Desktop…" (shows file + entry, asks, backup `claude_desktop_config.json.bak-<UTC timestamp>`) → restart Claude Desktop. Entry: `mcpServers.arrangia = {command: "<abs path>/arrangia", args: ["mcp"]}`. Config file: macOS `~/Library/Application Support/Claude/claude_desktop_config.json`, Windows `%APPDATA%\Claude\claude_desktop_config.json`, Linux `~/.config/Claude/claude_desktop_config.json`. Already present → "Already set up"; CLI moved → updated in place; broken JSON → nothing written.
- The app hosts the MCP endpoint (setting "Allow AI assistants to connect via MCP", default on, switches live): Streamable HTTP on `127.0.0.1:<random port>/mcp` (optional fixed port setting), bearer token (new per start), loopback Host/Origin only. Discovery file `<app data>/mcp.json` (port, token, pid; 0600; removed on exit).
- `arrangia mcp` (stdio, what Claude Desktop and other stdio clients start): running app found → forwards to it (the assistant edits what you see); none → starts the app (setting "Start on demand", default on; waits ≤ 20 s); else / `--headless` → own session on `--project` (default `.`, if it holds a `project.yaml`), the assistant uses `project_open` / `project_create`.
- Claude Code: `claude mcp add arrangia -- <path>/arrangia mcp` (recommended) or the HTTP command with port + token from the settings (valid until the app restarts). Headless/CI: `arrangia mcp --headless --project <dir>`.
- Live aids: top-bar sparkle indicator (grey = off, "MCP" = ready, orange with the client's name = connected; tooltip), click → "Assistant activity" (time, label, click selects, undo for the latest step); an assistant's changes are outlined on the canvas for 1.5 s and shown as small toasts "<client name>: …" (click selects). Projects an assistant opens/creates open in the app (start screen → editor).
- In the app, tools default to what you look at: omitted `target` = the current target (if it shows the strip), omitted `lang` = the current language. `view_get` / `selection_get` read the view and selection ("make this bigger").
- Client display names (from `clientInfo`): `claude-ai` = Claude Desktop, `claude-code` = Claude Code, others their `title`, else the prettified `name` (`my-client` → "My Client"), else "Assistant"; a client counts as connected while it has a session or was active in the last 5 min.
- Every change by an assistant is one undo step labelled `<client name>: …` (e.g. `Claude Desktop: Move title`) in the shared history (undo in the app or with `history_undo` / `history_redo`). Event actor: `{kind: "assistant", client, name}` (was `kind: "claude"` before 2026-11-10; still accepted).
- The assistant sees the result: `render_preview` returns a PNG (view `image` = one exported image, `canvas` = whole strip with hatched gaps, `gallery` = store gallery; `max_edge` default 1200) plus the errors/warnings of that target × language.
- External edits (text editor, git checkout, the GUI) are picked up before every tool call; a `project.yaml` that does not load gives error `load` and nothing is written.
- No network; files are written only inside the project folder and to paths given explicitly (`export.out_dir`, `strings_export.path`). Installed device frames are read-only over MCP (`frames_list`).
- Coordinates over MCP: lu, page 1000 lu wide; `element_move` x/y = element centre relative to the top-left of a page (default: home page), or `dx/dy`. Rotation degrees, positive = clockwise.
- Edit scope: `base` (default, all targets/languages), `variant` (format override, e.g. iPad landscape), `lang` (per-language text override).
- Resetting overrides: in scope `variant`/`lang` a field set to `null` is **removed from the override** (back to the base value; overrides never store `null`). `element_reset_override {id|ids, scope: variant|lang, variant?, lang?, fields?}` removes fields or the whole override. A hand-written `null` inside an override in `project.yaml` is ignored, reported as info `override_null` and dropped on the next save. In scope `base`, `null` still resets the field to its default.
- One undo step for several changes: `element_update {updates: [{id|ids, patch, scope?, lang?, variant?, strings?}, …]}` (e.g. font size for de and fr at once); ids may come from several strips (`element_update`, `element_remove`, `element_reorder`, `element_rotate`, `element_move` with dx/dy) – each strip is edited on the view's target if it shows that strip, else on the first target showing it. `apply_ops` `update_element` writes the base only; overrides via `set_lang_override`, `reset_lang_override_field`, `set_variant_override`, `reset_override_field`.
- Field reference without resources: `docs_get {topic}` (overview, elements, ops, project, markup, targets, scopes). Claude Desktop does not let the model read MCP resources by itself (the user attaches them), so the docs are a tool too.
- `assets_import`: kind `screenshot` | `asset` (any image for image elements and image fills → `assets/`, alias `image`) | `font`. Many single files with their own key/name in one call: `files: [{path, kind?, key?, name?, lang?, variant?}]` with per-file results and `dry_run`. Whole folders (fastlane, language folders): `screenshots_import`.
- Tools: project_open, project_create, project_summary, view_get, selection_get · catalog_devices, catalog_targets, catalog_galleries, catalog_fonts, store_config_get · strip_add, strip_update, strip_variant, target_add, target_update, target_remove, language_add, language_remove · element_list, element_get, element_add, element_update, element_remove, element_move, element_resize, element_rotate, element_reorder, element_group, element_ungroup, element_rename, element_reset_override, apply_ops · strings_get, strings_set, strings_missing, string_key_rename, strings_export, strings_import · styles_get, styles_update · assets_list, assets_import, screenshots_import, frames_list · render_preview, layout_report, validate, export · history_undo, history_redo, history_state · docs_get · upload_plan, upload_execute, upload_status, upload_log.
- Resources: `arrangia://project/project.yaml`, `arrangia://project/i18n/<lang>.yaml`, `arrangia://catalog/devices`, `arrangia://catalog/targets`, `arrangia://docs/<topic>` (same topics as `docs_get`; `ops` = op reference for `apply_ops`). Resources work through the stdio proxy too, but Claude Desktop shows them only for the user to attach.
- `export` sends MCP progress notifications (one per file) when the client passes a progress token.
- Example prompts: "Create an Arrangia project in ~/Screens/Fernwood (en, de) for a garden planner, three iPhone pages with headline + phone each, show me the gallery"; "Translate everything missing into French, then check page 2 in French for overflows"; "Select the title in the app and say: make this 20 % smaller"; "Export iPhone and iPad for all languages".

### Store accounts & store mapping (Phase 5.1, ADR 0016)
- Accounts are app-wide (App settings → Stores), not in the project: a project folder can be shared via Git, keys stay on the machine.
- Where keys are stored: the system keychain (macOS Keychain, Windows Credential Manager, Linux Secret Service), service `app.arrangia`, account `<account id>/apple` or `<account id>/google`. `settings.yaml` only lists id, label, Apple issuer id + key id, Google client email + project id.
- Import Apple: App Store Connect → Users and Access → Integrations → App Store Connect API → generate a team key, download `AuthKey_<KEYID>.p8` (only once possible). In Arrangia: "Add App Store Connect key…", pick the file, enter the issuer id (shown above the key list); the key id comes from the file name. Role needed for screenshots: App Manager (Admin works too) – to confirm in the real upload test (5.6).
- Import Google: Google Cloud console → IAM → Service accounts → create one, Keys → Add key → JSON. Play Console → Users and permissions → invite the service account e-mail, app permission to edit the store listing ("Store presence"); the Google Play Android Developer API must be enabled in the Cloud project. In Arrangia: "Add Google service account…", pick the JSON. Exact permission names to confirm in 5.6.
- Import checks the file (Apple: PEM EC P-256 private key; Google: `type: service_account`, `client_email`, `private_key`) and never shows key content in messages.
- Remove account = deletes the keychain entry too (with confirmation). Projects keep the reference and show "account not found".
- Test (app settings → Stores → Test): read-only. Apple: signs a token and lists apps → "key accepted, N apps visible (first: …)"; 0 apps = check the key's role/app access. Google: only gets an access token from Google → shows the service account e-mail; Play Console permissions are only checked at upload. Errors: "key rejected" (wrong issuer/key id, revoked or deleted key, wrong system clock), "no access" (Apple role too low), "cannot reach … – offline?", server problem / rate limit / timeout → try again later. CLI/MCP do not test connections.
- Project settings → Stores: App Store (account, Apple ID = numeric app id, bundle id, platform iOS/macOS), Google Play (account, package name), store languages per project language.
- App picker (App Store): with an account chosen, "Choose app…" lists every app the key can see (name – bundle id; SKU and primary language shown for the chosen one) and fills Apple ID + bundle id; ⟳ reloads (the list is kept for the app session). Typing the ids by hand still works. The platform is not suggested (App Store Connect's app list has no platform).
- Google Play cannot list the apps of a service account (no such API call) → type the package name (shown under the app name in Play Console). "Check" asks Play whether the package exists and the service account may edit it: it opens a temporary draft edit, reads the app details (default language + listing title) and deletes the edit again – never committed, nothing changes. Only runs when you click it. Errors: "Google Play does not know this package, or the service account has no access" (404), "no permission for this app" (403, invite the service account in Play Console).
- Help in the app: App settings → Stores (and the import dialog) → "How do I get an App Store Connect key?" / "How do I get a Google Play service account?" – numbered steps (summary of "Store upload – setup" below) and buttons that open App Store Connect API keys, Apple's guide, the Cloud console (enable the API, service accounts), Play Console and Google's guide in the browser (only these fixed links can be opened). Stated in the UI but not yet verified with a real account (to be confirmed in 5.6): role App Manager is enough; Play permissions "View app information (read-only)" + "Manage store presence"; new Play permissions can take up to 24 h. Individual Apple keys: the UI says they are not supported yet (the import needs an Issuer ID).
- `project.yaml`: `store: {apple: {account, app_id, bundle_id, platform}, google: {account, package_name}, locales: {apple: {en: en-GB}, google: {…}}}`; only stores listed there are uploaded.
- Locale mapping: explicit entry → project language if it is a store code (`fr-CA`, `pt-BR`) → built-in default for the tag → default for the base language (`de-AT` → `de-DE`). Google uses `iw-IL` for Hebrew, `zh-CN`/`zh-TW`, `es-419`; Apple uses `zh-Hans`/`zh-Hant`, `he`, `no`, plain `ja`, `ko`, `it`. Lists: `crates/core/data/store-locales.yaml` (checked 2026-11-10).
- Validation (only for mapped stores): language without store locale, two languages on the same store locale (e.g. `de` + `de-AT`), unknown code, store without any target slot, two targets on the same display type for the same language.
- MCP: `store_config_get` is read-only (no secrets); assistants cannot import keys or change the mapping (`apply_ops` refuses `set_store`).
- Testing without the real keychain: `ARRANGIA_KEYRING=mock` (in-memory), `ARRANGIA_KEYRING_SERVICE=<test name>`; the dev bridge uses the in-memory keychain unless `--system-keychain`.

### Store upload – setup (Phase 5; docs checked 2026-11-10, details in ADR 0017)
- Apple, App Store Connect API **team key**: App Store Connect → Users and Access → **Integrations** tab → "App Store Connect API" (left column) → Team Keys → "Generate API Key" (or +) → name + **Access** role → Generate. Needs an **Admin** account to create team keys. Role for screenshot upload: **App Manager** (edits App Store details; Admin works too but is broader than needed).
- The private key `.p8` can be **downloaded only once** (Apple keeps no copy). Note the **Key ID** (in the key list) and the **Issuer ID** (shown above the key list). Lost/leaked → revoke the key in the same place.
- Individual keys (user profile → Individual API Key) also work: no Issuer ID, access = that user's roles; a team Admin must grant "Generate Individual API Keys".
- Apple only lets you change screenshots on a version in an editable state (e.g. "Prepare for Submission", rejected); otherwise create a new version in App Store Connect first. Upload error then: "no editable App Store version".
- Apple processes each uploaded screenshot (seconds to minutes); wrong dimensions show up as a processing error per image.
- Apple's API limit: about 3500 requests/hour per key (rolling); Arrangia waits and retries on rate limits.
- Note: Apple deprecated the screenshot-set API (API 4.5.1) in favour of the "App Asset Library"; it still works, Arrangia will follow (ADR 0017).
- Google Play, service account: Google Cloud Console → create/select a project → enable "**Google Play Android Developer API**" → IAM & Admin → Service Accounts → "Create service account" → Keys → Add key → JSON (download). Linking the Play developer account to the Cloud project is no longer needed.
- Play Console → **Users and permissions** → "Invite new users" → service account e-mail → app permissions: **"Manage store presence"** (store listing text and images) + "View app information (read-only)" for the app(s) → Invite user.
- Google image upload: PNG/JPEG, max 15 MB per image; images appear in upload order. Changes go live only when the edit is committed (one commit per upload run).
- Secrets: both keys are imported once in the app settings and stored in the system keychain; Arrangia never writes them to the project or logs (5.1).

### Store upload – how to upload (Phase 5.2/5.5, ADR 0018)
- Always **plan → confirm → upload**. Nothing goes to a store without a plan and an explicit confirmation (also not from an AI assistant).
- Setup checklist first: the Upload dialog lists what is missing, in order, each with a button: 1. App settings → Stores (no App Store Connect key / Google service account on this computer, or the project's account is not set up here) → "Open app settings"; 2. Project settings → Stores (project not linked, no account or app chosen, store-language conflicts; languages without store language are a warning) → "Open project settings"; 3. Targets (no target with a store slot, two targets on the same slot) → "Open targets". The list refreshes when the opened dialog is closed. Codes: `no_account`, `account_missing_for_project`, `project_not_mapped`, `store_locale_unknown`, `store_locale_conflict`, `no_store_locale`, `no_store_slots`, `store_slot_conflict` (`upload_readiness`, also `readiness` in every plan; MCP `upload_plan` prints them as numbered hints).
- GUI: menu → "Upload to stores…" (or "Upload…" in the export dialog). One tab per store the project is mapped to. The plan table lists target · language · store locale · action (Upload / Replace / Unchanged / Skipped / Blocked) · images · problems. "Confirm & upload" asks once more, then shows progress per set; "Cancel upload" stops it; "Upload log" lists past runs.
- CLI: `arrangia upload plan [--store apple|google] [--target ID]… [--lang L]… [--remote] [--force] [--format json]` prints the plan and a confirm token; `arrangia upload execute --confirm <token> [--yes] [--changes-not-sent-for-review]` uploads with progress on stderr (asks y/N in a terminal; `--yes` for scripts); `arrangia upload log [--last] [--events]`. `--app-dir` / `ARRANGIA_APP_DIR` selects the settings with the store accounts. Exit codes: plan 1 = blocked; execute 0 = completed, 1 = partial/failed/cancelled, 2 = refused (token, keys).
- What the plan does: renders the store targets into `<project>/.cache/upload/images` (incremental, like export), puts the images in store order, hashes them, and compares with the last successful upload in the log. Same images → "Unchanged" (skipped). `--force` / "Upload unchanged sets too" uploads anyway.
- "Check store state" / `--remote`: also asks the store (needs network + the account key): Apple – app, editable version (else blocked: "no editable App Store version"), whether the localization and screenshot set exist, the checksums of the screenshots there; Google – opens a temporary edit (deleted again, nothing changes), compares the listing images' SHA-256. A set the store already holds identically shows "Unchanged".
- Blocked = cannot upload: validation errors of that target/language (missing screenshots, wrong size/format, too many images), two languages or targets on the same store set, file too large, account/app id missing. Fix them or narrow the plan with `--target`/`--lang`.
- Skipped: languages without a store locale; Apple targets of the other platform (`APP_DESKTOP` is the macOS version, the others iOS – `store.apple.platform`).
- The confirm token is valid 30 minutes, once, and only for the project state of the plan: any saved change or changed image → "make a new plan".
- Apple: each set is made to hold exactly the planned images in order: screenshots already there with the same file content (checksum) are kept, everything else in the set is deleted, the missing images are uploaded, the order is set and verified; a missing localization or set is created. "Upload unchanged sets too" (`--force`) uploads every image again. Apple also wants the same language in the app information before submission (App Store Connect shows it).
- Apple processing: after the upload Apple processes every screenshot (usually seconds; in the first real run one image stayed in `UPLOAD_COMPLETE` for over 10 minutes). Arrangia uploads all images of a set first, then waits for all of them together, up to 15 minutes per set. Images still processing after that are **not** an error: the set shows "Uploaded, Apple processing", the run finishes with "Apple is still processing N image(s)". They are in App Store Connect and are not uploaded again. Check later with "Check processing" (result) / "Check store state" (plan) / `arrangia upload plan --remote`: done → "Unchanged"; still processing → "Unchanged" with "Apple still processing N"; failed in the meantime → "Replace" (only the missing images are uploaded).
- Apple processing failed (`FAILED`, e.g. wrong dimensions): the set fails with Apple's message; the failed image is deleted; the next run uploads only that one.
- Progress per set in the GUI: "x.png: reserving…" → "uploading 45 %" → "committing…" → "Apple is processing 2 image(s) (UPLOAD_COMPLETE, 2:31)"; CLI prints the same on stderr. Cancel works at any time, also while waiting for Apple.
- A run never stays "running": a watchdog stops a run without any progress for 10 minutes (status failed, "no progress for …"); runs of an app that was closed or crashed during an upload show as "interrupted" the next time the upload log is opened (dialog, plan, `upload log`). Just plan and upload again.
- No editable App Store version (all versions released or in review): one message for the whole store – "No editable App Store version. Create a new version (e.g. 2.1.1) in App Store Connect, then plan again. Latest version: 2.1.0 (Ready for Distribution)." – with a button "Open App Store Connect" (the app's version page). The plan with "Check store state" shows it as a blocking step; in a run the Apple sets are "Skipped".
- Google: one edit for all languages; per language + type all images are deleted and re-uploaded in order, then committed. Any error or cancel discards the edit → nothing changes in Play. If Play refuses the commit asking for "changes not sent for review", run again with that option.
- Resume: just plan and upload again – sets that completed (same images) are skipped, failed/cancelled ones are uploaded again. A cancelled Apple set may be incomplete in App Store Connect until then.
- Rate limits: the run pauses ("Paused", up to 15 min) and retries; otherwise the set fails with "rate limited – try again later".
- Log: `<project>/.cache/uploads.sqlite` (runs, sets, files with store ids and status `done`/`processing`/`failed`, events with their real time: HTTP method, path, status – Apple's processing polls only as state changes – never keys or tokens). Not in Git (`.cache/`).
- AI assistants (MCP): `upload_plan` is free (dry run). `upload_execute {confirm_token}` needs the user's click "Confirm upload" in the Arrangia window (dialog with the plan; declined or unanswered for 5 minutes → `not_confirmed`). Without the app (`arrangia mcp --headless`) assistant uploads are off unless the app setting `mcp_allow_headless_upload: true` is set in `settings.yaml`. Progress: `upload_status`, history: `upload_log`.

## 4. Reference notes
- CLI exit codes: 0 ok, 1 project problems (validation errors / export errors), 2 command could not run.
- `validate --strict` fails on warnings; `--preview` treats missing screenshots as warnings.
- Text overflow (`text_overflow`) is an **error** (exit 1) – only the affected language/target; other languages still validate clean.
- `render --gallery --max-edge N`: N is the image *height* of the gallery, not its long side.
- Keyboard: arrows nudge 1 lu (⇧ 10), ⌘D duplicate, ⌘G / ⇧⌘G group/ungroup, `[` `]` z-order, ⌘Z/⇧⌘Z, ⌥ disables snapping while dragging, ⇧ free rotation / axis lock, ⌘N/⌘O/⌘E/⌘, menus.
- Zoom (all centre views: Canvas, Pages, Store gallery, All languages; each view keeps its own zoom for the session): bottom-bar −, %, +, Fit; ⌘+ / ⌘− zoom, ⌘0 fit, ⌘1 = 100 % (one export pixel per screen pixel; in the gallery: full-size gallery); ⌘/Ctrl + wheel or trackpad pinch zooms around the pointer; wheel/trackpad scroll pans (⇧ = horizontal); Space + drag or middle-button drag pans. In Pages, Store gallery and All languages a plain drag pans too (a click on a cell in All languages still opens it in the canvas).
- Fit mode: after Fit (and when a project or strip opens) the view stays fitted – resizing the window or a sidebar re-fits; the Fit button is highlighted while fit mode is on. Any manual zoom or pan ends it.
- Images still rendering show a grey placeholder with a small spinner (only until the first picture arrives; re-renders after edits keep the old picture until the new one is there). Rendering follows the zoom: zooming in requests sharper images.
- All languages: language labels sit in a fixed column on the left; Fit keeps rows at least ~140 px high and lets you scroll (pan) through many languages.
- Dev bridge (developers only): run the UI in a browser – see CLAUDE.md.

## Contributor notes
- Built-in device frames are own parametric artwork (devices.yaml + render/frames); how to add devices + ready-made Claude prompt: docs/dev/adding-devices.md.

## 5. Limits & FAQ
- Colour management (Display P3 → sRGB) not yet.
- RTL layouts not mirrored yet.
- Native store app gallery gaps are estimates.
