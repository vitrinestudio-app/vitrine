# 0027 – Imported frames stored in the project (for CI)

- Status: accepted (owner request 2026-11-12; details decided by the agent)
- Date: 2026-11-12
- Amends: 0012 (imported Apple bezels are no longer "never in a project"),
  0024 (pack id `imported` reserved)

## Context

ADR 0012 keeps imported Apple Product Bezels app-wide only. On a build
server (CI) that store is empty, so `vitrine export|render|validate` draws
the parametric frame (`imported_frame_missing`) and the images differ from
the owner's Mac. Apple's licence ("Apple Design Resources", mock-ups of
Apple-platform apps, no redistribution) forbids *us* to ship the bezels,
but a user may keep the files they downloaded in their own **private**
repository. The owner asked for an option "Store used device frames in the
project" so CI renders exactly what the Mac renders.

## Decision

### Layout: the app store layout, inside the project

```text
<project>/frames/imported/
  README.md                          generated licence notice (+ .gitignore advice)
  embed.yaml                         options of the copy: {all_sizes: bool}
  <source>/index.yaml                FrameSourceIndex, same as the app store
  <source>/<model>/<colour>/<orientation>/<vendor-model>.png|.mask.png
  <source>/licenses/<package slug>/License.rtf
```

`frames/imported/` is a `FrameStore` root, so install/remove/migrate,
the index format (screen rect, radius, swatch, package, licence name …)
and the mask files are reused unchanged; PNGs are copied byte for byte
(no recompression). Rejected: a frame pack per model (ADR 0024) – packs
have their own manifest, ids and compile cache, lose the vendor-specific
rules (`covers`, colour aliases, `bezel:` names) and would change
`frame: imported:apple` into `pack:<id>` in the project. With the store
layout the project file does not change at all.

The pack id `imported` is reserved (`pack_dirs` skips it, `import_pack`
refuses it): `frames/imported/` has no `frame.yaml` anyway.

### Resolution: project copy → app store → parametric

`FrameIndex::add_project_copies(source, dir, index)` merges the project's
frames into the source with `InstalledFrame.origin = Project` (a
`#[serde(skip)]` field, never written to an index); a project copy
replaces the app frame of the same model × colour × orientation × vendor
model, which is kept in `shadowed` (for "outdated" checks and copying).
Files of project copies are found under the index key `project:<source>`
(`ImportedFrame::files_key()`), so the renderer's per-source asset caches
and the export hash keep working. `vitrine_render::frame_packs::frame_index`
(the one entry point of GUI, CLI and MCP) loads the copies; nothing else
changes in resolve. `ImportedFrame.origin` (`app` / `project`) goes to the
inspector.

Why merging per frame is safe: `select_bezel` is monotone – the bezel it
picks from the full set is also picked from any subset that contains it.
The project stores exactly the picked bezels, so CI (project copies only)
and the Mac (project + app) choose the same bezel. "All sizes" keeps every
vendor model of a used model × colour × orientation for projects that pin
`bezel:` per variant or change targets often.

### Which frames: what the targets draw

`embed::used_frames` resolves every target × language (the target's
`langs`, else all available) with the full index and collects the drawn
`imported:*` bezels (packs excluded: project packs already travel). The
plan (`embed::plan`) marks each frame `add` (only on this computer),
`update` (stored, but this computer has a different import: size, screen,
date, package, source file or bezel byte size differ), `keep`, `remove`
(stored, no longer used) and lists `missing` devices (imported frame
installed nowhere). `apply` performs it, copies the package licence file,
writes `README.md` + `embed.yaml`, prunes licence folders no frame needs and
deletes the whole folder when nothing is left; `remove` = unembed. A
project copy wins over a re-imported app frame (CI and Mac stay equal);
"update" is offered, never automatic.

No new field in `project.yaml`: the folder's existence is the option
("stored"), `embed.yaml` keeps `all_sizes`. Not an op (files outside the
model, like assets and packs), so no undo step; the GUI asks before the
first store and before removing.

### Licence

- Import now keeps the package's licence files in
  `<app frames>/<source>/licenses/<package slug>/` (the index already had
  the file name); embedding copies them. Stores imported earlier have only
  the name → plan field `licenses_missing` ("re-import the package").
- Every surface shows the warning: private repositories only, never
  publish the project with the folder (public repo, template, gallery,
  archive); `.gitignore` `frames/imported/` for public repositories.
  GUI confirmation, CLI `licence:` line, README in the folder, website docs,
  validation info `imported_frames_stored`.
- Templates (ADR 0023) already whitelist file kinds (no `frames/` at all) and
  render previews from the package (parametric frames only); in addition
  `extract` refuses image assets under `frames/imported/`, so a stored
  bezel can never be copied into `assets/` of a template or a gallery
  upload. The Vitrine repo's `.gitignore` ignores `**/frames/imported/`.

### Validation

- `imported_frame_not_stored` (info, per target): the target draws bezels
  only this computer has – CI would draw the parametric frame.
- `imported_frames_stored` (info, project): licence reminder with the count.
- `stored_frames_stale` (info, project, only when all targets and languages
  are validated): unused copies (respecting `all_sizes`) and copies this
  computer has a newer import of.

### Front ends

- App: `frames_embed_status {all_sizes?}` (dry run + `uses_imported`,
  `folder_bytes`), `frames_embed {all_sizes?, dry_run?}`, `frames_unembed`
  (bytes freed); `frames_list.stored` (per source); `frames_thumbnail`
  accepts `project:<source>`.
- GUI: project settings → **Frames** tab (intro, licence box, status
  "N frames stored · MB · up to date / update available", Store/Update,
  Remove, "All sizes of the used models", cards with thumbnail, size,
  targets, badge new/re-imported/stored/no longer used, missing devices);
  inspector frame hint "Bezel stored in the project" / "from this
  computer's frame store".
- CLI: `vitrine frames embed [--all-sizes | --used-only] [--dry-run]
  [--format json]` (exit 1 when used frames are missing everywhere),
  `vitrine frames unembed [--dry-run]`.
- MCP: `frames_list` reports `stored` (read-only). Storing/removing is not an
  MCP tool: copying licensed vendor files into a repository is the user's
  decision.

## Consequences

- CI renders the owner's images when the project's repository carries
  `frames/imported/` (a few MB: only drawn bezels).
- A colleague's machine with an older import still draws the project copy.
- The GUI watcher does not reload the index when `frames/imported/` changes
  on disk (git pull); re-open the project (same as packs, ADR 0024).
- If Apple's licence terms change, only the warning texts change.
