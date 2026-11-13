---
title: Getting started
description: "Create a project from a template, find your way around the window, add your screenshots and headlines, and export your first set."
weight: 30
icon: book
---

This walkthrough takes about ten minutes. You'll need Vitrine Studio
([Install](../install/)) and a few screenshots of your app – three portrait
iPhone screenshots are perfect.

## 1. Create a project

Start Vitrine Studio. The start screen offers **New project**, **Open
project…** and your **Recent projects** (you can also drop a project folder
onto the window).

1. Click **New project**.
2. **Name:** your app's name, e.g. `Fernwood`.
3. **Location:** the parent folder – the project becomes a new folder in it.
4. **Template:** keep **Phone** – three pages with a headline and a phone
   each, rendered for iPhone 6.9″, Google Play and a blog header. (*Minimal*
   is a single iPhone page; your own templates appear here too.)
5. **Languages:** comma-separated, the first one is the default language,
   e.g. `en, de`.
6. **Create.**

{{< shot src="images/new-project.jpg" alt="The New project dialog: name, location and languages, and the Template gallery tab with search, filters and template cards such as Fruit Pop, Hero Landing, Paper Calm and the selected Pop-Art Panels." caption="New project from the online **Template gallery** – here *Pop-Art Panels*, in English, German and French." >}}

The first time you open a project, a one-screen overview *How Vitrine Studio
is organised* appears. It's worth a look; it's always in the Help menu.

## 2. A tour of the window

{{< shot src="images/editor.jpg" alt="The Vitrine Studio editor: the Layers tab on the left, a canvas with six pages in the middle, a selected phone with its settings in the inspector on the right." caption="The editor with a project made from the *Synthwave Sunset* template: menu and pickers in the top bar, views and zoom in the bottom bar." >}}

- **Top bar:** the ☰ menu (also opens from the app icon and the project
  name); the pickers **Canvas**, **Target**, **Language**
  and **Variant** decide what you see and edit; the targets button next to
  the target picker opens *Canvases & targets*; **Add** puts a new element on
  the visible page; on the right the assistant indicator, **Export**, the
  validation badge (**OK** or the number of problems) and undo/redo.
- **Left sidebar:** the tabs **Canvases**, **Layers**, **Assets** and
  **Translations**.
- **Centre:** the view. The switch in the bottom bar picks **Canvas** (the
  whole row of pages with the hatched gaps), **Pages** (each output image),
  **Store gallery** (as the store shows it) or **All languages** (every
  language side by side).
- **Right:** the **Inspector** – the selected element's settings, or the
  canvas settings (page format, page count, backgrounds, variants) when
  nothing is selected.
- **Bottom bar:** the view switch, the selection, the **?** button (keyboard &
  mouse help) and the zoom controls (−, %, +, *Fit to window*).

## 3. Add your screenshots

Each phone shows the screenshot with its **key** – in the Phone template
`welcome`, `features` and `start`. The quickest way: **drag a screenshot file
onto a phone** on the canvas. It becomes that phone's screenshot for the
current language.

Other ways:

- In the **Assets** tab, **Import…** imports single files; **Import
  folder…** (also in the ☰ menu: **Import screenshots…**) brings in a whole
  folder – including fastlane output. See [Screenshots](../screenshots/).
- Or copy files into the project folder yourself:
  `screenshots/en/welcome.png`, `screenshots/en/features.png`, … The app
  picks them up immediately.

Languages without their own screenshots use the default language's, so you can
start with English or your own language only.

## 4. Write your headlines

**Double-click a headline** on the canvas and type. <kbd>Enter</kbd> applies,
<kbd>Shift</kbd>+<kbd>Enter</kbd> makes a line break, <kbd>Esc</kbd> cancels.
Select a word and use the toolbar for **bold**, *italic* or a span style (the
accent colour).

You are editing the text of the **current language**. Switch the
**Language** picker to `de` and double-click again to write the German text –
or open the **Translations** tab to see all languages of every text in one
table.

Texts shrink automatically to fit their box. If a translation is too long even
at the minimum size, the validation badge turns red and the text is marked –
see [Fitting long texts](../languages/#fitting-long-texts).

## 5. Arrange

- Drag elements to move them – they snap to page edges, centres and other
  elements. Handles resize and rotate.
- A device can sit across two pages; drag it over the border.
- Drag on empty space to pan. To zoom, scroll while holding <kbd>⌘</kbd>
  (macOS) or <kbd>Ctrl</kbd> (Windows, Linux), or pinch on the trackpad;
  <kbd>⌘</kbd><kbd>0</kbd> fits the view to the window.
- Made a mistake? <kbd>⌘</kbd><kbd>Z</kbd>. Every change is one undo step and
  saved at once.

On Windows and Linux, read <kbd>Ctrl</kbd> wherever this guide says
<kbd>⌘</kbd>. The [editor page](../editor/) has everything else; the **?**
button at the bottom right shows all shortcuts.

## 6. Check all formats

Switch the **Target** picker to **play-phone**: the same layout as a Google
Play image (1:2 instead of about 1:2.17) with an Android frame. Switch the
view to **Store gallery** to see the set as the store will show it, or to
**All languages** to compare the translations.

Click the validation badge for a list of what's missing or doesn't fit –
*Show on the canvas* jumps to the problem.

## 7. Export

Press <kbd>⌘</kbd><kbd>E</kbd> (or **Export** in the top bar). The dialog
shows a grid of **targets × languages** – everything is ticked. Click
**Export**.

{{< shot src="images/export-dialog.jpg" alt="The export dialog: a grid of targets × languages, output folder, layout and options." caption="The export dialog of a project with six targets and three languages." >}}

The images land in the project's `export/` folder:

```text
export/
  index.html                  ← overview of every image
  ios-iphone-69/en/01_welcome.png
  ios-iphone-69/en/02_features.png
  ios-iphone-69/en/03_start.png
  ios-iphone-69/en/_gallery.png   ← preview of the store gallery
  ios-iphone-69/de/…
  play-phone/en/…
  blog-header/en/banner.jpg
```

**Show in Finder** / **Show in Explorer** opens the folder. Exports are
incremental: next time only changed images are rendered. Details:
[Export](../export/).

## The same on the command line

```sh
vitrine new my-app --lang en --lang de         # Phone template, en + de
cp ~/Desktop/shots/welcome.png my-app/screenshots/en/
cp ~/Desktop/shots/features.png my-app/screenshots/en/
cp ~/Desktop/shots/start.png my-app/screenshots/en/
vitrine validate --project my-app              # what's missing or doesn't fit
vitrine export --project my-app                # → my-app/export/
```

Edit the texts directly in `my-app/i18n/en.yaml` and `de.yaml` – or open the
project (the `my-app` folder) in the desktop app and write them there as
described above. You can mix both: the app reloads changed files at once. See the [CLI reference](../cli/).

## Next steps

- Add more formats – iPad, Mac, the Play feature graphic: [Targets & store formats](../targets/)
- Translate with translators or Claude: [Languages & translation](../languages/)
- Real Apple device frames: [Devices & frames](../devices-frames/)
- Upload directly to the stores: [Store upload](../store-upload/)
