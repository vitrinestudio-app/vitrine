---
title: Vitrine Studio
description: "Open-source desktop app for App Store and Google Play screenshots: design once on a continuous canvas, render for every language, device and store, upload directly – with an MCP server for AI assistants."

hero:
  eyebrow: "Open source · macOS, Windows, Linux"
  title:
    - "Store screenshots,"
    - "designed once."
    - "Rendered everywhere."
  free:
    big: "100 % free"
    rest: "– yes, really."
    note: "No subscription, no account, no watermark, no paid tier. Free and open source."
  lead: "Vitrine Studio is a desktop app for App Store and Google Play screenshots. Design one continuous canvas – backgrounds and devices run across the pages like in the store gallery – and render it for every language, device size and store. Then upload it, or let your AI assistant do the busywork."
  ctaGithub: "View on GitHub"
  note: "Pre-alpha: installers are not published yet. The source is on GitHub."
  pages:
    - ["Plan your", "garden"]
    - ["Bed by", "bed"]
    - ["At your", "desk"]
    - ["or out in", "the beds"]
    - ["From", "seed"]
    - ["to", "harvest"]
  notesLabel: "What the animation shows"
  notes:
    - "One background through every page"
    - "Tilt and overlap devices freely"
    - "One device across two pages"
    - "Mix phones, laptops and tablets"
    - "Gaps like in the store – content in a gap is left out"
  facts:
    - "Open source, GPL-3.0-or-later"
    - "No account, no cloud"
    - "Plain YAML project folders"
    - "Made with Rust + Tauri"

idea:
  eyebrow: "How it works"
  title: "Design a canvas, not a pile of images"
  lead: "Instead of composing every screenshot by hand, you lay out one strip of pages. Vitrine Studio does the rest – for every target you add."
  steps:
    - title: "Design once"
      text: "One canvas of store pages. A phone that crosses two screenshots exists exactly **once**, backgrounds flow across all pages, and the gaps match the store gallery so every transition lines up."
    - title: "Render every combination"
      text: "Languages × device sizes × stores, from one layout. Anchors, page grouping and variants adapt it to other aspect ratios – content is never maintained twice."
    - title: "Export or upload"
      text: "PNG/JPEG files in your own or the fastlane layout, colour-managed and incremental – or straight to App Store Connect and Google Play, always as **plan → confirm → upload**."

matrix:
  eyebrow: "One layout, many formats"
  title: "Every language. Every device. Every store."
  lead: "Add a language or a target and the whole set appears – translated, re-laid-out and checked for overflowing text."
  alt: "A grid of rendered screenshots: four languages times four device formats, all from one layout."
  points:
    - "**Fallback chains**: `fr-CA` inherits from `fr`, missing screenshots come from the default language"
    - "**Per-language overrides** and opt-in automatic hyphenation for long words"
    - "**Two portrait pages → one landscape image** for iPad sets"
    - "**Validation** per target × language: missing screenshots, overflowing text, store rules"
  countLabel: "images from one layout – and counting"
  devices:
    - {name: "iPhone 6.9″", shape: "phone"}
    - {name: "Android", shape: "android"}
    - {name: "iPad 13″", shape: "landscape"}
    - {name: "Mac", shape: "mac"}
  langs:
    - {code: "EN", headline: "Plan your garden", headline2: "Know the partners"}
    - {code: "DE", headline: "Plane deinen Garten", headline2: "Finde Partner"}
    - {code: "FR", headline: "Planifiez votre jardin", headline2: "Trouvez les alliés"}
    - {code: "ES", headline: "Planifica tu jardín", headline2: "Conoce los aliados"}

editor:
  eyebrow: "The editor"
  title: "A real desktop editor – the preview *is* the export"
  lead: "Every pixel you see is rendered by the same Rust pipeline that writes your files. Every change is saved immediately as plain YAML."
  alt: "The Vitrine Studio editor in dark mode: canvases and layers on the left, six pages of a garden-planner app on one canvas in the middle, the canvas settings in the inspector on the right."
  alt2: "A selected headline on the canvas with its text settings in the inspector."
  points:
    - title: "Direct manipulation."
      text: "Move, resize and rotate with snapping, edit texts inline, drag & drop screenshots onto devices."
    - title: "Four views."
      text: "The continuous canvas with hatched gaps, single pages, the store gallery and all languages side by side."
    - title: "Undo everything."
      text: "One history for your edits and your AI assistant's – each step labelled."

