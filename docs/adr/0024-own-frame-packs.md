# 0024 – Own frame packs (`frame.yaml` + PNG/SVG)

- Status: accepted (user agreed to the concept 2026-11-10; format decided by
  the implementing agent and confirmed by the user 2026-11-11); pack id
  `imported` reserved by 0027
- Date: 2026-11-11

## Context

KONZEPT §4.2 names three frame sources: parametric (built in), imported
Apple Product Bezels (ADR 0012) and **own frame packs**: any PNG or SVG
frame plus a small `frame.yaml` (screen rectangle, mask / corner radius,
model mapping), in the project's `frames/` or app-wide. Use cases: clay /
flat mock-ups, branded frames, Figma exports, vendor kits the user may use
(Samsung press images, Meta Quest art …, see
`docs/research/2026-11-11-vendor-frames.md`), devices the catalog lacks.
ADR 0012 already has the machinery: screen detection, store index, bezel
selection by target / screenshot size, the imported-frame render path.

## Decision

### Format

A pack is a folder `<id>/` with `frame.yaml` (`format: vitrine-frame/1`,
`name`, optional `author`/`license`/`source`/`attribution`, `models`,
`frames[]`) and the images. Per frame: `file` (PNG/SVG), `color` (label,
id = slug, default `Default`), `orientation` (default from the image
aspect), `bezel` (size name, default the pack name), `models` override,
`screen` `{x,y,w,h}` + `radius`, `mask`, `cut_screen`, `raster` (SVG long
side, default 3000 px). Unknown fields are errors. Reference:
`docs/guide/frame-packs.md`.

Choices:

- **Folder name = id.** No id field in the manifest, so renaming a folder
  cannot leave two ids. Import derives it from the name (`slug`) or `--id`.
- **Screen detection by default** (transparent interior, ADR 0012), explicit
  `screen`/`mask` as override; `cut_screen` turns mock-ups with a painted
  screen into frames (alpha × (1 − mask)).
- **`models` empty = any device.** The pack frame is then stored with model
  `*` (`ANY_MODEL`) and fits every device model; this covers "a new device
  or size" without catalog changes. With `models`, one index entry per model
  (same files).
- **Colours** are the pack's colour ids; ops accept any colour slug when the
  effective frame is external (`FrameStyle::is_external`, as for
  `imported:*`), and colour remapping on a frame-source switch works the
  same (measured swatches).

### Resolution: `frame: pack:<id>`

New `FrameStyle::Pack(id)` next to `Imported(source)` (both "external",
`FrameStyle::external_source()` → index key `apple` or `pack:<id>`).
A separate prefix instead of `imported:<id>`: vendor sources have built-in
rules and app-wide stores, packs have manifests and can live in the
project; ids cannot collide (`apple` vs `pack:apple`), and old projects
are unchanged. Missing pack → parametric `detailed` frame + info
`imported_frame_missing` (same code, same GUI hint path).

### Compile cache and index

Packs are **compiled** into bezel PNG + 8-bit mask PNG per frame plus an
`index.yaml` (`FrameSourceIndex`) and a `.stamp` (hash of `frame.yaml` and
the referenced files' size + mtime + a compile revision):

- project packs → `<project>/.cache/frames/<id>/` (projects ignore `.cache/`);
- app packs (`<app data>/frames/packs/<id>/`) → `<id>/.compiled/`.

PNGs are copied byte for byte unless `cut_screen` changes them. A compile
never fails as a whole: per-frame problems become `errors`, sanity findings
(screen aspect vs catalog model) `warnings`, both cached with the stamp.

`FrameIndex` gained per-source folders (`add_source(key, dir, index)`,
`source_dir`); `ImportedFrame.file/mask` are now relative to the source
folder (were: to the frames root) and the renderer keeps one asset cache per
source. App packs are added first, project packs after (project wins).
`FrameIndex::load` ignores `packs/` (no `index.yaml` there).

### SVG

Core does not render: `pack::SvgRasterizer` is implemented by
`vitrine_render::frame_packs::ResvgRasterizer` (resvg, already used for SVG
assets; no system fonts). Without a rasterizer an SVG frame is an error of
that frame. `vitrine_render::frame_packs::{load_packs, frame_index}` is
the one entry point for GUI, CLI and MCP.

### Import

`pack::import_pack(path, dest, target, id, apply, svg)`: folder (with
`frame.yaml` at the top or in its only sub-folder) or `.zip` (enclosed names
only, `__MACOSX` skipped, ≤ 500 files / 512 MB), compiled into a temporary
folder first (dry run = this check), then copied via a staging folder and
renamed into place (replacing a pack with the same id). Targets: `app` or
`project` (`<project>/frames/<id>/`). `is_pack(path)` tells packs from
vendor packages, so `vitrine frames import` and the GUI accept both.

### Front ends

- App: `frames_list` adds `packs`; `frames_pack_import {path, target?, id?,
  dry_run?}` (error details `not_a_pack` for other packages),
  `frames_pack_remove {id, target?}`, `frames_thumbnail {source:
  "pack:<id>"}`; the session's index includes the open project's packs.
- CLI: `frames import <pack> [--to app|project] [--id]`, `frames list
  --packs`, `frames remove --pack <id> [--from …]`; render/export/validate
  load the project's packs.
- MCP: `frames_list` returns packs (read-only); devices use them through
  `element_update {frame: "pack:<id>"}`.
- GUI: frame packages dialog (pick/drop a pack → check → import for this
  computer or the project; packs listed with thumbnails, remove), inspector
  and project default frame pickers list packs ("Pack: <name>").

## Consequences

- Projects with project packs render identically on every machine
  (images travel in `frames/`); app packs behave like Apple bezels.
- The first render after a pack change compiles it (detection on large
  PNGs takes ~100 ms per frame); later loads read the cached index.
- The GUI watcher does not recompile on file changes inside `frames/`;
  re-open the project or re-import (follow-up).
- Landscape devices need landscape frames (no automatic rotation yet).
- Packs could later be shared through the template gallery (phase 6.3/6.6).
