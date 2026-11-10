# Cinnabar GBA Roadmap

Cinnabar's core is validated against the open-source `gba-tests` CPU, memory,
and save suites. The next work is about compatibility depth, developer tools,
and the parts of the GBA that only show up when you run real games.

## v0.2 — Debugger and save states

- [x] Register/CPSR viewer with ARM and Thumb disassembly.
- [x] Breakpoints, step, step-frame, and frame stepping.
- [x] Memory hex viewer and editor.
- [x] Tile, palette, background-map, and OAM viewers (terminal).
- [x] PPU/DMA/timer/IRQ state readout and layer toggles.
- [x] Save-state serialization with multiple slots.
- [x] Rewind buffer.

v0.2 is complete in the headless debugger. Graphical panels remain part of the
v0.4 frontend polish.

**Why first:** a good debugger is the force multiplier for every compatibility
bug after this point.

## v0.3 — Compatibility depth

- EEPROM saves (512 B and 8 KB, bit-banged protocol).
- Flash command edge cases, save-type detection refinements.
- PPU timing: dot-level effects, HBlank timing, mosaic rendering.
- Sprite affine/double-size edge cases, OBJ window, priority/blending quirks.
- Audio DMA refills driven by timer overflows, SOUNDBIAS/PWM, better mixing.
- Per-game compatibility checklist and automated boot smoke tests.
- Run a legally dumped game for an hour without a regression.

## v0.4 — Frontend polish

- Gamepad support.
- ROM library with cover art and save management.
- Shaders: LCD grid, ghosting, color correction.
- Config file, key remapping, volume control.
- Better audio resampling and latency tuning.
- WASM build for the browser.

## v0.5 — The moonshots

- Link cable / multiplayer.
- Netplay.
- Real BIOS support (user-supplied, never committed).
- Cheat codes (GameShark / Action Replay).
- Automated compatibility dashboard.

## Non-goals

- Shipping ROMs or a Nintendo BIOS.
- Cycle-perfect hardware emulation for its own sake. Compatibility and
  testability come first.
- Replacing mGBA as the reference emulator. Cinnabar is a clean, readable Rust
  implementation with a strong headless core.