features:
  eyebrow: "Features"
  title: "Everything between “screenshots taken” and “live in the store”"
  lead: "Built for app developers who ship in many languages and on more than one platform."
  items:
    - icon: canvas
      title: "Continuous canvas"
      text: "Backgrounds, bands and devices run across page gaps. Content inside a gap is left out of the export – exactly like the store shows it."
    - icon: devices
      title: "Every device class"
      text: "iPhone (including the foldable iPhone Duo), iPad, Mac, Android phones and tablets, Apple Watch and Wear OS, browser windows and notebooks."
    - icon: frame
      title: "Device frames, three ways"
      text: "Built-in frames drawn parametrically, Apple Product Bezels you download and import yourself, or your own frame packs (PNG/SVG + `frame.yaml`)."
    - icon: store
      title: "Store formats from presets"
      text: "App Store and Google Play screenshot sets, App Store header and search-results creatives, the Play feature graphic, web slides and banners – cut from the same canvas."
    - icon: languages
      title: "Languages & translators"
      text: "Fallback chains, per-language overrides, automatic hyphenation, and CSV/XLIFF exchange that keeps your markup safe in translation tools."
    - icon: formats
      title: "One layout, many formats"
      text: "Vertical anchors, page grouping and variants adapt one design to other aspect ratios and to landscape sets."
    - icon: folder
      title: "Plain folders, Git-friendly"
      text: "A project is a folder of YAML files and images. No account, no cloud, no lock-in – diff it, branch it, review it."
    - icon: palette
      title: "Colour-managed output"
      text: "Display P3 screenshots are converted to sRGB once; exports are opaque, incremental and come with an `index.html` overview."
    - icon: fonts
      title: "Bundled font families"
      text: "Inter, Montserrat, Playfair Display, Lora, Bebas Neue and more – all SIL OFL, always available, no font files needed."
    - icon: terminal
      title: "CLI for CI"
      text: "`vitrine validate`, `export` and `upload` work in scripts and pipelines, with clear exit codes and JSON output."
    - icon: sparkle
      title: "AI assistants via MCP"
      text: "Claude and other MCP clients can design, translate, preview and export – live in your editor, every step undoable."
    - icon: template
      title: "Templates"
      text: "Start from a template, save your own canvases as templates and share them – a community gallery is on its way."

showcase:
  eyebrow: "Made with Vitrine Studio"
  title: "From one YAML file to every store"
  lead: "The example project – a fictional garden planner – rendered as the stores show it. Same layout, same file."
  tablistLabel: "Example outputs"
  note: "All device frames above are drawn by Vitrine Studio itself – no vendor artwork. The app screens inside are generated for the example."
  tabs:
    - label: "iPhone · English"
      image: "iphone-gallery-en.jpg"
      alt: "Six iPhone screenshots of a garden-planner app on one green canvas; a tilted phone crosses the gap between pages three and four."
      caption: "iPhone 6.9″ set: one continuous canvas, a phone crossing pages 3 and 4."
    - label: "iPad · Deutsch"
      image: "ipad-gallery-de.jpg"
      alt: "Three landscape iPad screenshots in German, each built from two phone pages."
      caption: "The same phone pages as an iPad landscape set in German – two pages per image, tablet frames, adjusted positions."
    - label: "iPhone · Français"
      image: "iphone-gallery-fr.jpg"
      alt: "The iPhone screenshot set in French."
      caption: "In French, with French app screenshots where they exist and English ones as fallback."
    - label: "Mac App Store"
      image: "mac-gallery-en.jpg"
      alt: "Three Mac App Store screenshots with a desktop app window on a green background."
      caption: "Mac App Store set (16:10) from its own canvas."
    - label: "Web"
      image: "web-gallery-en.jpg"
      alt: "Two web slides: a browser window and a notebook showing a web app."
      caption: "Web slides: a browser window and a notebook with browser chrome."
    - label: "Blog header"
      image: "blog-header-de.jpg"
      alt: "A wide blog header image cut from the phone canvas without gaps."
      caption: "A blog header cut from the same canvas – without gaps."

