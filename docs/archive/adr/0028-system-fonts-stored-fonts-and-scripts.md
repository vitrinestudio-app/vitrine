# 0028 – System fonts, stored fonts and fonts for other scripts

- Status: accepted (owner request 2026-11-12; details decided by the agent,
  open points in `docs/plan/STATUS.md`)
- Date: 2026-11-12
- Amends: 0003 (fonts: "system fonts are never loaded"), 0009 (system
  font fallback), 0023 (fonts in templates)

## Context

Until now a style could only name a bundled family or a font file copied
into the project's `fonts/`. Designers have whole font sets installed and
found copying files into every project odd (owner feedback 2026-11-12).
System fonts existed only as an opt-in last resort for characters no
other font had (ADR 0009). Scripts beyond Latin/Cyrillic/Greek had no
bundled font at all, and CJK depended on that opt-in fallback.

## Decision

### Resolution order

A family a style names resolves **bundled → project (`fonts/`, then
`fonts/system/`) → installed system fonts**. Bundled families win name
collisions (an installed "Montserrat" never replaces the bundled one);
project fonts win over installed ones. One function builds every
project's library – `core::layout::setup::project_library(root,
project, system)` – used by the GUI session, the CLI (`vitrine
render|export|validate|strings import|fonts|new`) and MCP (through the
app layer), so preview, export and validation on one machine see the
same fonts. `RenderContext::for_project` wraps it; `RenderContext::new`
stays bundled + project only (tests, goldens).

### Installed fonts: index and loading

`core::layout::system`: a process-wide index of the platform font
folders (macOS `/System/Library/Fonts`, `/Library/Fonts`,
`~/Library/Fonts`, downloaded `AssetsV2/com_apple_MobileAsset_Font*`;
Windows `%WINDIR%\Fonts`, per-user fonts; Linux `/usr/share/fonts`,
`/usr/local/share/fonts`, `~/.local/share/fonts`, `~/.fonts`), scanned
recursively with fontique's path loader – no platform font API, no C
dependency, the same code on every OS. The index keeps metadata only
(family, faces, weight range, italic, file); `FontLibrary::
add_system_family` maps the files (fontique memory-maps them) when a
project names the family. Hidden (`.`) families and `LastResort` are
skipped. The first scan takes ~1 s on macOS (319 families on the owner's
Mac), so it only runs when a project names a family the library lacks or
has characters no loaded font covers (`setup::needs_system_fonts`);
`fonts_refresh` / "Rescan installed fonts" rescans.

### One setting instead of two

The app setting `system_font_fallback` (opt-in, last resort) becomes
**`system_fonts`** ("Use installed fonts", **on by default**), covering
both named families and the fallback for missing characters; the old key
is read as an alias (only `true` was ever written). Rationale: both make
output depend on the machine, both are flagged by validation, and two
switches for "use what this computer has" confused more than they
helped. Off = bundled and project fonts only. The CLI uses system fonts
by default too; `--no-system-fonts` and `VITRINE_SYSTEM_FONTS=off` (any
process, e.g. CI) switch them off. Tests and golden images never use
them (they build libraries from bundled fonts directly; the example
fixture names only bundled families, so even the CLI tests never scan).

### Fallback chain per text

`family → project fallbacks (styles.fonts) → library fallbacks → Inter →
bundled script fonts of the text → the language's system fallbacks →
system fallbacks for every language`. `TextInput` gained `lang` (the
text's language): it selects the language's fallbacks and is set as the
shaping locale (`StyleProperty::Locale`).

- **Bundled script fonts** (always, deterministic): Noto Sans Arabic,
  Hebrew, Devanagari, Bengali, Tamil and Thai are added to a text's chain
  when it contains characters of that script (`layout::script`). They are
  ordinary bundled families (category `script`, selectable in pickers),
  lazily registered like the others.
- **CJK** (system fonts on): for Han/kana/Hangul characters the first
  installed family of the language's preference list is loaded – `ja`:
  Hiragino Sans, Yu Gothic, Meiryo, Noto Sans CJK JP …; `zh-Hans`:
  PingFang SC, Hiragino Sans GB, Heiti SC, Microsoft YaHei, Noto Sans CJK
  SC …; `zh-Hant`/`zh-TW`: PingFang TC, Heiti TC, Microsoft JhengHei …;
  `zh-HK`; `ko`: Apple SD Gothic Neo, Malgun Gothic, Noto Sans CJK KR – as
  a fallback **for that language only**, so Japanese and Chinese texts
  get their own Han glyph shapes. Then, as before, the family covering
  most remaining characters (greedy, ≤ 8 per call) for every language.

### Stored system fonts (`fonts/system/`)

Mirrors ADR 0027: "Store used system fonts in the project"
(`layout::stored`) copies the files of the installed families the project
uses – named by styles or loaded as fallback – into `fonts/system/` with
`fonts.yaml` (file, family, role `named`/`fallback` + language, original
path, size, SHA-256, licence read from the name table) and a generated
`README.md` licence notice. Only files of faces the project's weights
pick are copied (each style weight, its `**bold**` weight, upright and
italic, matched like layout; CSS matching is monotone, so the stored
subset picks the same faces). Stored fonts load as project fonts;
fallback entries rejoin the chains (`add_chain_fallback`). Plan actions
`add / update (SHA-256 differs) / keep / remove`; a stored family this
computer does not have is kept (stored on another machine). No field in
`project.yaml` (the folder is the option), not an op (files, no undo),
like ADR 0027.

Licence classification from name IDs 0, 8, 13, 14: `ofl` (may be
shared), `restricted` (Apple/Microsoft system fonts: no
redistribution), `other` (own licence text – check), `unknown` (no
licence – treat as not redistributable). Every surface warns: GUI
confirmation with each font's licence, plan field `restricted`, CLI
`licence:` line, README, website docs, validation `system_fonts_stored`.

### Templates

`template.yaml` gains `fonts_required: [{family, reason: license |
not_in_project}]`. Saving refuses a non-OFL project font as before
unless `skip_unlicensed_fonts` (CLI `--skip-unlicensed-fonts`, GUI
checkbox and a "Save without these fonts" retry on the error): the font
file is left out and the family is listed as required. Families the
styles use that are not in `fonts/` (system fonts) are always listed as
`not_in_project`. `fonts/system/` is never read for templates (only files
directly in `fonts/` are candidates). Inserting / creating from a
template reports `fonts_required`; the app and CLI name the ones this
computer lacks (`template::unavailable_fonts`; texts use Inter).

### Validation (infos)

- `system_font_used` – a style uses an installed family (not bundled,
  not in the project): other machines render differently or use Inter;
  store it or use a bundled font.
- `system_font_fallback` – characters of a language are drawn with a
  system fallback font.
- `system_fonts_stored` – licence reminder with the stored families.

### Front ends

- App: `catalog_fonts` gains `stored`, `system_named`, `installed`
  (every installed family for pickers, without bundled names);
  `fonts_embed_status`, `fonts_embed {dry_run}`, `fonts_unembed`,
  `fonts_refresh`; `template_insert` returns `fonts_unavailable`.
- GUI: searchable grouped font picker (Vitrine Studio · This project ·
  Installed on this computer) in the text and span styles; Project
  settings → Fonts: sources per family, system font state, rescan, the
  "Store used system fonts" section with licence badges, the bundled
  list and the script fonts; app setting "Use installed fonts".
- CLI: `vitrine fonts [--installed]`, `vitrine fonts embed [--dry-run]
  [--format json]`, `vitrine fonts unembed [--dry-run]`, global
  `--no-system-fonts`.
- MCP: unchanged tools; it renders with the same library as the GUI
  (setting `system_fonts`). Storing fonts is not an MCP tool (copying
  licensed files into a repository is the user's decision, as in
  ADR 0027).

### Complex scripts

parley's `complex-scripts` feature is now on: ICU4X dictionary data
for line segmentation of Thai, Lao, Khmer and Burmese (and dictionary
word segmentation for Chinese/Japanese). Without it Thai had no break opportunities (emergency
breaks) and ICU4X printed "No segmentation model" to stderr. Shaping
(Arabic joining, Indic reordering and conjuncts, Thai marks) is harfrust
and needed nothing. Layout now aligns lines with `Alignment::Left` and
applies the model's alignment itself: with `Start`, parley right-aligned
RTL lines and the model's alignment was added on top (RTL text ended up
right-aligned for `left` and pushed out of the box for `center`).
Alignment stays physical (`left` means left in every language); RTL
languages set `align: right` with a language override.

## Consequences

- Binary size (release, macOS arm64): +2.62 MB of fonts (six Noto
  files) and +3.44 MB of ICU segmentation dictionaries per binary.
- Korean (`ko`) breaks lines at spaces only (`WordBreak::KeepAll`).
- With system fonts on, a project may render differently on another
  machine; validation says so and storing the fonts fixes it for private
  repositories.
- CJK still needs installed fonts (macOS and Windows ship them; Linux
  needs Noto CJK) or project fonts; an optional font-pack download is
  proposed in `docs/research/2026-11-12-cjk-font-packs.md`.
- RTL limits: no automatic mirroring of layouts or alignment; a
  soft-hyphen at an RTL line end is drawn at the right edge.
