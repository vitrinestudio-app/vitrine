//! `index.html` of an export folder: an overview of every exported file
//! (not the gallery previews), grouped by language and target in page
//! order, with file name, pixel size, file size and a click-to-enlarge
//! viewer. Self-contained (inline CSS and JS, relative image paths, no
//! external resources), works from `file://`, light/dark by
//! `prefers-color-scheme`.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

use vitrine_core::Project;

use super::manifest::ManifestEntry;
use super::{ExportLayout, FileKind, image_size};

/// File name of the overview page in the export folder.
pub const INDEX: &str = "index.html";

/// One listed file.
struct Item<'a> {
    path: &'a str,
    entry: &'a ManifestEntry,
    width: u32,
    height: u32,
    bytes: u64,
}

/// Builds the page for the files of `manifest` that exist below `out_dir`.
#[allow(clippy::too_many_lines)] // one linear HTML template
pub fn render(
    project: &Project,
    out_dir: &Path,
    layout: ExportLayout,
    manifest: &BTreeMap<String, ManifestEntry>,
) -> String {
    let mut items: Vec<Item<'_>> = Vec::new();
    for (path, e) in manifest {
        if e.kind == Some(FileKind::Gallery) || e.layout != layout {
            continue;
        }
        let abs = out_dir.join(path);
        let Ok(meta) = std::fs::metadata(&abs) else {
            continue;
        };
        let (width, height) = if e.width > 0 && e.height > 0 {
            (e.width, e.height)
        } else {
            image_size(&abs).unwrap_or((0, 0))
        };
        items.push(Item {
            path,
            entry: e,
            width,
            height,
            bytes: meta.len(),
        });
    }

    // Order: project languages, then project targets, then image number.
    let lang_rank = |l: &str| {
        project
            .languages
            .available
            .iter()
            .position(|x| x.as_str() == l)
            .unwrap_or(usize::MAX)
    };
    let target_rank = |t: &str| {
        project
            .targets
            .iter()
            .position(|x| x.id.as_str() == t)
            .unwrap_or(usize::MAX)
    };
    items.sort_by(|a, b| {
        (lang_rank(&a.entry.lang), &a.entry.lang)
            .cmp(&(lang_rank(&b.entry.lang), &b.entry.lang))
            .then_with(|| {
                (target_rank(&a.entry.target), &a.entry.target)
                    .cmp(&(target_rank(&b.entry.target), &b.entry.target))
            })
            .then_with(|| a.entry.image.cmp(&b.entry.image))
            .then_with(|| a.path.cmp(b.path))
    });

    let mut langs: Vec<&str> = Vec::new();
    let mut targets: Vec<&str> = Vec::new();
    for it in &items {
        if !langs.contains(&it.entry.lang.as_str()) {
            langs.push(&it.entry.lang);
        }
        if !targets.contains(&it.entry.target.as_str()) {
            targets.push(&it.entry.target);
        }
    }

    let name = if project.name.is_empty() {
        "Export"
    } else {
        project.name.as_str()
    };
    let mut h = String::with_capacity(16 * 1024 + items.len() * 512);
    h.push_str("<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n");
    h.push_str("<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n");
    h.push_str("<meta name=\"generator\" content=\"Vitrine Studio\">\n");
    let _ = writeln!(h, "<title>{} · export</title>", esc(name));
    h.push_str("<style>\n");
    h.push_str(CSS);
    h.push_str("</style>\n</head>\n<body>\n<header>\n");
    let _ = writeln!(h, "<h1>{}</h1>", esc(name));
    let layout_label = match layout {
        ExportLayout::Standard => "standard layout",
        ExportLayout::Fastlane => {
            "fastlane layout (deliver: screenshots/, supply: metadata/android/)"
        }
    };
    let _ = writeln!(
        h,
        "<p class=\"meta\">{} · {} · {} · {}</p>",
        count(items.len(), "file"),
        count(langs.len(), "language"),
        count(targets.len(), "target"),
        layout_label
    );
    if langs.len() > 1 {
        h.push_str("<nav>");
        for l in &langs {
            let _ = write!(h, "<a href=\"#lang-{0}\">{0}</a>", esc(l));
        }
        h.push_str("</nav>\n");
    }
    h.push_str("</header>\n<main>\n");
    if items.is_empty() {
        h.push_str("<p class=\"empty\">No exported files.</p>\n");
    }

    let mut i = 0;
    while i < items.len() {
        let lang = items[i].entry.lang.as_str();
        let _ = writeln!(
            h,
            "<section id=\"lang-{0}\">\n<h2>{0}</h2>",
            esc(if lang.is_empty() { "?" } else { lang })
        );
        while i < items.len() && items[i].entry.lang == lang {
            let target = items[i].entry.target.as_str();
            let start = i;
            while i < items.len() && items[i].entry.lang == lang && items[i].entry.target == target
            {
                i += 1;
            }
            let group = &items[start..i];
            let mut dirs: Vec<&str> = Vec::new();
            for it in group {
                let dir = it.path.rsplit_once('/').map_or("", |(d, _)| d);
                if !dirs.contains(&dir) {
                    dirs.push(dir);
                }
            }
            let _ = writeln!(
                h,
                "<div class=\"target\">\n<h3>{} <span class=\"muted\">{} · {}</span></h3>\n<div class=\"grid\">",
                esc(if target.is_empty() { "?" } else { target }),
                count(group.len(), "file"),
                dirs.iter()
                    .map(|d| format!("{}/", esc(d)))
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            for it in group {
                let file = it.path.rsplit_once('/').map_or(it.path, |(_, f)| f);
                let size = format!("{}×{}", it.width, it.height);
                let caption = format!("{} · {size} · {}", it.path, human_bytes(it.bytes));
                let wide = it.width > it.height;
                let _ = writeln!(
                    h,
                    "<figure class=\"{cls}\"><button type=\"button\" class=\"thumb\" data-caption=\"{cap}\"><img src=\"{src}\" alt=\"{alt}\" loading=\"lazy\" width=\"{w}\" height=\"{hh}\"></button><figcaption><span class=\"name\" title=\"{title}\">{alt}</span><span class=\"muted\">{size} · {bytes}</span></figcaption></figure>",
                    cls = if wide { "wide" } else { "tall" },
                    cap = esc(&caption),
                    src = esc(&url_path(it.path)),
                    alt = esc(file),
                    w = it.width,
                    hh = it.height,
                    title = esc(it.path),
                    bytes = human_bytes(it.bytes),
                );
            }
            h.push_str("</div>\n</div>\n");
        }
        h.push_str("</section>\n");
    }
    h.push_str("</main>\n");
    h.push_str(VIEWER);
    h.push_str("<script>\n");
    h.push_str(JS);
    h.push_str("</script>\n</body>\n</html>\n");
    h
}

/// Writes `index.html` into `out_dir`.
pub fn write(
    project: &Project,
    out_dir: &Path,
    layout: ExportLayout,
    manifest: &BTreeMap<String, ManifestEntry>,
) -> std::io::Result<()> {
    std::fs::create_dir_all(out_dir)?;
    std::fs::write(
        out_dir.join(INDEX),
        render(project, out_dir, layout, manifest),
    )
}

fn count(n: usize, what: &str) -> String {
    if n == 1 {
        format!("1 {what}")
    } else {
        format!("{n} {what}s")
    }
}

fn human_bytes(b: u64) -> String {
    #[allow(clippy::cast_precision_loss)]
    let f = b as f64;
    if b < 1024 {
        format!("{b} B")
    } else if b < 1024 * 1024 {
        format!("{:.0} KB", f / 1024.0)
    } else {
        format!("{:.1} MB", f / (1024.0 * 1024.0))
    }
}

/// HTML text / attribute escaping.
fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            c => out.push(c),
        }
    }
    out
}