ai:
  eyebrow: "AI assistants"
  title: "Your assistant edits what you see"
  lead: "Vitrine Studio has a built-in MCP server. Claude – or any MCP client – can create projects, add devices and texts, translate, look at rendered previews, validate and export. Its changes appear live in the editor."
  points:
    - "**Add to Claude Desktop** with one click in the app settings; Claude Code and other clients with one command"
    - "Every change is **one undo step**, labelled with the assistant's name"
    - "The assistant **sees the result**: `render_preview` returns the rendered PNG"
    - "Runs **locally** on 127.0.0.1 with a token – store uploads still need **your click**"
  more: "Set up an AI assistant"
  chat:
    label: "A chat with an AI assistant that translates missing texts into French and checks page 2."
    client: "Claude Desktop"
    user: "Translate everything missing into French, then check page 2 for overflows."
    tools:
      - {name: "strings_missing", note: "fr · 14 keys"}
      - {name: "strings_set", note: "14 texts"}
      - {name: "render_preview", note: "page 2 · fr"}
      - {name: "validate", note: "fr · ok"}
    reply: "Done – 14 texts translated. Page 2 fits; one headline was hyphenated automatically, have a look."
    toast: "Claude Desktop: Set French texts · Undo"

fastlane:
  eyebrow: "Using fastlane?"
  title: "Fits right into your fastlane lane"
  lead: "Keep snapshot, screengrab, deliver and supply. Vitrine Studio reads your captures and writes exactly the folders deliver and supply upload."
  terminalLabel: "Terminal: importing fastlane screenshots and exporting in the fastlane layout."
  more: "Using Vitrine Studio with fastlane"
  points:
    - "Imports whole `fastlane/` folders – snapshot **and** screengrab output, store locales, `01Home` names"
    - "Skips frameit leftovers (`_framed.png`, `Framefile.json`) with a reason"
    - "Exports the **deliver/supply layout** with store-locale folders, conflict checks and limits"
    - "Optionally Vitrine Studio uploads the screenshots itself – deliver and supply keep uploading binary and metadata"
  terminal:
    - {comment: "1 · bring your captures in (frameit files are ignored)"}
    - {cmd: "vitrine import screenshots fastlane/"}
    - {out: "✓ 48 screenshots · 4 languages · 2 devices"}
    - {comment: "2 · design once in the app, then export for deliver/supply"}
    - {cmd: "vitrine export --layout fastlane --out fastlane"}
    - {out: "✓ screenshots/de-DE/01_ios-iphone-69.png …"}
    - {out: "✓ metadata/android/de-DE/images/phoneScreenshots/ …"}
    - {comment: "3 · upload as before"}
    - {cmd: "fastlane deliver"}

upload:
  eyebrow: "Store upload"
  title: "Straight to App Store Connect and Google Play"
  lead: "No more dragging 60 files into a browser. Vitrine Studio plans the upload, shows you exactly what changes, and only then sends it – from the app, the CLI or your AI assistant."
  steps:
    - title: "Plan"
      text: "Renders the store targets, hashes every image and compares with the last upload – or with the store itself. Unchanged sets are skipped."
    - title: "Confirm"
      text: "You see each set with its action: upload, replace, unchanged, skipped or blocked – and why. Nothing is sent before you confirm."
    - title: "Upload"
      text: "Progress per set, cancel at any time, resume by planning again. Keys stay in your system keychain."
  planLabel: "An upload plan table with targets, languages, store locales and actions."
  planTitle: "Upload plan · Fernwood"
  columns: ["Target", "Lang", "Store", "Action", "Images"]
  rows:
    - {target: "ios-iphone-69", lang: "en", locale: "en-US", action: "Unchanged", kind: "same", images: "6"}
    - {target: "ios-iphone-69", lang: "de", locale: "de-DE", action: "Replace", kind: "replace", images: "6"}
    - {target: "ios-iphone-69", lang: "fr", locale: "fr-FR", action: "Upload", kind: "upload", images: "6"}
    - {target: "ios-ipad-13-landscape", lang: "fr-CA", locale: "fr-CA", action: "Blocked", kind: "blocked", images: "3"}
  planFoot: "1 blocked: missing screenshot"
  confirm: "Confirm & upload"
  note: "App Store Connect (screenshot sets and the App Asset Library for iPhone Duo, header and search-results creatives) and Google Play (screenshots and feature graphic). **Status:** built and tested against fake stores; real-run testing with live accounts is in progress."

