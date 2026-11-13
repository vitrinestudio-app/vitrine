# Store rules and specs (checked November 2026 – re-verify before relying on sizes)

Vitrine's target presets carry the exact pixel sizes; prefer them over this list.

## Apple App Store

- Up to 10 screenshots per set and language; up to 3 app previews (15–30 s, autoplay muted).
  1–3 screenshots appear in search when no preview exists (orientation-dependent).
- iPhone: a 6.9″ set (1320 × 2868; 1290 × 2796 also accepted) covers current phones; smaller
  sizes are scaled. iPad: 13″ (2064 × 2752 or 2048 × 2732). Mac: 16:10 (2880 × 1800 …).
  iPhone Duo outer 1398 × 2034, inner 2007 × 2853 (optional until April 2027).
- Creative assets (iOS 27): product page header and search results visual, from the Asset
  Library; keep art inside the template safe areas; one idea per asset.
- **2.3.3** More than half of each device's screenshots must show the app in use. Others may
  be marketing. Overlays allowed if the core UI isn't obscured. No login/splash-only sets.
- **2.3.10** No names, icons or imagery of other platforms or marketplaces (no Android frames,
  no Play badges). iPad screenshots must not show an iPhone frame.
- Avoid in all assets: specific prices or discounts, URLs, ©, unverifiable claims, Apple
  recognitions (Editor's Choice, App of the Day, Apple Design Award), unrelated content,
  anything not suitable for 4+.
- Apple product images (imported bezels) are used as-is: no tilt, crop, mirror, reflection.

## Google Play

- Phone: up to 8 per device type; JPEG or 24-bit PNG without alpha; 320–3840 px; long side
  ≤ 2 × short side. For large-format recommendations: ≥ 4 screenshots at 9:16 (≥ 1080 × 1920)
  or 16:9 (≥ 1920 × 1080). Games: ≥ 3 showing gameplay.
- Tablets/Chromebooks: ≥ 4, 1080–7680 px, 9:16 or 16:9. Keep non-UI text away from edges
  (may be cropped on some surfaces).
- Wear OS: 1:1, ≥ 384 px, UI only – no frame, text, graphics or background.
- Show the real app; prioritise UI in the first three. Taglines only when needed and
  **≤ 20 % of the image**; no small fonts, no competing backgrounds.
- Avoid: "Best", "#1", "Top", "New", "Free", "Discount", "Sale", "Million Downloads", prices,
  rankings, awards, testimonials; CTAs ("Download now", "Install now"…); time-bound copy;
  **device imagery**; store badges; third-party marks; hands/fingers on devices.
- Clean status bar (no carrier, no notifications, full battery/Wi-Fi/signal). Alt text ≤ 140
  characters per image, not starting with "Image of".
- Feature graphic 1024 × 500: focal point centred, keep logos/text out of edge cut-off zones,
  no fine details, vibrant colours (not pure white/black/dark grey), no device imagery,
  branding that extends rather than repeats the icon.

## Practical consequences in Vitrine

- Default: one framed design for both stores. Google's "no device imagery / ≤ 20 % text"
  items are recommendations for promotion eligibility, not publishing requirements, and most
  large apps ignore them. Offer a frameless, low-text Play variant when the user wants to
  maximise Play featuring – and always use Android-looking frames on Play, never Apple ones.
- Count "app in use" pages per target before export; a 5-page set needs ≥ 3.
- Run `validate` per target and language; treat overflow and contrast warnings as errors for
  templates.
