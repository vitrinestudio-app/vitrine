# Targets

A target is one output: which canvas, at which pixel size, for which store
slot. Add them from presets (`catalog_targets` lists them; `target_add
{preset, canvas}`), change them with target_update.

```yaml
- id: ios-ipad-13-landscape
  kind: pages             # pages (default; one image per page or page group) | banner
  canvas: phone            # the canvas it shows
  size: [2752, 2064]      # export size in px of ONE image
  group: 2                # pages per image (2 portrait pages → one landscape image)
  variant: tablet-landscape   # variant of the canvas used for this target
  screenshots: ios-tablet # screenshot variant folder: screenshots/<lang>/ios-tablet/<key>.png
  gallery: appstore-ipad  # gallery profile (gaps, corner radius); or {preset, gap?, radius?}
  device_map: {iphone-island: ipad-modern}   # swap device models for this target
  langs: [en, de]         # languages to export (default: all available)
  format: png             # png (default) | jpeg (+ quality 1..100)
  store: {apple: {display_type: APP_IPAD_PRO_3GEN_129}}   # or {google: {image_type: phoneScreenshots}}
  screenshot_only: true    # pages only: each page's largest device alone, no frame/background/text (watch, Wear OS)
  safe_area: {top: 10, right: 5, bottom: 10, left: 5}   # insets in % of the image; texts outside → text_outside_safe_area
  # banner only:
  region: {pages: [0, 2]}  # or {rect: {page, x, y, width, height}} (default: whole canvas)
  fit: cover               # cover (crop) | contain (extend the background)
  gaps: none               # none | as_store
```

## How sizes work
- The design is in lu: a page is 1000 lu wide, its height follows the canvas
  aspect. The export scales a page (or a group of pages) to `size`.
- When the target's page aspect differs from the canvas's (e.g. a phone canvas
  exported for Google Play 1:2), the page height changes; elements keep their
  distance to the edge named by their `anchor_y`. Large differences are
  reported as `variant_suggested`: add a variant with overrides.
- `group: 2` puts two pages into one image (iPad landscape from a portrait
  phone canvas); combine it with a variant to rearrange elements.

## Gallery and gaps
The store shows images with gaps and rounded corners (`gallery` profile,
`catalog_galleries`). On the canvas the gap lies between pages; content in a
gap is cut from page exports (layout_report flags `in_gap`). render_preview
{view: "gallery"} shows the images as the store would.

## Presets
Each preset has a store, sizes (first = default; `orientation` picks
portrait or landscape), a default device model, screenshot variant, gallery
and store slot. Required sets (e.g. App Store iPhone 6.9″, iPad 13″) are
marked in catalog_targets. `target_add {preset, canvas, size?: "WxH",
orientation?}`.

## Banners
`kind: banner` renders one image of a region of the canvas (blog header, Open
Graph, feature graphic) instead of one image per page.

App Store creative assets (iOS 27+, optional, one per localization) are
banner presets: `ios-header` (21:9, 3840×1646), `ios-search-results` (3:2,
1920×1280 … 3840×2560, default region pages 0–2) and
`ios-creative-universal` (16:9 PNG 5244×2950, serves both). Their slot has a
`placement` (header | search_results | universal); they upload only through
the App Asset Library. Apple's safe area is only in its design templates:
set `safe_area` on the target when you know it.

## Watch and foldables
- Watch presets (`watch-ultra`, `watch-series`, `watch-series-7/-4/-3`) and
  Wear OS (`play-wear`, 1:1) are `screenshot_only`: the export is the page's
  screenshot (variant `watch` / `wear`) without frame or text. Wear OS rules
  forbid frames, texts and backgrounds (`store_decorations` info otherwise).
- iPhone Duo: `ios-iphone-duo-inner` (open, device `iphone-duo`) and
  `ios-iphone-duo-outer` (folded, `iphone-duo-folded`); a device element's
  `fold: open|folded` switches between the two models.
