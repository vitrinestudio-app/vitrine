#!/usr/bin/env bash
# Collects the licence files shipped with every Vitrine Studio build
# (ADR 0022, ADR 0026):
#
#   <out>/LICENSE.txt                  Vitrine Studio's own licence (GPL-3.0)
#   <out>/THIRD-PARTY-NOTICES.html     Rust crates (cargo about, about.toml)
#   <out>/fonts/<family>/OFL.txt       bundled fonts (SIL OFL 1.1)
#   <out>/hyphenation/LICENSES.md      bundled hyphenation patterns
#
# Usage: scripts/release/third-party-notices.sh <out-dir>
# Needs `cargo about` (pinned version: see .github/workflows/release.yml).
set -euo pipefail

out="${1:?usage: third-party-notices.sh <out-dir>}"
root="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$root"

rm -rf "$out"
mkdir -p "$out/fonts" "$out/hyphenation"

cp LICENSE "$out/LICENSE.txt"
# --fail: an unidentifiable licence must stop the release, not ship silently.
cargo about generate --workspace --locked --fail \
  -c about.toml scripts/release/about.hbs -o "$out/THIRD-PARTY-NOTICES.html"

mkdir -p "$out/fonts/inter"
cp assets/fonts/OFL.txt "$out/fonts/inter/OFL.txt"
for dir in assets/fonts/*/; do
  family="$(basename "$dir")"
  if [ -f "$dir/OFL.txt" ]; then
    mkdir -p "$out/fonts/$family"
    cp "$dir/OFL.txt" "$out/fonts/$family/OFL.txt"
  fi
done
cp assets/fonts/SOURCES.md "$out/fonts/SOURCES.md"
cp assets/hyphenation/LICENSES.md "$out/hyphenation/LICENSES.md"

echo "licence files in $out:"
find "$out" -type f | sort
