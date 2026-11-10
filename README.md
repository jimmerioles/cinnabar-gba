# Cinnabar

**A clean-room Game Boy Advance emulator in Rust.**

Cinnabar is split into a headless, deterministic emulator core and a small native
frontend. It boots real GBA ROMs, runs the full ARM and Thumb instruction sets,
renders modes 0–5, plays PSG and DMA sound, and persists SRAM/Flash saves. No
Nintendo BIOS image is required: the common BIOS SWI calls are implemented as
HLE.

![Cinnabar demo](docs/demo.png)

| Tiled mode + sprite + alpha blending | Full ARM test suite |
|---|---|
| ![Tiled demo](docs/ppu-tiled.png) | ![ARM tests passed](docs/tests-arm.png) |

## The highlights

- **ARM7TDMI interpreter** with ARM and Thumb, banked registers, exceptions, and
  the ARM7TDMI pipeline quirks that real test ROMs care about.
- **Passes the full `gba-tests` ARM and Thumb suites**, plus the memory, SRAM,
  Flash 64K, and Flash 128K suites.
- **Scanline PPU** with modes 0–5, text and affine backgrounds, sprites,
  windows, alpha blending, and brightness effects.
- **Audio** with the four GB channels (square, wave, noise), envelopes, sweep,
  frame sequencer, and DMA sound A/B, mixed to 48 kHz stereo.
- **DMA, timers, interrupts, keypad, cartridge saves**, and a BIOS HLE for
  decompression, CpuSet/CpuFastSet, division, square root, affine setup, and
  interrupt waits.
- **Native frontend** using `winit`, `softbuffer`, and `cpal`: integer-scaled
  video, audio, keyboard input, pause, fast-forward, screenshots, and save
  persistence.
- **Headless mode** for CI and screenshots: no window, no audio device, just
  frames in and a PNG out.

## Quick start

```bash
cargo run -p cinnabar-frontend -- --demo
```

That runs the bundled logo demo. To play a ROM you own:

```bash
cargo run -p cinnabar-frontend -- /path/to/game.gba
```

Build a release binary:

```bash
cargo build --release -p cinnabar-frontend
./target/release/cinnabar /path/to/game.gba
```

Headless screenshot:

```bash
cargo run -p cinnabar-frontend -- /path/to/game.gba \
  --headless --frames 600 --screenshot shot.png
```

Inspect a cartridge without running it:

```bash
cargo run -p cinnabar-frontend -- /path/to/game.gba --info
```

### Controls

| Key | GBA |
|---|---|
| `Z` | A |
| `X` | B |
| `Enter` | Start |
| `Backspace` | Select |
| Arrow keys | D-pad |
| `A` | L |
| `S` | R |
| `Esc` | Quit |
| `F1` | Pause |
| `F2` | Fast-forward |
| `F5` | Screenshot to `screenshots/` |
| `F11` | Fullscreen |

Saves are written next to the ROM as `<rom>.sav` (or to `--save <path>`), and
loaded automatically on the next run.

## Architecture

```text
crates/
├── cinnabar-core/       headless emulator; no window, audio device, or filesystem
│   ├── cpu/             ARM7TDMI: ARM, Thumb, banked registers, exceptions
│   ├── bus.rs           memory map, I/O dispatch, DMA and event stepping
│   ├── ppu.rs           scanline renderer, modes 0–5, sprites, windows, blending
│   ├── apu.rs           PSG + DMA sound, 48 kHz stereo mixer
│   ├── cart.rs          header parsing and ROM mirroring
│   ├── save.rs          SRAM and AMD-style Flash, 8-bit bus semantics
│   ├── bios.rs          HLE SWI implementations
│   └── system.rs        CPU + bus + frame loop
├── cinnabar-frontend/   winit + softbuffer + cpal + clap binary
└── cinnabar-testrom/    hand-written demo ROMs and their assembly
```

The core is deliberately I/O-free. `System::run_frame()` returns a
`FrameOutput` containing a 240×160 BGR555 framebuffer and interleaved stereo
audio. That makes the emulator testable in CI without a display or sound card.

The PPU renders one scanline at a time. The CPU advances the bus in cycle
chunks, and the bus steps timers, audio, the PPU, and DMA together. The frontend
owns the window, audio device, frame pacing, and save files.

## Validation