/// Relative URL of a `/`-separated path (each segment percent-encoded).
fn url_path(path: &str) -> String {
    let mut out = String::with_capacity(path.len());
    for b in path.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~' | b'/') {
            out.push(char::from(b));
        } else {
            let _ = write!(out, "%{b:02X}");
        }
    }
    out
}

const CSS: &str = r":root {
  color-scheme: light dark;
  --bg: #f5f5f7; --fg: #1d1d1f; --muted: #6e6e73; --card: #ffffff;
  --line: #d2d2d7; --accent: #0a66d8; --shade: rgba(0, 0, 0, .82);
}
@media (prefers-color-scheme: dark) {
  :root { --bg: #161618; --fg: #f5f5f7; --muted: #a1a1a6; --card: #232326;
    --line: #3a3a3d; --accent: #4c9bff; --shade: rgba(0, 0, 0, .9); }
}
* { box-sizing: border-box; }
body { margin: 0; background: var(--bg); color: var(--fg);
  font: 14px/1.45 -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Helvetica, Arial, sans-serif; }
header { padding: 24px 24px 8px; }
h1 { margin: 0 0 4px; font-size: 22px; }
h2 { margin: 28px 0 8px; font-size: 18px; border-bottom: 1px solid var(--line); padding-bottom: 4px; }
h3 { margin: 18px 0 8px; font-size: 15px; font-weight: 600; }
main { padding: 0 24px 40px; }
.meta, .muted { color: var(--muted); font-weight: 400; }
h3 .muted { font-size: 12px; margin-left: 6px; word-break: break-all; }
nav { display: flex; flex-wrap: wrap; gap: 6px; margin-top: 8px; }
nav a { color: var(--accent); text-decoration: none; padding: 2px 8px;
  border: 1px solid var(--line); border-radius: 999px; background: var(--card); }
.grid { display: flex; flex-wrap: wrap; gap: 14px; align-items: flex-start; }
figure { margin: 0; background: var(--card); border: 1px solid var(--line); border-radius: 10px;
  padding: 8px; display: flex; flex-direction: column; gap: 6px; }
figure.tall { width: 180px; }
figure.wide { width: 340px; }
.thumb { display: block; padding: 0; border: 0; background: none; cursor: zoom-in; width: 100%; }
.thumb img { display: block; width: 100%; height: auto; max-height: 360px; object-fit: contain;
  border-radius: 4px; background: repeating-conic-gradient(var(--line) 0 25%, transparent 0 50%) 0 0 / 16px 16px; }
.thumb:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; border-radius: 4px; }
figcaption { display: flex; flex-direction: column; font-size: 12px; min-width: 0; }
figcaption .name { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-weight: 500; }
.empty { color: var(--muted); }
dialog { border: 0; padding: 0; background: transparent; max-width: 100vw; max-height: 100vh;
  width: 100vw; height: 100vh; color: #f5f5f7; }
dialog::backdrop { background: var(--shade); }
dialog figure { background: none; border: 0; width: 100%; height: 100%; padding: 16px 64px;
  display: flex; flex-direction: column; align-items: center; justify-content: center; }
dialog img { max-width: 100%; max-height: calc(100vh - 80px); object-fit: contain; }
dialog figcaption { font-size: 13px; text-align: center; }
dialog button { position: absolute; background: rgba(255, 255, 255, .12); color: #fff; border: 0;
  border-radius: 999px; width: 40px; height: 40px; font-size: 20px; cursor: pointer; }
dialog button:hover { background: rgba(255, 255, 255, .25); }
#v-close { top: 14px; right: 14px; }
#v-prev { left: 14px; top: 50%; }
#v-next { right: 14px; top: 50%; }
";

const VIEWER: &str = "<dialog id=\"viewer\" aria-label=\"Image viewer\"><figure><img id=\"v-img\" alt=\"\"><figcaption id=\"v-cap\"></figcaption></figure><button type=\"button\" id=\"v-close\" aria-label=\"Close\">×</button><button type=\"button\" id=\"v-prev\" aria-label=\"Previous\">‹</button><button type=\"button\" id=\"v-next\" aria-label=\"Next\">›</button></dialog>\n";

const JS: &str = r"(function () {
  var thumbs = Array.prototype.slice.call(document.querySelectorAll('.thumb'));
  var dlg = document.getElementById('viewer');
  var img = document.getElementById('v-img');
  var cap = document.getElementById('v-cap');
  var cur = -1;
  function show(i) {
    if (!thumbs.length) return;
    cur = (i + thumbs.length) % thumbs.length;
    var t = thumbs[cur];
    img.src = t.querySelector('img').getAttribute('src');
    cap.textContent = t.getAttribute('data-caption') + '  (' + (cur + 1) + '/' + thumbs.length + ')';
    if (!dlg.open) {
      if (dlg.showModal) dlg.showModal(); else dlg.setAttribute('open', '');
    }
  }
  function close() {
    if (dlg.close) dlg.close(); else dlg.removeAttribute('open');
    if (cur >= 0) thumbs[cur].focus();
  }
  thumbs.forEach(function (t, i) { t.addEventListener('click', function () { show(i); }); });
  document.getElementById('v-close').addEventListener('click', close);
  document.getElementById('v-prev').addEventListener('click', function (e) { e.stopPropagation(); show(cur - 1); });
  document.getElementById('v-next').addEventListener('click', function (e) { e.stopPropagation(); show(cur + 1); });
  dlg.addEventListener('click', function (e) { if (e.target !== img) close(); });
  document.addEventListener('keydown', function (e) {
    if (!dlg.open) return;
    if (e.key === 'ArrowRight') show(cur + 1);
    else if (e.key === 'ArrowLeft') show(cur - 1);
    else if (e.key === 'Escape') { e.preventDefault(); close(); }
  });
})();
";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_and_encodes() {
        assert_eq!(
            esc("<a href=\"x\">&'"),
            "&lt;a href=&quot;x&quot;&gt;&amp;&#39;"
        );
        assert_eq!(url_path("ios/en/01_a+b.png"), "ios/en/01_a%2Bb.png");
        assert_eq!(url_path("de DE/ü#.png"), "de%20DE/%C3%BC%23.png");
        assert_eq!(human_bytes(512), "512 B");
        assert_eq!(human_bytes(2048), "2 KB");
        assert_eq!(human_bytes(3 * 1024 * 1024), "3.0 MB");
    }
}
