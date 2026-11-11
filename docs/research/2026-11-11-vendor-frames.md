# Vendor device frames and mockup packages: sources, formats, terms

Research 2026-11-11 (web only, nothing downloaded except the Apple licence
PDF text for reading). Question: which vendors offer device frame / mockup
image packages, in which format, and under what terms, with a view to
user-built "own frame packages" (PNG/SVG + `frame.yaml`). We never
redistribute vendor graphics. This is not legal advice; every statement
below is a paraphrase or quote of the cited page, all checked 2026-11-11.

## Summary

| Source | What / format | Store marketing use | Redistribute / embed in software | Attribution |
|---|---|---|---|---|
| Apple Product Bezels | PNG + Photoshop per model, `.dmg` | Yes, mock-ups/screenshots of apps for Apple OSes only; "as is" | No (licence 2B, 2C, 3) | Apple credit lines where legal info is given (marketing guidelines) |
| Google Device Art Generator | Web tool, PNG output, Pixel/Wear/TV frames | Google says *don't* use in Play screenshots / feature graphic | No licence stated for the art | n/a |
| Android Studio device-art-resources (AOSP) | Per-device folders (`back`, `mask`, layout) in AOSP git | Not stated | Licence for the artwork not stated in the tree | n/a |
| Samsung | Official product images on samsungmobilepress.com | Galaxy Store: images overlaid with app screenshots allowed without agreement | Not stated (press images otherwise press-only per forum) | Not stated |
| Meta Quest Hardware Art | Controller/headset art (.fbx, .png, .eps, .ai) | Within apps/marketing per "Art Attribution License 1.0" (third-party summary) | Restricted (e.g. converted GLTF/GLB) | Yes, "Oculus lineart attribution.txt" |
| Design at Meta "Devices" | Redrawn mockups of many brands | Prototyping/mock-ups only; commercial use needs written permission | No; "use in software programs or other products" excluded | Not stated |
| Microsoft Surface Duo | Transparent PNGs + Figma kit (archived docs) | Presenting screenshots | Not stated | Not stated |
| MockUPhone | Web generator | Output CC BY 3.0 | Output only; device art from many third parties | "Created with MockUPhone" suggested |
| Figma Community | Free files CC BY 4.0 by default | Yes (CC BY), but vendor trademarks/designs not cleared | Yes under CC BY, per file | Required (CC BY) |

## Apple Product Bezels (for comparison)

- Design Resources lists Product Bezels for current iPhone, iPad, Mac, Watch,
  TV, Studio Display; format "Photoshop, PNG" (Keynote for the Live Video
  set), each a `.dmg`. Page points to the Marketing Resources and Identity
  Guidelines; no bezel-specific licence on the page.
  https://developer.apple.com/design/resources/ (checked 2026-11-11)
- Licence (LYL142, 06/21/2023), 2A: use "solely for creating mock-ups of user
  interfaces designed for use in software products that run only on Apple's
  macOS, iOS, watchOS, tvOS, and/or visionOS", including showing them "in
  screen shots, images or other depictions of such Mock-Ups".
  2B: "You may not embed the Apple Design Resources in any software programs
  or other products"; no redistribution, sublicensing, network use.
  2C: Template Content may not be "extracted, copied, modified, distributed,
  or repackaged". 2D: no derivative works. 3: no standalone distribution.
  https://developer.apple.com/support/downloads/terms/apple-design-resources/Apple-Design-Resources-License-20230621-English.pdf (checked 2026-11-11)
- Marketing guidelines: use product images "as is" (no added reflections,
  shadows, cropping, tilting, obstructing, flipping); copy may sit beside the
  product, "not on top of them"; no 3D renderings or illustrations of Apple
  products; don't show Apple products with competing products; min. device
  size 200 px on screen; status bar full signal/Wi-Fi/battery.
  https://developer.apple.com/app-store/marketing/guidelines/ (checked 2026-11-11)
