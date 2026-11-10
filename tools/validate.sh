#!/usr/bin/env bash
# Validate Cinnabar's CPU, memory and save behaviour against the open-source
# gba-tests suite. Run from anywhere; the script locates the repository root.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"
"$root/tools/fetch-test-roms.sh" >/dev/null

cargo build -q -p cinnabar-core --example screenshot
bin="$root/target/debug/examples/screenshot"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

failures=0

check_result() {
    local rom="$1" reg="$2" expected="$3" name="$4"
    local out
    out="$("$bin" "$rom" 180 "$tmp/$name.png" 2>&1)"
    local actual
    actual="$(printf '%s\n' "$out" | sed -n "s/.* ${reg}=\\([0-9A-F]*\\).*/\\1/p")"
    if [ "$actual" = "$expected" ]; then
        printf 'PASS %-18s %s=%s\n' "$name" "$reg" "$actual"
    else
        printf 'FAIL %-18s %s=%s (expected %s)\n' "$name" "$reg" "$actual" "$expected"
        failures=$((failures + 1))
    fi
}

check_result "test-roms/gba-tests/arm/arm.gba" r12 00000000 arm
check_result "test-roms/gba-tests/thumb/thumb.gba" r7 00000000 thumb
check_result "test-roms/gba-tests/memory/memory.gba" r12 00000000 memory
check_result "test-roms/gba-tests/save/sram.gba" r12 00000000 sram
check_result "test-roms/gba-tests/save/flash64.gba" r12 00000000 flash64
check_result "test-roms/gba-tests/save/flash128.gba" r12 00000000 flash128
check_result "test-roms/gba-tests/save/none.gba" r12 00000000 none

check_visual() {
    local rom="$1" min_colors="$2" name="$3"
    local out colors
    out="$("$bin" "$rom" 30 "$tmp/$name.png" 2>&1)"
    colors="$(printf '%s\n' "$out" | sed -n 's/.* unique_colors=\([0-9]*\).*/\1/p')"
    if [ -n "$colors" ] && [ "$colors" -ge "$min_colors" ]; then
        printf 'PASS %-18s colors=%s\n' "$name" "$colors"
    else
        printf 'FAIL %-18s colors=%s (expected >=%s)\n' "$name" "${colors:-?}" "$min_colors"
        failures=$((failures + 1))
    fi
}

check_visual "test-roms/gba-tests/ppu/hello.gba" 2 hello
check_visual "test-roms/gba-tests/ppu/shades.gba" 10 shades
check_visual "test-roms/gba-tests/ppu/stripes.gba" 2 stripes

if [ "$failures" -ne 0 ]; then
    echo "$failures validation(s) failed"
    exit 1
fi
echo "all selected gba-tests validations passed"
