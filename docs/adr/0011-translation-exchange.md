# 0011 – Translation exchange (CSV, XLIFF)

- Status: accepted
- Date: 2026-11-10

## Context

KONZEPT §5.1 asks for exchanging translations with translators as CSV and
XLIFF. Translations are flat `key → text` maps per language
(`i18n/<lang>.yaml`) with Arrangia's inline markup (§3.5, ADR 0003:
`**bold**`, `_italic_`, `[span]…[/]`, `\n`, `\-`). Translators work in
Excel/Numbers (CSV) or CAT tools (XLIFF); both must round-trip without
damaging the markup, and an import must show its effect on the layout
before it is applied. Task 3.4.

## Decision

### Core API (`core::exchange`, no file system access)

- `export(project, Format, &ExportOptions) -> Vec<ExportFile>` (in memory;
  front ends write them, `output_paths` picks names). `Format` = `csv` |
  `xliff12` | `xliff20`. Options: target languages (default: all but the
  default language), `missing_only`, `context`, CSV `delimiter` (`,` `;`
  tab) and `bom`, XLIFF `markup` (`inline` | `plain`).
- `plan_import(project, &[ImportInput], &ImportOptions) -> ImportPlan`:
  reads any mix of files (format from the extension, else content;
  UTF-8 with/without BOM, UTF-16 with BOM), compares them with the project
  and lists per language new/changed/removed/unchanged/skipped keys,
  unknown keys and columns, markup issues and per-file errors (a broken
  file never stops the others). `check_layout(&mut plan, project,
  &ValidateOptions)` validates the changed languages before and after and
  keeps `text_overflow` / `emergency_break` findings (`new` = caused by
  the import). `ImportPlan::ops()` = `set_string` ops, applied as **one
  transaction** (one undo step); `conflicts()` detects strings edited
  between planning and applying. The plan is serde both ways, so the GUI
  can show it and send it back to apply.
- "Unchanged" compares rendering (`markup::parse` text + spans), not
  bytes: `\-` vs U+00AD or `\n` vs a real newline are not changes.

### CSV

`key, <default lang>, <lang>…` plus, with context, `max_chars, context,
notes`. RFC 4180 quoting via the `csv` crate (newlines stay inside quoted
cells), CRLF line ends, optional BOM and `;` for Excel. Markup stays as
text. Import maps columns by language code (case-insensitive), requires
`key`, ignores `max_chars`/`context`/`notes`/`comment`, reports other
columns; delimiter is sniffed from the header. Empty cell = no change
(default) or remove the translation (`empty: remove`; never for the
default language). Keys not in the project are reported, or created with
`create_keys`.

### XLIFF

One file per target language (`<project>-<lang>.xlf`), source = default
language; keys without default text are skipped (XLIFF needs a source).
Unit ids: the key when it is an NMTOKEN (2.0), else `u<n>`; the key is
always in `name` (2.0) / `resname` (1.2), which import prefers. Missing
translations: 2.0 `state="initial"` without `<target>`, 1.2
`<target state="needs-translation"/>`; existing ones `translated`. Import
skips units without target and untranslated units whose target is a copy
of the source. Context goes into notes (`category="context"`/`"length"`,
1.2 `<note from="arrangia">`), not into `maxwidth`/`slr:sizeRestriction`:
the limit is an estimate (proportional fonts, auto-fit), and CAT tools
would enforce it as a hard limit.

**Markup: inline elements (default), plain text as option.** Bold, italic
and spans become paired codes, breaks and soft hyphens placeholders, so
CAT tools protect and move them as tags:

| Arrangia     | XLIFF 2.0                                                    | XLIFF 1.2                    |
|--------------|--------------------------------------------------------------|------------------------------|
| `**…**`      | `<pc type="fmt" subType="xlf:b" dataRefStart dataRefEnd>`    | `<g ctype="bold">`           |
| `_…_`        | `<pc type="fmt" subType="xlf:i" …>`                          | `<g ctype="italic">`         |
| `[name]…[/]` | `<pc type="fmt" dataRefStart="[name]" dataRefEnd="[/]">`     | `<g ctype="x-span-name">`    |
| `\n`         | `<ph type="fmt" subType="xlf:lb" dataRef/>`                  | `<x ctype="lb"/>`            |
| `\-`         | `<ph dataRef/>` (`<data>\-</data>`)                          | `<x ctype="x-soft-hyphen"/>` |

2.0 keeps the original markers in `<originalData>` so import restores the
exact marker from `dataRef*`; `subType`/`ctype` are the fallback, and
`<sc>/<ec>`, `<bx>/<ex>`, `<mrk>`, `<cp>` are understood. Ids are stable
per kind and occurrence, so source and target tags pair up. Text between
tags is Arrangia source text with minimal escaping (`\*` only where a
literal would read as markup), so import concatenates text and markers;
markup is parsed into a tree (overlapping pairs are split). The round trip
is lossless except for one corner: overlapping bold/italic where the
italic remainder starts with a space. `markup: plain` writes the markup
string as text plus a file note explaining the syntax.

Import diagnostics: unbalanced markers, unknown span styles, unmappable
inline elements (dropped, text kept) and – as info – bold/italic/span
usage that differs from the default language.

### Front ends

CLI `arrangia strings export --format csv|xliff12|xliff20 [--lang]…
[--out path|dir] [--missing-only] [--context] [--delimiter] [--bom]
[--plain-markup] [--json]` and `arrangia strings import <file>…
[--dry-run] [--create-keys] [--empty keep|remove] [--no-layout-check]
[--json]` (exit 1 when a file could not be read or the import causes new
layout errors). GUI: `strings_export`, `strings_import_plan`,
`strings_import_apply`, `dialog_save_file`; translations panel buttons
open an export dialog and an import review (diff per key, issues, layout
findings) that applies as one undo step.

## Consequences

- No new heavy dependencies: `csv` and `quick-xml` (pure Rust); XLIFF is
  written by hand for tidy, stable output and checked by a structural
  test (namespaces, required attributes, element order, NMTOKEN ids,
  `dataRef` targets) instead of schema validation.
- XLIFF 2.0 modules (size restriction, metadata) are not used; adding
  them later does not change the core mapping.
- Translation via Claude/MCP can reuse `plan_import`/`check_layout` with
  generated content.
