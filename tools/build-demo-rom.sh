#!/usr/bin/env bash
# Rebuild the hand-written demo ROM.
#
# Requires: llvm-mc, llvm-objcopy, python3.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
src="$root/crates/cinnabar-testrom/roms/demo.S"
out="$root/crates/cinnabar-testrom/roms/demo.gba"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

llvm-mc -triple=armv4t-none-eabi -filetype=obj "$src" -o "$tmp/demo.o"
llvm-objcopy -O binary --only-section=.text "$tmp/demo.o" "$tmp/demo.bin"

python3 - "$tmp/demo.bin" "$out" <<'PY'
import sys
from pathlib import Path

src, out = map(Path, sys.argv[1:])
data = bytearray(src.read_bytes())
if len(data) < 0x8000:
    data += bytes(0x8000 - len(data))
total = sum(data[0xA0:0xBD]) & 0xFF
data[0xBD] = (-(total + 0x19)) & 0xFF
out.write_bytes(data)
print(f"wrote {out} ({len(data)} bytes)")
PY
