# 0031 – fastlane: captures and exports in separate folders

- Status: accepted
- Date: 2026-11-13
- Amends: ADR 0010 (screenshot import), the fastlane export layout
  (`core::fastlane`, `render::export`)

## Context

The documented lane imported the raw captures from `fastlane/` and
exported Vitrine Studio's images with `--layout fastlane --out fastlane`
into the same folders. Checked against fastlane `master` on 2026-11-13
(`docs/research/2026-11-10-fastlane-conventions.md`, section "Captures and
uploads share folders"):

- snapshot writes to `fastlane/screenshots/<locale>/` by default
  (`output_directory`), the folder deliver uploads. Its
  `clear_previous_screenshots` (default off) deletes every `*.png` one
  level below `output_directory`, Vitrine Studio's images included.
- screengrab writes to `fastlane/metadata/android/<locale>/images/
  <type>Screenshots/` by default, the folder supply uploads. Its
  `clear_previous_screenshots` (default off) deletes the `*.png` in those
  folders.
- deliver uploads every `*.{png,jpg,jpeg}` of `screenshots/<locale>/`
  (only `*framed*`/`*watch*` files once a `*_framed.*` file exists). Raw
  iPhone captures have valid store sizes, so they land in the same
  screenshot set as Vitrine Studio's images. Without
  `overwrite_screenshots` deliver adds to the remote set (MD5 duplicates
  skipped).
- supply uploads every `*.{png,jpg,jpeg}` of `images/<type>/` and replaces
  the remote screenshots of that type; `images/<type>.{png,jpg,jpeg}`
  takes the last glob hit.

So a shared folder meant: the next import read Vitrine Studio's own
exports as captures (unresolved rows: the plugin's `vitrine_import` failed
the lane), and deliver/supply uploaded the raw captures next to the
designed images.

## Decision

1. **Recommended layout: captures in their own folder.** snapshot and
   screengrab write below `fastlane/captures/` in their usual structure
   (`Snapfile: output_directory("./fastlane/captures/screenshots")`,
   `Screengrabfile: output_directory("fastlane/captures/metadata/android")`);
   `vitrine import screenshots fastlane/captures` reads it exactly like a
   whole fastlane folder (no import change needed), the export writes
   deliver's and supply's folders, which then hold only Vitrine Studio's
   images. The plugin's `vitrine_import` uses `./fastlane/captures` when
   it exists, else `./fastlane` (with a hint).
2. **Import never takes Vitrine Studio's exports.** Files listed in an
   export manifest `.vitrine-export.json` (in the source tree or up to
   three folders above the source) are skipped with the reason "exported
   by Vitrine Studio". In fastlane folders, files named like the fastlane
   export layout are skipped too when no manifest lists them
   (`<nn>_<target id>[_<marker>]` with a target of the project in deliver
   folders, `<nn>_<own locale>` in supply folders) – snapshot
   (`<Device>-<screen>`) and screengrab (`<name>_<millis>`) never name
   files like that.
3. **Export reports foreign images, deletes them only on request.** With
   the fastlane layout, images in the upload folders the export writes into
   (deliver locale folders, supply screenshot folders, supply single images
   of the same type with another extension) that no export of this folder
   produced (not in the manifest) are listed in `ExportReport::foreign`
   with one warning per folder. `--remove-foreign`
   (`ExportOptions::remove_foreign`) deletes exactly those files after a
   complete export (`removed_foreign`); without it nothing outside the
   manifest is ever touched. Folders the export does not write into are
   not scanned (a manually kept locale stays the user's business).
4. **The plugin stops the lane before an upload would mix images.**
   `vitrine_export(foreign_images:)`: `"fail"` (default) fails with the
   list and the two fixes, `"warn"` only prints, `"remove"` passes
   `--remove-foreign`.

## Consequences

- Existing lanes that keep one folder keep working for the import (own
  exports skipped) and get a clear failure in `vitrine_export` instead of
  a mixed upload; moving the captures (two config lines) or
  `foreign_images: "remove"` fixes it.
- With snapshot's `clear_previous_screenshots` and a shared folder,
  snapshot also deletes Vitrine Studio's PNGs; the next export re-renders
  them (the incremental cache checks that the file exists).
- The GUI and MCP export show the warning through the layout notes; they
  have no `remove_foreign` switch (CLI and plugin only).
