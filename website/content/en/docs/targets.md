---
title: Targets & store formats
description: "Add store formats from presets, adapt one canvas to other sizes with anchors, grouping and variants, create banners and App Store creatives, and set up watch targets."
weight: 60
icon: devices
---

A **target** is one output: which canvas, at which pixel size, with which
frames, gaps and store slot. Every target is rendered in every language of the
project (or the languages you pick for it).

## Adding a target

Open **Canvases & targets…** (☰ menu, or the button next to the Target picker)
→ a canvas → **Add target**. Pick a preset – the filter box helps – and **Add
with this size** if a preset offers several sizes. Each preset carries the
store's exact sizes, the store slot, a matching device and gallery profile.
Badges show whether the store **requires** the set (*required*, *conditional*,
*optional*) and how well the data is verified.

{{< placeholder "Canvases & targets: the Add target list with presets, badges and sizes." >}}

### Presets

| Preset | Size (px, default first) | Notes |
|---|---|---|
| `ios-iphone-69` | 1320×2868, 1290×2796 | iPhone 6.9″/6.7″ – uploaded as Apple's 6.7″ set |
| `ios-iphone-65` | 1284×2778, 1242×2688 | iPhone 6.5″ |
| `ios-iphone-63` | 1206×2622, 1179×2556 | iPhone 6.3″ – **the required iPhone set** |
| `ios-iphone-61` | 1170×2532, 1125×2436, 1080×2340 | iPhone 6.1″/5.8″ |
| `ios-iphone-55`, `ios-iphone-47` | 1242×2208, 750×1334 | home-button iPhones |
| `ios-iphone-duo-inner`, `-outer` | 2007×2853, 1398×2034 | iPhone Duo, App Asset Library only |
| `ios-ipad-13` | 2064×2752, 2048×2732 | iPad 13″ – required if your app runs on iPad |
| `ios-ipad-129`, `ios-ipad-11`, `ios-ipad-105` | | older/smaller iPads |
| `mac` | 2880×1800, 2560×1600, 1440×900, 1280×800 | Mac App Store, 16:10 only |
| `watch-ultra`, `watch-series`, `watch-series-7`, `-4`, `-3` | 422×514 … 312×390 | screenshot only |
| `play-phone` | 1080×1920, 1440×2560 | Google Play phone 9:16 |
| `play-phone-2x1` | 1080×2160, 1440×2880 | Google Play phone 1:2 |
| `play-tablet-7`, `play-tablet-10`, `play-chromebook` | | Google Play large screens |
| `play-wear` | 1024×1024, 384×384 | Wear OS, screenshot only |
| `play-feature-graphic` | 1024×500 | banner, **required** to publish on Google Play |
| `ios-header` | 3840×1646 | App Store product page header (21:9) |
| `ios-search-results` | 3840×2560 … 1920×1280 | App Store search results (3:2) |
| `ios-creative-universal` | 5244×2950 | App Store universal creative (16:9) |
| `banner-blog-header`, `banner-open-graph`, `banner-web-hero` | 2400×1000, 1200×630, 2880×1200 | for your website and social media |

Every preset has a landscape size too where the store allows it (*Swap width
and height*). For Apple you only *need* one iPhone set (6.3″) and, for iPad
apps, one iPad set (13″); App Store Connect scales the others down. Google
Play wants 2–8 images per device type, and at least four of 1080 px or more
to be eligible for recommendations.

## The target form

| Field | Meaning |
|---|---|
| **Id** | the target's name, also the export folder name |
| **Kind** | *Pages* (one image per page or page group) or *Banner* (one image from a region) |
| **Size (px)** | the export size of one image |
| **Pages per image** | `2` puts two portrait pages side by side in one landscape image |
| **Variant** | which variant of the canvas this target uses; *none* = base layout |
| **Gallery profile** | the store's gap and corner radius, used for the canvas gaps and the gallery preview |
| **Screenshots** | the screenshot folder (*screenshot variant*) to use, e.g. `ios-phone`, `android-phone`, `ios-tablet` |
| **Device swap** | replace device models for this target, e.g. phone with island → Android phone |
| **Languages** | *all*, or only some |
| **Format** | PNG, or JPEG with a *quality* |
| **Store slot** | where the images are uploaded: App Store display type or Google Play image type |
| **Screenshot only** | export each page's screenshot alone (watches) |
| **Safe area** | insets in % of the image, shown dashed on the canvas |

Under the form, the target's store rules are checked (sizes, counts, formats).

## One canvas, many formats

You rarely need a second canvas for another device size. Instead:

