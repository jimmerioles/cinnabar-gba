#!/usr/bin/env bash
# Rebuild the hand-written demo ROMs.
#
# Requires: llvm-mc, llvm-objcopy, python3.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

for name in demo logo tiled irq_dma halt input; do
    src="$root/crates/cinnabar-testrom/roms/$name.S"
    out="$root/crates/cinnabar-testrom/roms/$name.gba"

    llvm-mc -triple=armv4t-none-eabi -filetype=obj "$src" -o "$tmp/$name.o"
    llvm-objcopy -O binary --only-section=.text "$tmp/$name.o" "$tmp/$name.bin"

    python3 - "$tmp/$name.bin" "$out" <<'PY'
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
done
