# project.yaml structure

The whole project file (read it with resource `vitrine://project/project.yaml`
when your client offers resources). Tools edit it for you; this is the map.
Unknown fields are errors everywhere.

```yaml
format: vitrine/1        # always
revision: 12              # bumped on every save (optimistic locking)
name: Fernwood
languages:
  default: en             # fallback for texts and screenshots
  available: [en, de, fr, fr-CA]
  fallback: {fr-CA: fr}   # optional extra fallbacks (de-AT → de → default is automatic)
styles:
  colors: {primary: "#0B3324", accent: "#F2B33D"}      # used as "@primary"
  text:                   # text styles (element `style:`)
    headline: {font: Inter, weight: 800, italic?: false, size: 96, line_height: 1.1,
               tracking: 0, color: "@primary", transform?: none|uppercase|lowercase,
               shadow?: soft, stroke?: {color, width}, glow?: {color, radius},
               hyphenate?: false}
  spans:                  # inline styles for [name]…[/] in translations
    accent: {color: "@accent", weight?: 800, italic?: true, font?: …,
             background?: colour, stroke?, glow?, shadow?}
  shadows:                # presets; soft, hard, none are built in
    lifted: {layers: [{color: "#00000033", blur: 30, x: 0, y: 20}]}
  frame: detailed         # default device frame: stylized | detailed | imported:apple
  fonts:                  # per family: fallbacks for missing characters
    Inter: {fallback: [Noto Sans JP]}
strips:
- id: phone
  page: {aspect: "1320:2868", count: 5, ids?: [hero, plan, …]}   # ids name export files
  backgrounds: ["#F4F7F0", {kind: linear-gradient, angle: 160, stops: [["#F4F7F0", 0], ["#DCEBD5", 1]]}]
  elements: [ … ]          # docs_get {topic: "elements"}
  variants:
    tablet-landscape:
      content_scale: 0.92  # scales device content (default 1)
      overrides:           # element id → fields (only values, never null)
        phone-plan: {at: {x: 500, y: 1683}, width: 600}
      # (variants are added with strip_variant)
targets:
- id: ios-iphone-69        # docs_get {topic: "targets"}
  strip: phone
  size: [1320, 2868]
  screenshots: ios-phone
  gallery: appstore-iphone
  store: {apple: {display_type: APP_IPHONE_67}}
store:                      # store mapping (read-only over MCP: store_config_get)
  apple: {account: team-a, app_id: "1234567890", bundle_id: com.example.app}
  # apple.upload_flow: auto (default: screenshot sets; Duo/header/search via the App Asset Library) | asset_library | screenshot_sets
  google: {account: play, package_name: com.example.app}
  locales: {apple: {en: en-GB}}   # language → store locale; others use built-in defaults
import:                     # screenshot import mappings (screenshots_import save_mapping)
  fastlane:
    devices: {"iPhone 16 Pro Max": ios-phone}
    locales: {de-DE: de}
    keys: {01_home: home}
```

Translations: `i18n/<lang>.yaml` is a flat map `key: text` (keys may contain
dots: `hero.title: Plan your garden **in minutes**`).

Folders: `screenshots/<lang>/[<variant>/]<key>.png` (device elements show the
screenshot of their `screenshot` key; the variant folder comes from the
target's `screenshots`), `assets/` (images for image elements and image
fills), `fonts/` (TTF/OTF; family names usable in styles).

Tools per section: languages → language_add / language_remove (default
language and fallbacks: apply_ops set_languages), styles → styles_update,
strips → strip_add / strip_duplicate / strip_rename / strip_remove /
strip_update, elements → element_*, variants →
strip_variant / element_update {scope: variant}, targets → target_add /
target_update, import → screenshots_import.
