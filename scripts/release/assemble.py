#!/usr/bin/env python3
"""Assembles the release metadata from the collected build artifacts (ADR 0026).

Usage: assemble.py --dist DIR --version 0.1.0 --tag v0.1.0 --repo owner/name

Reads DIR (the files of scripts/release/collect-artifacts.sh from every
platform, plus the `signing-<platform>.txt` status lines) and writes into DIR:

- latest.json      Tauri updater manifest (only platforms with a `.sig`;
                   skipped entirely when no updater signature exists)
- SHA256SUMS.txt   checksums of every release file (`sha256sum -c` format)
- RELEASE_NOTES.md draft release notes (edit in the GitHub draft)

Standard library only (runs on the GitHub runner's python3).
"""

import argparse
import datetime
import hashlib
import json
import pathlib
import sys

# Updater manifest keys per artifact (tauri-plugin-updater looks up
# `{os}-{arch}-{installer}` first, then `{os}-{arch}`). The universal macOS
# app serves both architectures.
UPDATER_ARTIFACTS = [
    ("_macos_universal.app.tar.gz", ["darwin-aarch64", "darwin-aarch64-app", "darwin-x86_64", "darwin-x86_64-app"]),
    ("_windows_x64-setup.exe", ["windows-x86_64", "windows-x86_64-nsis"]),
    ("_linux_x86_64.AppImage", ["linux-x86_64", "linux-x86_64-appimage"]),
]


def sha256(path: pathlib.Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for block in iter(lambda: f.read(1 << 20), b""):
            h.update(block)
    return h.hexdigest()


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--dist", required=True)
    ap.add_argument("--version", required=True)
    ap.add_argument("--tag", required=True)
    ap.add_argument("--repo", required=True)
    args = ap.parse_args()
    dist = pathlib.Path(args.dist)
    base = f"https://github.com/{args.repo}/releases/download/{args.tag}"
    release_page = f"https://github.com/{args.repo}/releases/tag/{args.tag}"

    signing = sorted(
        line.strip()
        for p in dist.glob("signing-*.txt")
        for line in p.read_text(encoding="utf-8").splitlines()
        if line.strip()
    )

    # latest.json
    platforms = {}
    for suffix, keys in UPDATER_ARTIFACTS:
        name = f"VitrineStudio{suffix}"
        artifact, sig = dist / name, dist / f"{name}.sig"
        if artifact.is_file() and sig.is_file():
            entry = {"signature": sig.read_text(encoding="utf-8").strip(), "url": f"{base}/{name}"}
            for key in keys:
                platforms[key] = entry
    if platforms:
        manifest = {
            "version": args.version,
            "notes": f"Vitrine Studio {args.version}. Release notes: {release_page}",
            "pub_date": datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
            "platforms": platforms,
        }
        (dist / "latest.json").write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
        print(f"latest.json: {', '.join(sorted(platforms))}")
    else:
        print("no updater signatures (TAURI_SIGNING_PRIVATE_KEY not set): no latest.json", file=sys.stderr)

    # SHA256SUMS.txt (after latest.json so it is included)
    skip = {"SHA256SUMS.txt", "RELEASE_NOTES.md"}
    files = sorted(
        p for p in dist.iterdir() if p.is_file() and not p.name.startswith("signing-") and p.name not in skip
    )
    sums = "".join(f"{sha256(p)}  {p.name}\n" for p in files)
    (dist / "SHA256SUMS.txt").write_text(sums, encoding="utf-8")

    def link(suffix: str) -> str:
        name = f"VitrineStudio{suffix}"
        return f"[`{name}`]({base}/{name})" if (dist / name).is_file() else "–"

    def cli(suffix: str) -> str:
        name = f"vitrine-cli{suffix}"
        return f"[`{name}`]({base}/{name})" if (dist / name).is_file() else "–"

    unsigned = [s for s in signing if "unsigned" in s or "not notarized" in s]
    notes = f"""## Vitrine Studio {args.version}

<!-- Owner: replace this line with the highlights of the release. -->

### Downloads

| Platform | App | Command line tool only |
|---|---|---|
| macOS 11+ (Apple silicon + Intel) | {link('_macos_universal.dmg')} | {cli('_macos_universal.tar.gz')} |
| Windows 10/11 x64 | {link('_windows_x64-setup.exe')} | {cli('_windows_x64.zip')} |
| Linux x86_64 | {link('_linux_x86_64.AppImage')} · {link('_linux_amd64.deb')} · {link('_linux_x86_64.rpm')} | {cli('_linux_x86_64.tar.gz')} |
| Linux aarch64 (arm64) | – | {cli('_linux_aarch64.tar.gz')} |
| Linux, static (musl: Alpine, any distribution) | – | {cli('_linux_x86_64-musl.tar.gz')} · {cli('_linux_aarch64-musl.tar.gz')} |

The app contains the `vitrine` command line tool (App settings → Command line
tool installs it for terminals). The glibc Linux builds need Ubuntu 22.04 /
Debian 12 or newer; the static musl builds run anywhere on Linux. For CI there
is also a container image (amd64 + arm64, Alpine, static build):
`docker pull ghcr.io/{args.repo.split('/')[0].lower()}/vitrine-cli:{args.version}`.
Checksums: `SHA256SUMS.txt` (`sha256sum -c SHA256SUMS.txt`).

### Signing

{chr(10).join(f'- {s}' for s in signing) if signing else '- (no signing information)'}
"""
    if unsigned:
        notes += """
Unsigned builds trigger a warning on first start: macOS – right-click the app →
Open (or System Settings → Privacy & Security → Open Anyway); Windows –
SmartScreen "More info" → "Run anyway".
"""
    notes += f"""
### Licence and source

Vitrine Studio is free software under the GNU GPL v3 or later. The
corresponding source code of this release is the tag
[`{args.tag}`](https://github.com/{args.repo}/tree/{args.tag}) (also as the
"Source code" archives below). Third-party licences ship with every download
(`licenses/THIRD-PARTY-NOTICES.html`, fonts: SIL Open Font License 1.1).
"""
    (dist / "RELEASE_NOTES.md").write_text(notes, encoding="utf-8")
    print(sums, end="")
    return 0


if __name__ == "__main__":
    sys.exit(main())