The emulator is checked against the open-source
[`jsmolka/gba-tests`](https://github.com/jsmolka/gba-tests) suite:

```bash
./tools/validate.sh
```

```text
PASS arm                r12=00000000
PASS thumb              r7=00000000
PASS memory             r12=00000000
PASS sram               r12=00000000
PASS flash64            r12=00000000
PASS flash128           r12=00000000
PASS none               r12=00000000
PASS hello              colors=2
PASS shades             colors=15
PASS stripes            colors=2
all selected gba-tests validations passed
```

| Suite | Status |
|---|---|
| ARM instructions | **PASS** (all tests) |
| Thumb instructions | **PASS** (all tests) |
| Memory mirrors | **PASS** |
| SRAM saves | **PASS** |
| Flash 64K | **PASS** |
| Flash 128K / bank switching | **PASS** |
| PPU hello / shades / stripes | **PASS** (visual) |
| Mode 0 tiles + sprite + alpha blending | **PASS** (demo + integration test) |
| DMA + timer/VBlank IRQ | **PASS** (integration test) |
| VBlankIntrWait / halt-wake | **PASS** (integration test) |
| BIOS internals | Not applicable: Cinnabar uses HLE instead of a real BIOS |
| EEPROM saves | Not implemented |
| `unsafe` suite | Test 2 is documented upstream as failing on real hardware; it is not part of our validation |

The workspace test suite covers CPU decoding, flags, exceptions, PPU mode 3, APU
synthesis, frontend video/audio conversion, CLI, save storage, and the demo
cartridge integration test:

```bash
cargo test --workspace
```

### Reproducing the test ROMs

```bash
./tools/fetch-test-roms.sh   # clones gba-tests into test-roms/ (gitignored)
./tools/validate.sh          # runs the selected suites and PPU demos
```

### Rebuilding the demo ROMs

The demo ROMs are hand-written ARM assembly:

```bash
./tools/build-demo-rom.sh    # requires llvm-mc, llvm-objcopy, python3
```

Source lives in `crates/cinnabar-testrom/roms/`.

## Development

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Or use the `Makefile`:

```bash
make test
make lint
make validate
make demo
```

## What works, and what is next

Cinnabar is a real emulator, not a tech demo, but no emulator is ever finished.
The honest compatibility picture:

**Working**

- ARM and Thumb instruction sets, banked registers, mode changes, exceptions.
- Memory map, mirrors, cartridge ROM access, SRAM/Flash saves.
- PPU modes 0–5, text/affine backgrounds, sprites, windows, alpha and brightness
  effects.
- PSG channels, DMA sound, frame sequencer, stereo mixer.
- DMA, timers, IRQ controller, keypad, BIOS HLE including `Halt` and
  `VBlankIntrWait`.
- Windowed frontend, audio, headless screenshots, save persistence.

**Known limitations**

- BIOS internals are HLE, not a cycle-accurate BIOS. The `bios` test ROM is not
  supported by design.
- EEPROM saves are not implemented yet.
- Audio DMA timing is approximated, and high square/noise frequencies alias at
  48 kHz.
- Mosaic and a few PPU edge cases are register-complete but not fully rendered.
- No save states, debugger, link cable, netplay, or shaders yet.
- Commercial game compatibility is still a work in progress. The CPU, memory,
  and save foundations are validated; PPU edge cases and timing are the next
  compatibility frontier.

See [`ROADMAP.md`](ROADMAP.md) for the next milestones.

## Legal

Cinnabar contains **no Nintendo ROMs and no Nintendo BIOS**. The demo cartridges
are generated from the assembly in this repository. You must supply your own
legally dumped ROMs and BIOS if you want to use them. `gba-tests` is fetched
separately and remains under its own license.

Game Boy Advance is a trademark of Nintendo. Cinnabar is not affiliated with or
endorsed by Nintendo.

## Credits

- [`jsmolka/gba-tests`](https://github.com/jsmolka/gba-tests) for the test ROMs
  that keep the CPU honest.
- GBATEK, TONC, and Pan Docs for the hardware documentation.
- mGBA and NanoBoyAdvance as reference implementations and debugging companions.
- The Rust ecosystem: `winit`, `softbuffer`, `cpal`, `clap`, `rtrb`, and `image`.

## License

MIT. See [`LICENSE`](LICENSE).
