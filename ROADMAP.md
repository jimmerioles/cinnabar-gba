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

- [x] EEPROM saves (512 B and 8 KB, bit-banged protocol, runtime size detection).
- [x] Flash command edge cases (ID mode, bank switching, sector erase) and
  save-type detection refinements.
- [x] HBlank timing and mosaic rendering.
- [ ] Dot-level PPU effects (still scanline-based).
- [x] Sprite affine/double-size handling, OBJ window, window flag selection.
- [x] Audio DMA refills driven by timer overflows and SOUNDBIAS resolution
  quantization.
- [ ] Exact SOUNDBIAS PWM output and more advanced mixing.
- [x] Per-game compatibility checklist and automated boot smoke tests.
- [ ] One-hour commercial-ROM soak (manual check via `./tools/soak.sh`).

v0.3 is a compatibility-depth pass, not a claim of cycle accuracy. Dot-level
PPU effects, exact PWM output, and a commercial-library soak remain documented
follow-ups.

## v0.4 — Frontend polish

- [x] Gamepad support (gilrs; D-pad/left stick and shoulders).
- [x] ROM library scanner with cover discovery and save management.
  A graphical cover grid remains.
- [x] Shaders: LCD grid, ghosting, and color correction.
- [x] TOML config, key remapping, and volume control.
- [x] Cubic audio resampling and volume control (buffer tuning remains optional).
- [x] WASM build for the browser (experimental canvas frontend; audio and
  gamepad support remain follow-ups).

v0.4 desktop polish and the experimental browser preview are in place.

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