- Consequence for us: matches the current design (user imports, app-wide
  store, never bundled). Parametric "stylized"/"detailed" iPhone frames are
  our own drawings; the guideline bans *illustrations of Apple products* in
  marketing, so users who want to be strictly guideline-compliant should use
  the imported bezels for Apple devices.

## Google / Android

- Device Art Generator (web tool, PNG screenshot in, framed PNG out):
  "Do *not* use graphics created here in your feature image or screenshots
  for your Google Play store listing, using the screenshot alone without
  framing is recommended." Intended for "your website or in other
  promotional materials". No licence for the frame art is given.
  https://developer.android.com/distribute/marketing-tools/device-art-generator (checked 2026-11-11)
- Play graphic guidelines: avoid device imagery in screenshots (it "can
  become obsolete quickly or alienate some users"); Wear OS screenshots must
  "Not position the screenshots within device frames"; device imagery also
  listed as to avoid in the feature graphic.
  https://support.google.com/googleplay/android-developer/answer/9866151 (checked 2026-11-11)
- Android Studio's frame artwork lives in AOSP
  `platform/tools/adt/idea/artwork/resources/device-art-resources` (Pixel
  through Pixel 8 Pro, Pixel Fold/Tablet, Wear OS, TV, Automotive, generic
  phone/tablet; snapshot of 2024-05-15). No LICENSE/NOTICE in that tree's
  root; the artwork's licence is **not confirmed** (do not assume Apache 2.0).
  https://android.googlesource.com/platform/tools/adt/idea/+/7be882f65e4540436c97b64b0249dc52aa9825f8/artwork/resources/device-art-resources (checked 2026-11-11)
- Pixel marketing imagery: no public Pixel device-image kit for developers
  found. Android brand guidelines: all creatives referencing Android/Google
  trademarks need approval via the brand request form; Partner Marketing Hub
  has the Play badge.
  https://developer.android.com/distribute/marketing-tools/brand-guidelines (checked 2026-11-11)

## Samsung

- No official Samsung device-frame kit for developers found.
- Galaxy Store IP checklist: "Official Samsung mobile device and other
  product images available at samsungmobilepress.com" and "those images
  overlaid with screenshots of your apps are allowed without agreements".
  Brand identifiers and designs of other companies need licensing
  agreements; no implied Samsung endorsement. The checklist says it is not
  part of the App Distribution Guide.
  https://developer.samsung.com/galaxy-store/ip-infringement.html (checked 2026-11-11)
- A Samsung forum answer says newsroom images are for press publications
  only; i.e. outside Galaxy Store the samsungmobilepress terms apply.
  https://forum.developer.samsung.com/t/using-an-official-samsung-image-as-a-store-asset/22012 (checked 2026-11-11, search snippet only)
- "Good Lock" is a Samsung customisation app, not a source of frames (no
  relevant result found).

## Meta Quest

- Store asset guidelines (updated 2026-05-11): screenshots "must reflect
  actual in-experience content", "No banners, badges, titles, or marketing
  text"; trailers may show only Meta Quest hardware. Nothing on frames.
  https://developers.meta.com/horizon/resources/asset-guidelines/ (checked 2026-11-11)
- Quest hardware/controller art package (.fbx, .png, .eps, .ai) with
  "Oculus lineart attribution.txt"; licence named "Art Attribution License
  1.0" in a community post (use in apps allowed, redistribution in converted
  formats restricted). Not verified against the licence text itself.
  https://developers.meta.com/horizon/downloads/package/oculus-controller-art/ ,
  https://communityforums.atmeta.com/discussions/dev-general/considering-a-more-open-license-for-meta-quest-hardware-art/1293178 (checked 2026-11-11)
- Brand guidelines: Meta photos/images "may only be used as provided in
  these guidelines or with permission from Meta".
  https://developers.meta.com/horizon/resources/publish-brand-guidelines/ (checked 2026-11-11)

## Microsoft

- Only official frames: Surface Duo / Duo 2 transparent PNGs (sized for a
  full-resolution screenshot) plus a Figma kit; docs archived (2022-10-14).
  No licence text on the page. No kit for current Surface/Windows devices
  found.
  https://learn.microsoft.com/en-us/dual-screen/design/device-frame (checked 2026-11-11)

