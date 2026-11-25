# Cinnabar GBA

A clean-room Game Boy Advance emulator in Rust, with a native frontend and a browser arcade.

[Play the browser arcade](https://jimmerioles.github.io/cinnabar-gba/) · [Read the case study](https://jimmerioles.github.io/cinnabar-gba/case-study.html) · no install, no BIOS, and no ROM required for the built-in demos.

https://github.com/user-attachments/assets/c5e93545-4d9d-4f6a-9ea1-4d5212dd9d69

Cinnabar runs real GBA ROMs on a deterministic, headless core and gives that core three ways to run: a desktop window, a terminal debugger, and a WebAssembly arcade. It implements ARM and Thumb, the scanline PPU, PSG and DMA audio, and SRAM/Flash/EEPROM saves. Common BIOS calls are implemented in Rust, so no Nintendo BIOS image is required.

It is for people who want an emulator they can read, test, and extend. The core owns no window, audio device, or filesystem, which is why the same code runs in CI, on the desktop, and in a browser tab. The [case study](https://jimmerioles.github.io/cinnabar-gba/case-study.html) goes deeper on the architecture, the browser arcade, the test strategy, and the tradeoffs.

## What you get

- **CPU.** ARM7TDMI interpreter with ARM and Thumb, banked registers, exceptions, and the pipeline quirks that test ROMs catch. Passes the full `gba-tests` ARM, Thumb, memory, SRAM, and Flash suites.
- **Graphics.** Scanline PPU with modes 0–5, text and affine backgrounds, sprites, windows, mosaic, alpha blending, and brightness effects.
- **Sound.** The four GB channels plus DMA sound, timer-clocked FIFOs, and a 48 kHz stereo mixer.
- **Saves.** SRAM, Flash 64K/128K, and EEPROM 512 B/8 KB, with automatic save-type detection. Native saves live next to the ROM; browser saves live in IndexedDB.
- **Three ways to run it.** A `winit` desktop app, a headless debugger with breakpoints, save states, and rewind, and a browser arcade with gamepad and touch input.
- **Headless by design.** `System::run_frame()` returns a 240×160 framebuffer and stereo samples, so screenshots, tests, and benchmarks need no display or sound card.
- **Fast.** About 450 fps on a 300-frame Celeste Classic run, roughly 7× the GBA's 59.73 Hz frame rate.

## Run it

Needs stable Rust 1.85+ and the usual Linux window and audio libraries. The commands below cover Arch and Debian/Ubuntu.

```bash
# Arch
sudo pacman -S rustup alsa-lib wayland libxkbcommon
rustup default stable

# Debian / Ubuntu
sudo apt install build-essential pkg-config libasound2-dev libudev-dev \
  libwayland-dev libxkbcommon-dev libx11-dev libxkbcommon-x11-dev

git clone https://github.com/jimmerioles/cinnabar-gba.git
cd cinnabar-gba

cargo run -p cinnabar-frontend -- --selftest   # no ROM, no window
cargo run -p cinnabar-frontend -- --demo       # built-in logo cartridge
cargo run -p cinnabar-frontend -- /path/to/game.gba

cargo build --release -p cinnabar-frontend     # ./target/release/cinnabar
cargo test --workspace

cargo install wasm-pack                         # once
make serve                                      # arcade at http://localhost:8000
```

`--selftest` checks the CPU, PPU, keypad, DMA, timers, interrupts, and bundled ROMs without a ROM or a window. `--demo` runs the logo cartridge. For your own game, use a legally dumped `.gba` file.

## Use it

The default `ergonomic` layout keeps the left hand on ESDF and the right on JKL, with the outer home-row keys as shoulders:

| Action | Keys |
|---|---|
| D-pad | `E` `S` `D` `F` |
| B / A | `J` / `K` |
| L / R | `A` / `L` |
| Start / Select | `Enter` / `Backspace` |
| Pause / fast-forward | `F1` / `F2` |
| Screenshot / fullscreen | `F5` / `F11` |
| Quit | `Esc` |

The key labels do not match the GBA buttons on purpose. Use `--keyboard-layout classic` or `numpad` for the other presets, or remap any button in `~/.config/cinnabar/config.toml`. See [`docs/config.example.toml`](docs/config.example.toml). Gamepads work automatically.

Battery saves are written to `<rom>.sav` next to the ROM or to `--save <path>`, and loaded on the next run. Drop a ROM onto the window, or ask for cartridge details without running it:

```bash
./target/release/cinnabar /path/to/game.gba --info
./target/release/cinnabar --library ~/Games/gba
./target/release/cinnabar --library ~/Games/gba --play METROID
```

The library scans for `.gba` files and sidecar covers, then launches by title, game code, or file stem. The debugger starts paused at the cartridge entry point; type `help` for breakpoints, instruction and frame stepping, memory editing, PPU viewers, save states, and rewind:

```bash
./target/release/cinnabar /path/to/game.gba --debug
```

The browser arcade boots a built-in cartridge immediately. Click the sound pill once to unlock audio, or drop in a legally dumped ROM; the file stays in your browser. Built-in demos can be linked directly with `?demo=input`, `?demo=tiled`, and so on. Saves can be exported and imported as `.sav` files.

## How it works

Cinnabar models the GBA as a CPU, a bus, and a set of peripherals. `Cpu::step()` executes one ARM or Thumb instruction; `Bus::step()` routes memory and advances the PPU, APU, timers, DMA, and interrupts in 32-cycle chunks. `System::run_frame()` returns a 240×160 framebuffer and stereo samples. Because the core is I/O-free, the same code drives the desktop app, the headless debugger, and the WASM arcade. Read the guided tour in [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md).

## Tests

`cargo test --workspace` covers CPU decoding, flags, exceptions, PPU, APU, frontend conversions, saves, the CLI, and the demo cartridge integration test. `./tools/validate.sh` fetches and runs the open-source `gba-tests` CPU, memory, and save suites, plus PPU and Celeste smoke checks. CI runs both.

[![CI](https://github.com/jimmerioles/cinnabar-gba/actions/workflows/ci.yml/badge.svg)](https://github.com/jimmerioles/cinnabar-gba/actions/workflows/ci.yml)

## Limits

- BIOS calls are HLE, not a cycle-accurate BIOS. The `bios` test ROM is unsupported by design.
- PPU timing is scanline-based, so dot-level effects and exact HBlank timing are approximate.
- Sound DMA refill timing is approximated, and high square/noise frequencies alias at 48 kHz.
- No link cable or netplay yet.
- Commercial compatibility is a work in progress. Two retail titles are verified; edge cases remain.

See [`docs/COMPATIBILITY.md`](docs/COMPATIBILITY.md) for the coverage matrix and the per-game checklist.

## Credits

Built on the open-source [`jsmolka/gba-tests`](https://github.com/jsmolka/gba-tests) suite, GBATEK, TONC, and Pan Docs, with mGBA and NanoBoyAdvance as reference implementations. The native frontend uses `winit`, `softbuffer`, `cpal`, and `clap`. The browser arcade uses `wasm-bindgen` and an AudioWorklet. Celeste Classic is an open-source homebrew port fetched by `tools/fetch-celeste.sh`; it is not bundled. The explainer's source and production notes are in [`docs/explainer/`](docs/explainer/).

The name Cinnabar comes from the red mineral once ground into vermilion pigment, and from Cinnabar Island in Pokémon Red and Blue. It is a nod, not an official association.

## License

MIT. See [`LICENSE`](LICENSE).

Cinnabar ships no Nintendo ROMs and no Nintendo BIOS. You must supply your own legally dumped ROMs. Game Boy Advance is a trademark of Nintendo. Cinnabar is independent and not affiliated with Nintendo, Game Freak, or The Pokémon Company.
