#!/usr/bin/env bash
# Fetch the open-source gba-tests ROM suite into test-roms/ (gitignored).
set -euo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
mkdir -p "$root/test-roms"
if [ ! -d "$root/test-roms/gba-tests" ]; then
  git clone --depth 1 https://github.com/jsmolka/gba-tests.git "$root/test-roms/gba-tests"
fi
echo "gba-tests ready at $root/test-roms/gba-tests"
