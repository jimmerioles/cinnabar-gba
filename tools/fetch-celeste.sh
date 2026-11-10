#!/usr/bin/env bash
# Fetch the open-source Celeste Classic GBA port for manual testing.
# Source: https://github.com/JeffRuLz/Celeste-Classic-GBA (see upstream for licensing).
set -euo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
out="$root/test-roms/homebrew/celeste.gba"
mkdir -p "$(dirname "$out")"
if [ ! -f "$out" ]; then
    curl -L --fail --progress-bar \
        -o "$out" \
        https://github.com/JeffRuLz/Celeste-Classic-GBA/releases/download/v1.2/Celeste.Classic.v1.2.Homebrew.gba
fi
echo "Celeste Classic ready: $out"
