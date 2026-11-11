---
title: Getting started
description: "Install Vitrine Studio, create a first project and export your first screenshots."
weight: 10
icon: book
---

## Install

Installers for macOS, Windows and Linux are not published yet (see
[Download](/download/)). Until then, build from source with
[rustup](https://rustup.rs) and the
[Tauri 2 prerequisites](https://v2.tauri.app/start/prerequisites/):

```sh
git clone https://github.com/vitrinestudio-app/vitrine.git && cd vitrine
cargo run -p vitrine-gui --release        # the desktop app
cargo install --path crates/cli            # the `vitrine` command line tool
```

## Your first project in the app

1. Start Vitrine Studio. The start screen offers **New project**, **Open** and
   your recent projects.
2. **New project** → pick a template card (`Phone`: three pages with headline
   and device for iPhone 6.9″, Google Play and a blog banner; `Minimal`: one
   page) and your languages.
3. Drop your app screenshots onto the devices – or import a whole folder
   (*Import screenshots*, also fastlane output).
4. Double-click a headline to type it. Every change is saved immediately;
   there is no "Save".
5. **Export** (⌘E) writes the images into `export/` – or upload them with
   *Upload to stores…*.

On the first project the app shows a one-screen guide "How Vitrine Studio is
organised"; the Help menu has it again, plus *Keyboard & mouse* (or press `?`).

## Your first project on the command line

```sh
vitrine new my-app --lang en --lang de     # a phone project from a template
# put screenshots into my-app/screenshots/en/ios-phone/<key>.png (see project.yaml)
vitrine validate --project my-app          # what is missing or does not fit
vitrine export --project my-app            # PNG/JPEG files in my-app/export/<target>/<lang>/
```

Try it on the example project of the repository:
`vitrine export --project tests/fixtures/example`.

## The project folder

| Path | Content |
|---|---|
| `project.yaml` | canvases, styles, targets, variants, store mapping |
| `i18n/<lang>.yaml` | the texts per language |
| `screenshots/<lang>/<variant>/<key>.png` | your app screenshots |
| `assets/`, `fonts/`, `frames/` | images, project fonts, own frame packs |
| `export/`, `.cache/` | output and caches – keep them out of Git |

External edits – a text editor, `git checkout`, an AI assistant – are picked
up live while the project is open.
