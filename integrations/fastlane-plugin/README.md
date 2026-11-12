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

Two actions that call the `vitrine` CLI – fastlane runs on every CI system
mobile teams use (GitHub Actions, Bitrise, Codemagic, CircleCI, Jenkins, and
locally), so this one plugin covers all of them:

| action | does |
|---|---|
| `vitrine_validate` | `vitrine validate`; fails the lane on errors (warnings with `strict: true`) |
| `vitrine_export` | `vitrine export --layout fastlane --out ./fastlane`; sets `DELIVER_SCREENSHOTS_PATH` / `SUPPLY_METADATA_PATH` (unless already set) and the lane context `VITRINE_SCREENSHOTS_PATH`, `VITRINE_METADATA_PATH` |

The CLI is found via the `binary` option, `$VITRINE_CLI`, or `vitrine` on
`PATH` (Vitrine Studio app → *Install command line tool*, or the CLI
archive of a GitHub release).

```ruby
# fastlane/Pluginfile
gem "fastlane-plugin-vitrine"

# fastlane/Fastfile
lane :screenshots do
  capture_screenshots                                   # snapshot, as before
  sh("vitrine", "import", "screenshots", ".", "--project", "../store-screens")
  vitrine_validate(project: "store-screens", strict: true)
  vitrine_export(project: "store-screens")              # replaces frameit
  upload_to_app_store(skip_binary_upload: true, skip_metadata: true, overwrite_screenshots: true)
  upload_to_play_store(skip_upload_apk: true, skip_upload_aab: true, skip_upload_metadata: true,
                       skip_upload_changelogs: true, skip_upload_images: false, skip_upload_screenshots: false)
end
```

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
