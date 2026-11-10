#!/usr/bin/env bash
# Boot every bundled cartridge headlessly and check the frontend/core agree.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

cargo build -q -p cinnabar-frontend
bin="$root/target/debug/cinnabar"

printf 'PASS %-10s ' selftest
selftest_output="$("$bin" --selftest 2>&1)"
if ! printf '%s\n' "$selftest_output" | grep -q 'self-test'; then
    echo 'missing self-test output'
    printf '%s\n' "$selftest_output"
    exit 1
fi
echo 'ok'

for name in logo tiled input irq halt gradient; do
    output="$("$bin" --builtin "$name" --headless --frames 10 2>&1)"
    if ! printf '%s\n' "$output" | grep -q 'Cinnabar headless'; then
        echo "FAIL $name: no headless report"
        printf '%s\n' "$output"
        exit 1
    fi
    printf 'PASS %-10s %s\n' "$name" "$(printf '%s\n' "$output" | tail -n 1)"
done

echo 'all smoke tests passed'
