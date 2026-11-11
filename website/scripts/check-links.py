#!/usr/bin/env python3
"""Checks internal links and #fragments of the built site (website/public).

    cd website && hugo --minify && python3 scripts/check-links.py

Every href/src that points into the site must exist; a #fragment must match
an id on the target page. External links (http…, mailto:) are not fetched.
Exit code 1 when something is broken.
"""
import re
import sys
from html.parser import HTMLParser
from pathlib import Path
from urllib.parse import unquote, urljoin, urlparse

ROOT = Path(__file__).resolve().parent.parent / "public"
BASE = "https://www.vitrinestudio.app/"


class Page(HTMLParser):
    def __init__(self):
        super().__init__()
        self.links, self.ids = [], set()

    def handle_starttag(self, tag, attrs):
        a = dict(attrs)
        if a.get("id"):
            self.ids.add(a["id"])
        for k in ("href", "src"):
            if a.get(k) and not (tag == "link" and a.get("rel") in ("alternate", "canonical")):
                self.links.append(a[k])
        if a.get("srcset"):
            self.links += [p.strip().split(" ")[0] for p in a["srcset"].split(",")]


def parse(path):
    p = Page()
    p.feed(path.read_text(encoding="utf-8"))
    return p


def resolve(url_path):
    f = ROOT / unquote(url_path).lstrip("/")
    if f.is_dir():
        f = f / "index.html"
    return f


pages = {f: parse(f) for f in ROOT.rglob("*.html")}
broken = []
for f, p in pages.items():
    page_url = BASE + str(f.relative_to(ROOT)).replace("index.html", "")
    for link in p.links:
        if re.match(r"^(mailto:|tel:|data:|javascript:)", link):
            continue
        u = urlparse(urljoin(page_url, link))
        if u.netloc and u.netloc != urlparse(BASE).netloc:
            continue
        target = resolve(u.path)
        if not target.exists():
            broken.append(f"{f.relative_to(ROOT)}: {link} (missing {target.relative_to(ROOT)})")
            continue
        if u.fragment and target.suffix == ".html":
            ids = pages[target].ids if target in pages else parse(target).ids
            if unquote(u.fragment) not in ids:
                broken.append(f"{f.relative_to(ROOT)}: {link} (no #{u.fragment})")

print(f"{len(pages)} pages checked, {len(broken)} broken links")
for b in sorted(set(broken)):
    print("  " + b)
sys.exit(1 if broken else 0)
