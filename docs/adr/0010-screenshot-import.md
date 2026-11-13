# 0010 – Screenshot import

- Status: accepted
- Date: 2026-11-10

## Context

KONZEPT §5.2 asks for importing screenshots with mapping rules: the
Vitrine Studio folder convention (`screenshots/<lang>/[<variant>/]<key>.png`),
fastlane snapshot output (`<locale>/<Device Name>-<screen>.png`) with a
configurable device name → variant mapping, and drag & drop with
detection by file name and pixel size. Phase 2 already imports single
files in the GUI (`assets_import`, variant from the pixel size). Task 3.3
needs the same rules in CLI and GUI, a dry run that shows what goes
where, and a mapping that can be stored in the project.

## Decision

### Plan, then apply (`core::import`)

- `plan_screenshot_import(project, project_dir, source_dir, rules) ->
  ImportPlan` only reads (file list, image headers, existing
  destinations); `plan_files` does the same for a list of files (flat
  rules). `apply(plan, project_dir) -> ApplyReport` copies, never moves:
  each file is written to a temp file next to its destination and
  renamed (`io::write_atomic`); files whose destination already has the
  same bytes are skipped. Nothing is ever deleted (no prune).
- `ImportRules { format: auto|vitrine|fastlane|flat, mapping,
  include_framed, overrides }`. `overrides` are per-file corrections
  (`lang`, `variant` with `""` = language folder, `key`, `skip`) keyed by
  the path relative to the source folder; the GUI's row edits are just
  overrides and a re-plan, so all detection stays in Rust.
- The plan lists per file: source, destination, detected
  language/variant/key, device/locale/screen as found, variant
  candidates, a confidence (`high`/`medium`/`low`, the lowest of the
  three parts) with notes explaining each decision, and an action:
  `new`, `overwrite`, `unchanged`, `duplicate` (another file maps to the
  same destination; the most confident one wins) or `unresolved` (no
  language, variant or key; not copied). Ignored files (non-images,
  `screenshots.html`, frameit `*_framed.png`, files outside the expected
  layout) are listed with a reason. `missing` lists, per project
  language × target screenshot variant, the used keys that still have no
  own file afterwards (`missing` or `fallback`, computed with the normal
  lookup chain over disk + planned files).

### Detection rules

- **Format**: auto picks fastlane when `screenshots.html` exists or most
  files look like `<locale>/<Device Name>-<screen>`, Vitrine Studio when files
  sit in language folders with slug names, else flat. A leading
  `screenshots/` folder is ignored (a project can be a source).
- **Language**: locale map → exact project language → case-insensitive
  → shorter subtags (`de-DE` → `de`; this is the documented default, so
  it counts as certain). Languages that are not project languages are
  unresolved, never created implicitly.
- **Variant**: device map (fastlane device name, case-insensitive) or a
  variant folder / variant id in the file name → the pixel size against
  the catalog's typical screenshot sizes → the display aspect (±2 %);
  candidates used by the project's targets win, several such candidates
  make the match `low`. Flat files without any variant go into the
  language folder (served to all variants).
- **fastlane device names** contain spaces, parentheses and sometimes
  hyphens (`iPad Pro 13-inch (M4)`). A name splits at a `-` outside
  parentheses; the device prefix is the longest candidate shared by as
  many files as the shortest one, and a purely numeric segment
  (`…-01-home`) is never part of the device. Mapped device names always
  win.
- **Key**: key map (raw screen name, then without numeric prefix) →
  screen name without numeric prefix (`01_home` → `home`), slugified →
  matched against the keys the project's device elements use (exact, then
  ignoring `-`/`_`/case). Unused keys are imported with low confidence.

### Mapping in `project.yaml`

```yaml
import:
  fastlane:
    devices: {iPhone 17 Pro Max: ios-phone, iPad Pro 13-inch (M4): ios-tablet}
    locales: {de-DE: de, en-US: en}
    keys: {01_home: home}
```

Optional, omitted when empty, changed only through `Op::SetImport`
(validated: variant and key values are ids). The plan's `mapping` is the
effective mapping (project + rules + what was detected: devices with one
consistent variant, locales that differ from their language, keys set by
a row correction or map); "save mapping" stores exactly that, so the next
import maps the same way without depending on pixel sizes. The section
is called `fastlane` because that is where names need mapping; `locales`
and `keys` apply to every format.

### Front ends

- CLI: `vitrine import screenshots <dir> [--format …] [--lang-map
  L=LANG] [--device-map "NAME=VARIANT"] [--key-map S=KEY]
  [--include-framed] [--dry-run] [--save-mapping] [--format-output
  text|json]`. Exit 1 when files are unresolved, duplicate or failed to
  copy, 2 when it cannot run. `--save-mapping` is ignored in a dry run.
- GUI: `import_plan {dir | files, format?, mapping?, overrides?}` and
  `import_apply {…same, save_mapping}`. Apply re-plans with the same
  arguments instead of taking a plan id or a client-side plan: the
  backend stays stateless and a plan edited in the UI cannot point
  outside the project. The mapping change is one undo step; copied files
  reach the previews through the folder watcher.

## Consequences

- One rule set for CLI, GUI and (Phase 4) MCP `screenshots_import`.
- Detection by size depends on the catalog's `screenshot.sizes`; unusual
  sizes fall back to the aspect and then to the user's mapping.
- The GUI's single-file import and drag & drop (Phase 2) still use the
  older per-file dialog; `plan_files` is ready to replace it.

## Amendment 2026-11-13: own exports are never imported

Files an export manifest `.vitrine-export.json` lists (in the source or up
to three folders above it) and, in fastlane folders, files named like the
fastlane export layout are skipped with a reason. Recommended fastlane
layout with captures in `fastlane/captures/`: ADR 0030.
