# More bundled fonts – proposal (2026-11-11)

Status: **proposal, nothing bundled yet.** The owner decides on the set
and the loading approach before any font file is added.

Today Vitrine Studio bundles Inter 4.1 only (`assets/fonts/`, four static
files, 1.67 MB, embedded with `include_bytes!` in
`core::layout::fonts::BUNDLED`). Project fonts come from the project's
`fonts/` folder; system fonts are an opt-in last resort (app setting).
Users who want a typeface other than Inter today have to copy font
files into every project.

## How the facts were checked

- Licence, designer, category, axes and script subsets: `METADATA.pb` of
  each family in the Google Fonts repository
  (`https://github.com/google/fonts/tree/main/ofl/<family>`), fetched
  2026-11-11. All families below live in `ofl/` and declare
  `license: "OFL"` (SIL Open Font License 1.1).
- File sizes: GitHub contents API for the same folders, 2026-11-11
  (the files Google Fonts serves; upstream repos may differ slightly).
- Compression: gzip -9 of the bundled Inter files gives ~48 % of the
  original size (measured locally); installers (DMG, MSI, AppImage)
  compress similarly.

## Candidates

Sizes are per file, upright / italic. "var" = one variable file covering
the weight range; "static" = one file per weight.

| # | Family | Role | Files | Weights | Size | Scripts | Source |
|---|---|---|---|---|---|---|---|
| 1 | **Montserrat** (Julieta Ulanovsky et al.) | geometric sans, headlines | var `wght` + italic | 100–900 | 745 KB / 762 KB | Latin ext, Cyrillic ext, Vietnamese | github.com/JulietaUla/Montserrat |
| 2 | **DM Sans** (Colophon) | neutral grotesk, alternative to 1 | var `opsz,wght` + italic | 100–1000 | 240 KB / 285 KB | Latin ext only | github.com/googlefonts/dm-fonts |
| 3 | **Playfair Display** (Claus Eggers Sørensen) | high-contrast display serif | var `wght` + italic | 400–900 | 301 KB / 279 KB | Latin ext, Cyrillic, Vietnamese | github.com/clauseggers/Playfair |
| 4 | **Lora** (Cyreal) | calm text serif | var `wght` + italic | 400–700 | 212 KB / 221 KB | Latin ext, Cyrillic ext, Vietnamese | github.com/cyrealtype/Lora-Cyrillic |
| 5 | **Nunito** (Vernon Adams, Cyreal, Jacques Le Bailly) | rounded sans | var `wght` + italic | 200–1000 | 277 KB / 282 KB | Latin ext, Cyrillic ext, Vietnamese | github.com/googlefonts/nunito |
| 6 | **Oswald** (Vernon Adams, Kalapi Gajjar, Cyreal) | condensed sans | var `wght` (no italic) | 200–700 | 172 KB | Latin ext, Cyrillic ext, Vietnamese | github.com/googlefonts/OswaldFont |
| 7 | **Bebas Neue** (Ryoichi Tsunekawa, Dharma Type) | all-caps display | static, 1 file | 400 | 61 KB | Latin ext only, capitals only | github.com/dharmatype/Bebas-Neue |
| 8 | **Caveat** (Impallari Type) | handwritten, casual notes/arrows | var `wght` | 400–700 | 404 KB | Latin ext, Cyrillic ext | github.com/googlefonts/caveat |
| 9 | **Dancing Script** (Impallari Type) | script, elegant accents | var `wght` | 400–700 | 134 KB | Latin ext, Vietnamese | github.com/googlefonts/DancingScript |
| 10 | **JetBrains Mono** (JetBrains) | monospace, developer apps, code snippets | var `wght` + italic | 100–800 | 187 KB / 192 KB | Latin ext, Cyrillic ext, Greek, Vietnamese | github.com/JetBrains/JetBrainsMono |

Also looked at: Quicksand (rounded, 125 KB, no Cyrillic – Nunito is the
better rounded choice), Barlow Condensed (static only, 18 files of
~105 KB – Oswald is one variable file), Source Serif 4 (1.2 MB, large
for a second serif).

Why these suit store screenshots:

- **Montserrat / DM Sans** – the typical bold, wide marketing headline
  look; Montserrat covers Cyrillic (DM Sans does not, which matters for
  ru/uk/bg store pages).
- **Playfair Display** – lifestyle, food, fashion, finance apps; strong
  contrast reads well at large sizes; **Lora** for longer serif
  subtitles where Playfair is too loud.
- **Nunito** – friendly rounded look for kids, health, habit and
  productivity apps; very wide weight range (up to 1000) for punchy
  headlines.
- **Oswald / Bebas Neue** – condensed headlines that fit long German or
  French titles into narrow pages (complements auto-fit); Bebas for
  sports/fitness/gaming style all-caps.
- **Caveat / Dancing Script** – hand-drawn callouts ("Try it!") and
  accents next to device frames; only for short texts.
- **JetBrains Mono** – developer tools, terminal and code screenshots.

## Other scripts (CJK, Arabic, Hebrew, Indic, Thai)

Inter covers Latin, Cyrillic and Greek. None of the candidates covers
CJK, Arabic, Hebrew, Thai or Indic scripts. Sizes of the matching Noto
families (OFL, `ofl/notosans*`, 2026-11-11):

| Family | Size |
|---|---|
| Noto Sans JP (var `wght`) | 9.6 MB |
| Noto Sans KR | 10.4 MB |
| Noto Sans SC (Simplified Chinese) | 17.8 MB |
| Noto Sans TC (Traditional Chinese) | 11.9 MB |
| Noto Sans Arabic (var `wdth,wght`) | 845 KB |
| Noto Sans Hebrew | 113 KB |
| Noto Sans Thai | 219 KB |
| Noto Sans Devanagari | 642 KB |

