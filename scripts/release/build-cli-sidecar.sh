#!/usr/bin/env bash
# Builds the `vitrine` CLI in release mode and places it where the Tauri
# bundler expects the sidecar (`bundle.externalBin` in
# crates/gui/tauri.bundle.conf.json): crates/gui/binaries/vitrine-<target>[.exe].
#
# Usage: scripts/release/build-cli-sidecar.sh <target>
#   <target>  a Rust target triple, or `universal-apple-darwin`: builds
#             aarch64 + x86_64 and joins them with `lipo` (tauri-build needs
#             the per-arch files during the per-arch cargo runs, the bundler
#             the universal one).
# Honours CARGO_TARGET_DIR (default: target).
set -euo pipefail

target="${1:?usage: build-cli-sidecar.sh <target-triple|universal-apple-darwin>}"
root="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$root"
target_dir="${CARGO_TARGET_DIR:-target}"
case "$target_dir" in /*) ;; *) target_dir="$root/$target_dir" ;; esac
bin_dir="crates/gui/binaries"
mkdir -p "$bin_dir"

build_one() {
  local triple="$1" ext=""
  case "$triple" in *windows*) ext=".exe" ;; esac
  cargo build --release --locked -p vitrine-cli --target "$triple"
  cp "$target_dir/$triple/release/vitrine$ext" "$bin_dir/vitrine-$triple$ext"
  echo "sidecar: $bin_dir/vitrine-$triple$ext"
}

if [ "$target" = "universal-apple-darwin" ]; then
  build_one aarch64-apple-darwin
  build_one x86_64-apple-darwin
  lipo -create -output "$bin_dir/vitrine-universal-apple-darwin" \
    "$bin_dir/vitrine-aarch64-apple-darwin" "$bin_dir/vitrine-x86_64-apple-darwin"
  lipo -info "$bin_dir/vitrine-universal-apple-darwin"
else
  build_one "$target"
fi