## Community / third-party sources

- **Design at Meta "Devices"** (formerly facebook.design/devices): redrawn
  mockups of Apple, Google, Samsung, Microsoft, … devices. Page: don't edit
  them or pass them off as your own. Licence v1.4 (2022): use "solely for
  creating prototypes, including mock-ups and displaying such mock-ups"; no
  commercial use without written permission, "including use in software
  programs or other products"; device images may be subject to the owners'
  copyright/trademark, must follow the manufacturer's guidelines and not imply
  endorsement. → Store marketing images are commercial use: unclear/no.
  https://www.meta.com/design-at-meta/tools/devices/ ,
  https://www.meta.com/design-at-meta/license/ (checked 2026-11-11)
- **MockUPhone** (Oursky/Skymakers, web generator): "All art generated by
  MockUPhone is licensed under a Creative Commons Attribution 3.0 Unported
  License"; device art credited to many third parties (Pixeden,
  GraphicBurger, Medialoot, Dribbble artists, Figma creators); their source
  licences not stated. Vendor trademark/design rights are not cleared by CC.
  https://mockuphone.com/attribution (checked 2026-11-11)
- **Figma Community**: free files default to CC BY 4.0 ("shared and adapted
  for any purpose, including commercial", credit required); creators may add
  other licences; paid files under the Community Paid Resource License (no
  redistribution). Check each file; CC BY covers the drawer's copyright only.
  https://help.figma.com/hc/en-us/articles/360042296374-Figma-Community-copyright-and-licensing (checked 2026-11-11)
- Commercial mockup shops (Mockuuups, LS Graphics, Yellow Images, …):
  subscriptions, usually "no resale/redistribution of the mockups".
  https://mockuuups.studio/license/ (checked 2026-11-11, search snippet)

## Implications for Vitrine Studio

- **Ship nothing vendor-made.** No bezel PNGs, no AOSP device-art files, no
  Design at Meta mockups, no MockUPhone/Figma device art in the repo or the
  installer, not even as "examples" or test fixtures. Apple's licence (2B)
  explicitly forbids embedding in software; the others either forbid it or
  leave it unlicensed. Our parametric frames stay the only built-ins.
- **Don't offer downloads or a frame "store"** that fetches vendor art from
  third-party URLs on the user's behalf; that is distribution by another
  route. Pointing to the official page (help link) is fine.
- **Plausible user-built frame packs** (the user downloads, we import):
  - Apple Product Bezels (already supported; Apple OS apps only).
  - Samsung images from samsungmobilepress.com, for Galaxy Store listings
    (explicitly allowed there with app screenshots overlaid).
  - Meta Quest hardware art (with its attribution file) for Quest-related
    marketing, if its licence fits the use.
  - Surface Duo PNGs (legacy).
  - Figma Community CC BY device kits or the user's own drawings; the user
    keeps the attribution in the pack's `frame.yaml` (`source`, `license`,
    `attribution` fields) and we can surface it in exports/notes.
  - Android Studio device-art folders from the user's own SDK install: technically
    straightforward, but licence unclear and Google advises against frames in
    Play screenshots; mention, don't promote.
- **Guidance by store:** Google Play recommends *no* frames in phone
  screenshots and forbids them for Wear OS; our Play targets should default
  to "no frame" (or warn in `validate`, as a store rule in data, not code).
  Apple marketing guidelines want unaltered Apple bezels (no shadow,
  tilting, text overlap) — our effects on imported Apple bezels could be an
  info-level validation note.
- **Suggested in-app text** (import dialog / docs):
  > "Frame packages are imported from files you obtained yourself. Vitrine
  > Studio does not include or distribute device images from Apple, Google,
  > Samsung or other manufacturers. Each vendor's terms govern how you may
  > use their images — for example, Apple's bezels may only be used to show
  > apps for Apple platforms, and Google recommends unframed screenshots on
  > Google Play. Keep the source and licence of each package in its
  > frame.yaml, and check the vendor's guidelines before publishing."
