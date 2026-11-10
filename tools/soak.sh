#!/usr/bin/env bash
# Run a user-supplied ROM headlessly for a long soak.
set -euo pipefail

usage() {
    echo "usage: $0 <rom.gba> [frames]" >&2
    echo "  default frames: 216000 (about one hour of emulated time)" >&2
    exit 2
}

rom="${1:-}"
[ -n "$rom" ] || usage
[ -f "$rom" ] || {
    echo "ROM not found: $rom" >&2
    exit 2
}

frames="${2:-216000}"
case "$frames" in
    ''|*[!0-9]*)
        echo "frames must be a positive integer" >&2
        exit 2
        ;;
esac

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

cargo build --release -q -p cinnabar-frontend
bin="$root/target/release/cinnabar"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

start="$(date +%s)"
"$bin" "$rom" --headless --frames "$frames" --screenshot "$tmp/final.png" \
    --save "$tmp/soak.sav"
elapsed="$(( $(date +%s) - start ))"

echo "soak survived: $frames frames in ${elapsed}s; screenshot: $tmp/final.png"