templates:
  eyebrow: "Templates"
  title: "Start from a template – or share yours"
  lead: "A template is one or more canvases with their targets, variants, styles, texts and fonts – a coordinated phone, tablet and feature-graphic set in one file."
  cards:
    - {name: "Phone", style: "phone", text: "Three pages with headline and device: iPhone 6.9″, Google Play and a blog banner.", meta: "Built in"}
    - {name: "Minimal", style: "minimal", text: "One page, one device – the quickest way to a first image.", meta: "Built in"}
    - {name: "Your own", style: "own", text: "Save any canvas as a template, reuse it across apps, insert it into a project.", meta: ".vitrine-template"}
  points:
    - "Templates are **CC0** – fonts inside keep their OFL licence"
    - "Safe import: size limits, no executable content, every file checked"
    - "**Community gallery** – browse, filter and download without an account"

oss:
  eyebrow: "Open source"
  title: "Yours to read, build and keep"
  lead: "Vitrine Studio is free software under the GNU GPL v3 or later. Your projects are plain files on your disk – they outlive any app version."
  points:
    - icon: heart
      title: "Free, GPL-3.0-or-later"
      text: "No subscription, no account, no watermark. Read the code, build it, change it."
    - icon: lock
      title: "Local first"
      text: "Projects never leave your computer unless you upload to a store or share a template. Store keys live in the system keychain."
    - icon: git
      title: "Contributions welcome"
      text: "Pull requests need a one-time [contributor licence agreement](https://github.com/vitrinestudio-app/vitrine/blob/main/CLA.md), signed in the pull request."
  yaml: |
    format: vitrine/1
    name: Fernwood
    languages:
      default: en
      available: [en, de, fr, fr-CA]
    canvases:
    - id: phone
      page: {aspect: 1320:2868, count: 6}
      elements:
      - id: title-plan
        kind: text
        page: 0
        text: plan.title
        style: headline
      - id: phone-plan
        kind: device
        model: iphone-island
        screenshot: home
  yamlCaption: "A project is readable YAML – diff it, review it, script it."

faq:
  eyebrow: "FAQ"
  title: "Questions & answers"
  items:
    - q: "Is Vitrine Studio really 100 % free?"
      open: true
      a: "Yes – 100 % free, really. Vitrine Studio is free, open-source software under the GPL-3.0-or-later. There is no subscription, no paid tier, no account and no watermark on your screenshots, and no limit on languages, devices or exports. Using it for commercial apps is fine."
    - q: "When can I download it?"
      a: "Vitrine Studio is in pre-alpha. Signed installers for macOS, Windows and Linux are being prepared; until then you can [build it from source](https://github.com/vitrinestudio-app/vitrine) with Rust."
    - q: "Do I need an account or an internet connection?"
      a: "No. Projects are local folders. The network is only used when you upload to a store or use the template gallery – publishing a template there needs a GitHub login, browsing and downloading do not."
    - q: "Does it come with Apple device frames?"
      a: "Vitrine Studio draws its own device frames parametrically and ships no vendor artwork. You can import Apple's Product Bezels yourself (downloaded from Apple Design Resources) or use your own frame packs."
    - q: "Can I keep my fastlane setup?"
      a: "Yes. Vitrine Studio imports snapshot and screengrab output and exports the exact folder layout deliver and supply upload – Vitrine Studio designs the screenshots in your lane, everything else stays with fastlane."
    - q: "Which AI assistants work with it?"
      a: "Any MCP client. Claude Desktop is set up with one click from the app settings, Claude Code with one command. Assistants can plan store uploads, but executing one needs your confirmation in the app."
    - q: "Which languages are supported?"
      a: "Any language your fonts cover. Automatic hyphenation is bundled for English, German, French, Spanish, Dutch, Italian, Danish, Portuguese, Norwegian and Finnish. Right-to-left layouts are not mirrored yet; for CJK scripts add a font such as Noto Sans to the project."
    - q: "Is my project locked into Vitrine Studio?"
      a: "No. A project is a folder of readable YAML files, your screenshots and images. Exports are ordinary PNG/JPEG files."

cta:
  badge: "100 % free – yes, really"
  title: "Ship better store pages in every language"
  lead: "Free and open source. No subscription, no account, no watermark – design once, render everywhere."
  docs: "Read the docs"
---
