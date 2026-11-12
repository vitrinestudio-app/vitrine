#!/usr/bin/env bash
# Builds the standalone `vitrine` CLI for one Linux target (CLI-only release
# archives, ADR 0026) and checks the result:
#
#   aarch64-unknown-linux-gnu    plain `cargo build` (native on an arm64 runner)
#   x86_64-unknown-linux-musl    `cargo zigbuild`: zig is the C compiler and
#   aarch64-unknown-linux-musl   linker (aws-lc-sys, bundled SQLite), the
#                                result is a static executable
#
# Checks: musl builds must be static (no dynamic section / NEEDED entries);
# glibc builds list their NEEDED libraries and the newest GLIBC symbol
# version. When the host can run the binary (Linux, same architecture) it
# also runs a smoke test: `--version`, a new project, `validate --preview`
# and one rendered page.
#
# Usage: scripts/release/build-cli.sh <target-triple>
# Needs: rustup target <triple>; for musl `cargo-zigbuild` + `zig` on PATH;
#        `file` and `readelf` (binutils) for the checks.
# Honours CARGO_TARGET_DIR (default: target).
set -euo pipefail

triple="${1:?usage: build-cli.sh <target-triple>}"
root="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$root"
target_dir="${CARGO_TARGET_DIR:-target}"
case "$target_dir" in /*) ;; *) target_dir="$root/$target_dir" ;; esac

case "$triple" in
  *-unknown-linux-musl)
    cargo zigbuild --release --locked -p vitrine-cli --target "$triple"
    ;;
  *-unknown-linux-gnu)
    cargo build --release --locked -p vitrine-cli --target "$triple"
    ;;
  *)
    echo "build-cli.sh is for Linux targets only (macOS/Windows: build-cli-sidecar.sh)" >&2
    exit 2
    ;;
esac

bin="$target_dir/$triple/release/vitrine"
echo "built: $bin ($(du -h "$bin" | cut -f1))"
file "$bin"

if command -v readelf > /dev/null; then
  case "$triple" in
    *-musl)
      # Captured first: `grep -q` in a pipe can SIGPIPE readelf (pipefail).
      dynamic="$(readelf -d "$bin" 2>&1 || true)"
      headers="$(readelf -l "$bin")"
      if grep -q NEEDED <<< "$dynamic"; then
        echo "error: the musl build links shared libraries:" >&2
        grep NEEDED <<< "$dynamic" >&2
        exit 1
      fi
      if grep -q INTERP <<< "$headers"; then
        echo "error: the musl build has a program interpreter (not static)" >&2
        exit 1
      fi
      echo "static: no NEEDED entries, no interpreter"
      ;;
    *-gnu)
      echo "needed libraries:"
      readelf -d "$bin" | grep NEEDED | sed 's/^/  /'
      newest="$(readelf -V "$bin" | grep -o 'GLIBC_[0-9.]*' | sort -uV | tail -n 1)"
      echo "newest glibc symbol version: $newest"
      ;;
  esac
else
  echo "readelf not found: linkage not checked" >&2
fi

# Smoke test where the binary runs natively.
arch="${triple%%-*}"
if [ "$(uname -s)" = "Linux" ] && [ "$(uname -m)" = "$arch" ]; then
  if command -v ldd > /dev/null; then ldd "$bin" || true; fi
  tmp="$(mktemp -d)"
  trap 'rm -rf "$tmp"' EXIT
  export VITRINE_APP_DIR="$tmp/app" NO_COLOR=1
  "$bin" --version
  "$bin" new "$tmp/p" > /dev/null
  "$bin" --project "$tmp/p" validate --preview > /dev/null
  "$bin" --project "$tmp/p" render --target ios-iphone-69 --page 1 --max-edge 400 -o "$tmp/page.png"
  file "$tmp/page.png"
  echo "smoke test passed"
else
  echo "smoke test skipped (host $(uname -s)/$(uname -m) cannot run $triple)"
fi
