#!/usr/bin/env bash
# Dumps the `--help` output of every `vitrine` command and subcommand into
# website/data/cli_help.json, which the CLI reference page of the website
# renders (shortcode `cli-help`). Run it after changing a command or flag:
#
#   scripts/docs/cli-help.sh [path/to/vitrine]
#
# Without an argument it builds the CLI first (cargo build -p vitrine-cli).
set -euo pipefail

root="$(cd "$(dirname "$0")/../.." && pwd)"
bin="${1:-}"
if [[ -z "$bin" ]]; then
  cargo build -q -p vitrine-cli --manifest-path "$root/Cargo.toml"
  bin="${CARGO_TARGET_DIR:-$root/target}/debug/vitrine"
fi

subcommands() {
  # The names listed under "Commands:" of a help text, without `help`.
  "$bin" "$@" --help | awk '/^Commands:/{f=1;next} /^$/{f=0} f{print $1}' | grep -v '^help$' || true
}

out="$root/website/data/cli_help.json"
mkdir -p "$(dirname "$out")"
tmp="$(mktemp)"
{
  emit() {
    # One JSON entry: "<command path>": "<help text>".
    python3 -c 'import json,sys; print(json.dumps(sys.argv[1]) + ": " + json.dumps(sys.stdin.read().rstrip()) + ",")' "$*"
  }
  echo "{"
  "$bin" --help | emit "vitrine"
  for c in $(subcommands); do
    "$bin" "$c" --help | emit "vitrine $c"
    for s in $(subcommands "$c"); do
      "$bin" "$c" "$s" --help | emit "vitrine $c $s"
      for t in $(subcommands "$c" "$s"); do
        "$bin" "$c" "$s" "$t" --help | emit "vitrine $c $s $t"
      done
    done
  done
  echo '"_": ""}'
} > "$tmp"
python3 -c 'import json,sys; d=json.load(open(sys.argv[1])); d.pop("_"); json.dump(d, open(sys.argv[2],"w"), indent=1, ensure_ascii=False); open(sys.argv[2],"a").write("\n")' "$tmp" "$out"
rm -f "$tmp"
echo "wrote $out ($(python3 -c 'import json,sys; print(len(json.load(open(sys.argv[1]))))' "$out") commands)"