- CJK alone would add ~50 MB, far more than the whole app. **Do not embed
  CJK.** Options: (a) keep the current path – project fonts or the
  system fallback (macOS/Windows ship CJK fonts, but then preview and
  export depend on the machine); (b) an optional "font pack" download
  into the app data folder (like imported frames), shared by all
  projects and listed as source `pack` in the fonts pane; (c) document
  "put Noto Sans JP into `fonts/`" for reproducible exports (CI, team).
  Recommendation: (c) now, (b) later if users ask.
- Arabic, Hebrew, Thai, Devanagari are small (~1.8 MB together) and
  could ship as built-in **fallbacks** (registered as fallback chain
  for every bundled family, not offered as display faces). Caveat: RTL
  layouts are not mirrored yet (NOTES "Limits"), so Arabic/Hebrew render
  correctly shaped but in an LTR page layout. Worth doing together with
  RTL support, not before.

## Binary size: embed vs. data files

Today: Inter 1.67 MB embedded in `vitrine-core`, so it is in every
binary that links core (`vitrine` CLI incl. MCP, `vitrine-gui`).

| Set | Raw | In installer (~48 %) |
|---|---|---|
| Recommended Latin set, uprights only (Montserrat, Playfair, Lora, Nunito, Oswald, Bebas, Caveat, Dancing Script, JetBrains Mono) | 2.5 MB | ~1.2 MB |
| … plus italics (Montserrat, Playfair, Lora, Nunito, JetBrains Mono) | 4.2 MB | ~2.0 MB |
| + DM Sans (both) | +0.5 MB | +0.25 MB |
| + RTL/Indic/Thai fallbacks | +1.8 MB | +0.9 MB |

- **Embedding** (`include_bytes!`, as today): simplest, always
  available (CLI from `cargo install`, tests, MCP headless), deterministic.
  Cost: each binary grows by the raw size (the GUI app bundle and the CLI
  both carry them: 2 × 4.2 MB for the full Latin set), plus a bit of
  compile/link time. Startup memory is unaffected in practice: embedded
  data is mapped from the binary and only paged in when read, but
  `FontLibrary::bundled()` today registers every face via fontique
  (reads the tables of every file on every library build).
- **Data files next to the binary** (Tauri `bundle.resources`, a
  `fonts/` folder beside the CLI in release archives): the binaries stay
  small and the files are shared by GUI and CLI in one installation.
  Cost: install layout must be known at runtime (Tauri resource dir on
  macOS `Contents/Resources/`, next to the exe on Windows/Linux); a CLI
  installed without the folder silently loses the fonts, so projects
  render differently – a reproducibility risk unless a missing bundled
  family is reported (diagnostic like `font_missing`).

What `core::layout` would need for data files / lazy loading:

1. A small **manifest** (`assets/fonts/fonts.yaml`: family, file,
   weight range, italic, scripts/coverage summary, licence file),
   embedded, so families, weights and the fonts pane are known without
   reading any font file.
2. `FontLibrary` faces with **lazy blobs**: `FontSource::Bundled` gets
   a path or an embedded slice; the bytes are loaded (file read, or
   `include_bytes!` slice) on first `match_face` / layout use of that
   family, then cached process-wide (`OnceLock` per file) so every
   `RenderContext` shares one copy.
3. A **resolver for the font folder**: `VITRINE_FONTS_DIR` env, then the
   Tauri resource dir (passed in by the GUI), then `<exe>/fonts` and
   `<exe>/../share/vitrine/fonts`; tests and golden images keep using
   the embedded Inter so they stay hermetic.
4. **Variable fonts**: parley/skrifa already carry variation
   coordinates through layout and glyph outlines (`normalized_coords` in
   `layout/engine.rs`, `run.coords` in `render/src/text.rs`), but
   `FontLibrary::match_face` stores one weight per face (the default
   instance). Before switching to variable files, `match_face` and the
   fonts pane must understand a weight *range* per face, with a golden
   test for e.g. Montserrat 300/700/900 from one file.

## Recommendation

1. **Set (9 families, Latin + mostly Cyrillic):** Montserrat, Playfair
   Display, Lora, Nunito, Oswald, Bebas Neue, Caveat, Dancing Script,
   JetBrains Mono – with italics for Montserrat, Playfair, Lora, Nunito
   and JetBrains Mono. Keep Inter as the default. DM Sans only if the
   owner wants a second neutral grotesk (no Cyrillic).
2. **Loading:** keep **embedding**, but make it lazy (manifest + blobs
   loaded on first use, item 1–2 above). 4.2 MB raw (~2 MB in the
   installer) is acceptable for a desktop app and keeps CLI, MCP
   headless and CI exports reproducible without an install layout.
   Revisit data files only if the set grows past ~10 MB.
3. **Variable files** (one file per style) once `match_face` supports
   weight ranges (item 4); until then use static instances of the
   weights offered in the UI (e.g. 400/600/700/900), which costs about
   the same per weight as the Inter files today (~120–400 KB each).
4. **No CJK embedding.** Document project fonts for CJK now; consider an
   optional font-pack download later. Small Arabic/Hebrew/Thai/Devanagari
   Noto fallbacks together with RTL support.
5. Licence handling: one `OFL.txt` per family next to the files
   (copyright lines differ per family), listed in the About/licence
   notices like Inter today; never modify the fonts (OFL "Reserved Font
   Name" rules then do not apply).
