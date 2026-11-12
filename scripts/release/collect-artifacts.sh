#!/usr/bin/env bash
# Copies the bundles of one release build into <out>/ with stable, space-free
# names and packs the standalone CLI archive (ADR 0026):
#
#   macos:   VitrineStudio_macos_universal.dmg
#            VitrineStudio_macos_universal.app.tar.gz[.sig]   (updater)
#            vitrine-cli_macos_universal.tar.gz               (signed CLI from the .app)
#   windows: VitrineStudio_windows_x64-setup.exe[.sig]        (installer + updater)
#            vitrine-cli_windows_x64.zip
#   linux:   VitrineStudio_linux_x86_64.AppImage[.sig]        (updater)
#            VitrineStudio_linux_amd64.deb, VitrineStudio_linux_x86_64.rpm
#            vitrine-cli_linux_x86_64.tar.gz
#   CLI-only builds (job `cli` of release.yml, no app bundle):
#   linux-aarch64:       vitrine-cli_linux_aarch64.tar.gz        (glibc, Ubuntu 22.04+)
#   linux-x86_64-musl:   vitrine-cli_linux_x86_64-musl.tar.gz    (static, Alpine)
#   linux-aarch64-musl:  vitrine-cli_linux_aarch64-musl.tar.gz   (static, Alpine)
#
# Every CLI archive carries the licence files (LICENSE.txt, third-party
# notices, font and hyphenation licences).
#
# Usage: scripts/release/collect-artifacts.sh <macos|windows|linux|linux-aarch64|
#          linux-x86_64-musl|linux-aarch64-musl> <version> <out-dir>
# Honours CARGO_TARGET_DIR (default: target).
set -euo pipefail

platform="${1:?platform}"
version="${2:?version}"
out="${3:?out dir}"
root="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$root"
target_dir="${CARGO_TARGET_DIR:-target}"
licenses="crates/gui/bundle/licenses"
mkdir -p "$out"
out="$(cd "$out" && pwd)"
stage="$(mktemp -d)"

copy_with_sig() { # <src> <dest name>
  cp "$1" "$out/$2"
  if [ -f "$1.sig" ]; then cp "$1.sig" "$out/$2.sig"; fi
}

one() { # the single file matching a glob, or fail (bundle names contain spaces)
  local IFS=$'\n'
  local matches=( $1 )
  if [ "${#matches[@]}" -ne 1 ] || [ ! -e "${matches[0]}" ]; then
    echo "expected exactly one file for $1, found: ${matches[*]:-none}" >&2
    exit 1
  fi
  printf '%s' "${matches[0]}"
}

cli_dir="$stage/vitrine-cli_${version}"
mkdir -p "$cli_dir"
cp -R "$licenses" "$cli_dir/licenses"

case "$platform" in
  macos)
    bundle="$target_dir/universal-apple-darwin/release/bundle"
    copy_with_sig "$(one "$bundle/dmg/*.dmg")" "VitrineStudio_macos_universal.dmg"
    copy_with_sig "$(one "$bundle/macos/*.app.tar.gz")" "VitrineStudio_macos_universal.app.tar.gz"
    # The CLI as signed (and notarized) inside the app bundle.
    app="$(one "$bundle/macos/*.app")"
    cp "$app/Contents/MacOS/vitrine" "$cli_dir/vitrine"
    tar -C "$stage" -czf "$out/vitrine-cli_macos_universal.tar.gz" "vitrine-cli_${version}"
    ;;
  windows)
    bundle="$target_dir/x86_64-pc-windows-msvc/release/bundle"
    copy_with_sig "$(one "$bundle/nsis/*-setup.exe")" "VitrineStudio_windows_x64-setup.exe"
    cp crates/gui/binaries/vitrine-x86_64-pc-windows-msvc.exe "$cli_dir/vitrine.exe"
    (cd "$stage" && 7z a -tzip -bso0 "$out/vitrine-cli_windows_x64.zip" "vitrine-cli_${version}")
    ;;
  linux)
    bundle="$target_dir/x86_64-unknown-linux-gnu/release/bundle"
    copy_with_sig "$(one "$bundle/appimage/*.AppImage")" "VitrineStudio_linux_x86_64.AppImage"
    copy_with_sig "$(one "$bundle/deb/*.deb")" "VitrineStudio_linux_amd64.deb"
    copy_with_sig "$(one "$bundle/rpm/*.rpm")" "VitrineStudio_linux_x86_64.rpm"
    cp crates/gui/binaries/vitrine-x86_64-unknown-linux-gnu "$cli_dir/vitrine"
    chmod +x "$cli_dir/vitrine"
    tar -C "$stage" -czf "$out/vitrine-cli_linux_x86_64.tar.gz" "vitrine-cli_${version}"
    ;;
  linux-aarch64 | linux-x86_64-musl | linux-aarch64-musl)
    case "$platform" in
      linux-aarch64) triple=aarch64-unknown-linux-gnu ;;
      linux-x86_64-musl) triple=x86_64-unknown-linux-musl ;;
      linux-aarch64-musl) triple=aarch64-unknown-linux-musl ;;
    esac
    cp "$target_dir/$triple/release/vitrine" "$cli_dir/vitrine"
    chmod +x "$cli_dir/vitrine"
    tar -C "$stage" -czf "$out/vitrine-cli_linux_${platform#linux-}.tar.gz" "vitrine-cli_${version}"
    ;;
  *)
    echo "unknown platform $platform" >&2
    exit 2
    ;;
esac

rm -rf "$stage"
ls -la "$out"