### Different aspect ratios: anchors

A phone canvas designed for iPhone (page aspect 1320:2868) also renders for
Google Play (1080:2160): the page gets a little shorter. Each element keeps its
distance to the edge named by its **Anchor** – headlines at the top stay at the
top (*Top*), phones at the bottom stay at the bottom (*Bottom*). If a target's
page is much taller or shorter than the canvas page, validation suggests a
variant (`variant_suggested`).

### Landscape from portrait pages: grouping

The iPad landscape set of the example project uses the **phone** canvas:
`Pages per image: 2` puts pages 1+2, 3+4 and 5+6 into three landscape images.
The device swap draws iPads instead of iPhones, and a variant moves the phones
a little. Exported file names show both pages: `01_plan+beds.png`.

{{< shot src="images/ipad-gallery-de.jpg" alt="iPad landscape gallery of the example project: each image shows two phone pages side by side." caption="`ios-ipad-13` landscape with `Pages per image: 2`, variant `tablet-landscape` and a device swap to iPad." >}}

### Rearranging: variants

When a format needs elements elsewhere, create a **variant** for it (Variant
picker → **New variant…**), tick the targets under **Shown by**, and move
things while the variant is shown. Only the differences are stored. See
[Variants](../editor/#variants).

## Banners

A **banner** target renders **one** image from a region of a canvas instead of
one image per page – the Google Play feature graphic, App Store creatives, a
blog header, an Open Graph image.

### Banner from this canvas

The quickest way: **Canvases & targets** → a canvas → **Banner from this
canvas**. It offers:

- **Google Play feature graphic** (1024×500)
- **App Store product page header** (21:9)
- **App Store search results** (3:2)
- **App Store universal creative (16:9)**

For each it proposes as many pages as fit the banner's aspect (about 4 phone
pages for the feature graphic, 5 for the header, 3 for search results) –
**From page** moves the range – and shows *already: …* if the canvas has one.
**Add** creates the target: seamless (no gaps), filling the image (*cover*),
with the same screenshots, device swap and variant as the canvas's screenshot
target of that store – so the Play banner shows the Android phones.

{{< shot src="images/blog-header-de.jpg" alt="A wide banner with the German headline and phones from the example project." caption="A banner target (`blog-header`, German) cut from the phone canvas." >}}

### Banner settings

- **Region:** *whole canvas*, a *page range* (*from page* … *to*), or a *free
  rectangle* (page, x, y, width, height in lu).
- **Fit:** *cover (crop)* or *contain (extend background)*.
- **Gaps:** *none* (seamless) or *as in the store*.

Banners often want their own arrangement: give the banner target its own
variant (the example uses `banner`), e.g. to hide the subtitles.

### Google Play feature graphic

Required to publish on Google Play: 1024×500, JPEG or 24-bit PNG without
transparency. Vitrine Studio exports are always opaque. Google's guidance:
keep the key content (logo, name, slogan) away from the edges, no "Best" or
"#1", no Play badges, and avoid device imagery – use a variant without the
devices if you follow it strictly. It is uploaded with the screenshots, one per
language.

### App Store creatives (header and search results)

Optional images for the App Store on iOS 27 and later, one per localization:
the **product page header** (21:9, 3840×1646), the **search results** asset
(3:2, 1920×1280 to 3840×2560), or one **universal** creative (16:9 PNG,
5244×2950) that serves both. Apple's rules: a short localized phrase that
enhances the visual, no prices, URLs, ©, unverifiable awards, other platforms
or Apple awards, suitable for 4+. They are uploaded only through the App Asset
Library.

**Safe area:** Apple defines it only in its design templates, without numbers.
If you know it, set **Safe area** (top/right/bottom/left in %) on the target:
the canvas shows it dashed and validation warns `text_outside_safe_area`.

## Screenshot-only targets

**Screenshot only** (*Export each page's screenshot alone*) is on by default
for all watch presets and `play-wear`. Each page exports the screenshot of its
**largest device** alone, filling the image – no frame, background, text,
shadow or rotation – because the stores want the bare watch UI (Apple draws its
own watch mask). Use the target's screenshot folder (`watch`, `wear`) so the
right captures are picked. A page without a device stays empty
(`screenshot_only_empty`).

For Apple Watch, one size is enough for all localizations; choose the one you
have captures for.

## Store sizes Vitrine Studio does not offer

Some sizes listed on Apple's help page (1260×2736, 1512×2316) are rejected by
the App Store Connect uploader, so presets don't offer them. Store-specific
values are data files with sources and dates in the repository
(`crates/core/data/targets.yaml`).
