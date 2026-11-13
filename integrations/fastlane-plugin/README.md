# fastlane-plugin-vitrine – DRAFT

> **Draft, not published.** Not on RubyGems, not in fastlane's plugin list.
> Publishing = a gem `fastlane-plugin-vitrine` pushed to RubyGems (fastlane's
> plugin list picks gems up by that prefix), ideally from its own repository
> (proposed `vitrinestudio-app/fastlane-plugin-vitrine`) with fastlane's
> usual plugin scaffold (`fastlane new_plugin vitrine`: rspec, rubocop,
> CI). Owner decisions open: name and repository. Licence: MIT (decided
> 2026-11-12).
> Only Ruby syntax was checked here (`ruby -c`); the actions were not run
> inside fastlane yet.

Three actions that call the `vitrine` CLI – fastlane runs on every CI system
mobile teams use (GitHub Actions, Bitrise, Codemagic, CircleCI, Jenkins, and
locally), so this one plugin covers all of them:

| action | does |
|---|---|
| `vitrine_import` | `vitrine import screenshots`; copies snapshot/screengrab captures (default `./fastlane/captures`, else `./fastlane`; or any folder via `source`) into the project; Vitrine Studio's own exports are skipped; `lang_map`/`device_map`/`key_map` as hashes, `save_mapping`, `dry_run`; fails the lane when files cannot be mapped |
| `vitrine_validate` | `vitrine validate`; fails the lane on errors (warnings with `strict: true`) |
| `vitrine_export` | `vitrine export --layout fastlane --out ./fastlane`; sets `DELIVER_SCREENSHOTS_PATH` / `SUPPLY_METADATA_PATH` (unless already set) and the lane context `VITRINE_SCREENSHOTS_PATH`, `VITRINE_METADATA_PATH`; `foreign_images: "fail"` (default) / `"warn"` / `"remove"` for images in the upload folders that Vitrine Studio did not export |

The CLI is found via the `binary` option, `$VITRINE_CLI`, or `vitrine` on
`PATH` (Vitrine Studio app → *Install command line tool*, or the CLI
archive of a GitHub release).

Raw captures and finished images live in separate folders: snapshot and
screengrab write below `fastlane/captures/` (their usual structure), Vitrine
Studio exports into deliver's `fastlane/screenshots/` and supply's
`fastlane/metadata/android/`. deliver and supply upload every image in those
folders, so only Vitrine Studio's images should be there (ADR 0030 in the
Vitrine Studio repository).

```ruby
# fastlane/Snapfile
output_directory("./fastlane/captures/screenshots")

# fastlane/Screengrabfile
output_directory("fastlane/captures/metadata/android")

# fastlane/Pluginfile
gem "fastlane-plugin-vitrine"

# fastlane/Fastfile
lane :screenshots do
  capture_screenshots                                   # snapshot → fastlane/captures/screenshots
  capture_android_screenshots                           # screengrab → fastlane/captures/metadata/android
  vitrine_import(project: "store-screens")              # reads fastlane/captures
  vitrine_validate(project: "store-screens", strict: true)
  vitrine_export(project: "store-screens")              # replaces frameit
  upload_to_app_store(skip_binary_upload: true, skip_metadata: true, overwrite_screenshots: true)
  upload_to_play_store(skip_upload_apk: true, skip_upload_aab: true, skip_upload_metadata: true,
                       skip_upload_changelogs: true, skip_upload_images: false, skip_upload_screenshots: false)
end
```

Keeping the captures in `fastlane/screenshots` and `fastlane/metadata/android`
(the snapshot/screengrab defaults) still imports correctly – the import
skips the files Vitrine Studio exported (listed in `fastlane/.vitrine-export.json`,
or named `01_<target>.png` / `01_<locale>.png`) – but then the captures sit
in the upload folders: `vitrine_export` stops the lane and lists them.
`foreign_images: "remove"` deletes them after each export instead (they are
captured again on the next run); `"warn"` only prints them.

Paths in the actions are relative to the folder fastlane runs actions in
(the project root, one level above `fastlane/`).

Why not also wrap `vitrine upload`? deliver/supply already upload in a
fastlane setup, and Vitrine Studio's own upload has a plan → confirm
token → execute flow that a single action would hide. A later
`vitrine_upload` action could run `upload plan` and `upload execute
--confirm <token> --yes` with the credentials mapped from fastlane's
`app_store_connect_api_key` (owner decision).

## Licence

MIT (see `LICENSE`). The plugin only runs the `vitrine` program; Vitrine
Studio itself is licensed under the GNU GPL v3 or later.
