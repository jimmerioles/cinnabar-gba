# GBA Emulator in Rust — Build Plan

**Working name:** Cinnabar GBA
**Status:** v0.4 frontend polish implemented, including a playable browser/WASM
arcade with audio, gamepad/touch input, saves, and shaders. See
[`README.md`](README.md) and [`ROADMAP.md`](ROADMAP.md) for the current feature
set, validation results, and remaining work.
**Goal:** a legally dumped GBA game boots, runs at full speed with audio, saves persist, and a debugger makes it pleasant to fix bugs.

**Not the goal:** cycle-perfect emulation, link cable, netplay, or a shader stack. Compatibility first, accuracy where it matters.

---

## North star

One game you love, running because of code you wrote.

The first real milestone is not "GBA complete." It is:

> **A commercial game reaches the title screen, then gameplay, at 59.73 fps, with no missing graphics.**

Everything else is refinement.

---

## Ground rules

- **The core is headless.** No `winit`, no audio device, no file dialogs. It must run in tests and CI.
- **No copyrighted ROMs or BIOS in the repo.** HLE the BIOS first. If you use a real BIOS, it stays local.
- **Compatibility over accuracy.** Implement what games actually hit, then use test ROMs to find the sharp edges.
- **Every milestone ends in a demo.** A screenshot, a GIF, or a frame counter. No invisible progress.
- **No debugger before the first game boots.** A debugger is a force multiplier, not a starting point.
- **No stretch goals until v1.** Link cable, netplay, and shaders are rewards, not distractions.

---

## Architecture

Start as a two-crate workspace. Split later only if it hurts.

```text
gba/
├── Cargo.toml              # workspace
├── crates/
│   ├── gba-core/           # pure logic, no I/O, fully testable
│   │   └── src/
│   │       ├── cpu/        # arm.rs, thumb.rs, disasm.rs
│   │       ├── bus.rs      # memory map + I/O dispatch
│   │       ├── cart.rs     # ROM header, save detection, backup media
│   │       ├── ppu.rs      # scanline renderer, backgrounds, sprites
│   │       ├── apu.rs      # 4 GB channels + 2 DMA sound channels
│   │       ├── dma.rs
│   │       ├── timer.rs
│   │       ├── irq.rs
│   │       └── system.rs   # owns everything, runs a frame
│   └── gba-frontend/       # winit, pixels, cpal, input, CLI
└── PLAN.md
```

### Core API sketch

```rust
pub struct System {
    cpu: Cpu,
    bus: Bus,
    ppu: Ppu,
    apu: Apu,
    scheduler: Scheduler,
}

impl System {
    pub fn new(cart: Cartridge) -> Self;
    pub fn run_frame(&mut self) -> FrameOutput; // framebuffer + audio samples
    pub fn set_keys(&mut self, keys: Keys);
    pub fn save_state(&self) -> Vec<u8>;
    pub fn load_state(&mut self, bytes: &[u8]) -> Result<()>;
}
```

Keep the boundary boring:

- `FrameOutput { framebuffer: [u16; 240 * 160], audio: Vec<f32> }`
- Frontend presents the framebuffer, queues audio, maps keys.
- Emulation runs on one thread. Audio consumes from a lock-free ring buffer.
- Save states are per-component snapshots. Add them after the first game boots.

### Frame loop

```text
winit events ──> Frontend ──> System::run_frame() ──> framebuffer + audio
                    │                                      │
                    └──────── present / queue audio <──────┘
```

Target: 280,896 CPU cycles per frame at 16.78 MHz ≈ 59.7275 Hz.

---

## Milestones

### M0 — Warm-up: CHIP-8 (optional, one weekend)

If you have never written an emulator, do this first. It is the same shape at 1/100th the size: fetch, decode, execute, framebuffer, input, timers.

**Done when:** IBM logo and Pong run.

**Timebox:** one weekend. Do not turn it into a product.

**Side quest (optional):** a DMG (original Game Boy) emulator running Tetris. Great trainer for PPU modes, interrupts, and MBC mappers. Timebox it to two weeks. If it stops being fun, skip straight to M2.

---

### M1 — Skeleton and frontend (2–4 evenings)

- Workspace with `gba-core` and `gba-frontend`.
- 240×160 `u16` framebuffer, scaled 4x in a window.
- Fixed 59.7275 Hz frame pacing with an FPS counter.
- Keyboard mapping for the GBA buttons.
- Screenshot key (PNG).
- `--info rom.gba` prints the cartridge header.

**Done when:** `cargo run` opens a window, draws color bars, holds ~60 fps, and Esc quits.

---

### M2 — ARM7TDMI CPU (2–3 weeks)

This is the heart of the project. Do it before the PPU.

- ARM (32-bit) and Thumb (16-bit) decode.
- 16 registers, CPSR flags, condition codes, barrel shifter.
- Data processing, branches, multiplies, load/store, halfword transfers, block transfers, PSR transfers.
- Banked registers and CPU modes: USR, SYS, SVC, ABT, IRQ, UND, FIQ.
- Exceptions: reset, undefined, SWI, prefetch abort, data abort, IRQ, FIQ.
- Pipeline approximation: PC reads as `+8` in ARM, `+4` in Thumb.
- A `Bus` trait so the CPU can be tested against a tiny RAM harness.
- Table-driven unit tests for every opcode family.

**Done when:** `armwrestler` and `FuzzARM` pass, and the CPU can execute a hand-written test ROM that writes results to RAM.

**Rabbit hole warning:** do not chase exact prefetch/waitstate timing yet. Get instructions correct first.

---

### M3 — System, bus, cartridge, BIOS HLE (2–3 weeks)

- Full memory map: BIOS, EWRAM, IWRAM, I/O, palette, VRAM, OAM, ROM, SRAM.
- Cartridge header parsing and ROM mapping at `0x08000000`.
- Save-type detection by scanning for `SRAM_V`, `FLASH_V`, `EEPROM_V` signatures.
- HLE BIOS SWI dispatcher:
  - `Halt`, `VBlankIntrWait`, `IntrWait`
  - `CpuSet`, `CpuFastSet`
  - `LZ77UnCompWram/Vram`, `RLUnCompWram/Vram`, `HuffUnComp`
  - `Div`, `DivArm`, `Sqrt`, `ArcTan`, `ArcTan2`
  - `BgAffineSet`, `ObjAffineSet`, `BitUnPack`
  - `RegisterRamReset`, `SoftReset`
- DMA channels 0–3 (immediate, HBlank, VBlank, special).
- Timers 0–3, including cascade.
- Interrupt controller: IE, IF, IME, IRQ dispatch.

**Done when:** a homebrew ROM runs its startup code and reaches a known loop; `gba-tests` CPU tests mostly pass.

---

### M4 — PPU (3–4 weeks)

Render in this order. Each step is visible.

1. **Mode 3** — 240×160 direct-color bitmap. Easiest path to a real image.
2. **Mode 4** — 8-bit indexed bitmap with palette.
3. **Modes 0–2** — tiled backgrounds, text and affine.
4. **Sprites** — OAM parsing, 1D/2D mapping, priority, flipping, affine.
5. **Effects** — windows, blending (alpha/brightness/darkness), mosaic.
6. **Mode 5** — small bitmap mode.

Start with a scanline renderer, not a dot-level renderer. Add dot timing only if a game demands it.

**Done when:** a test ROM draws backgrounds and sprites correctly, and your first commercial game reaches its title screen.

---

### M5 — First game, input, saves (1–2 weeks)

- Boot a legally dumped commercial ROM you own.
- Fix the CPU/PPU bugs it exposes, one at a time.
- Input latency under two frames.
- Backup media: SRAM, Flash 64K/128K, EEPROM 512B/8K.
- Persist saves to disk with a per-game filename.

**Done when:** you can start a new game, play for ten minutes, save, quit, relaunch, and continue.

This milestone is the emotional payoff. Record a video of it.

---

### M6 — Audio (1–2 weeks)

- Four GB-style channels: two square, wave, noise.
- Two GBA DMA sound channels (A/B), 8-bit and 16-bit.
- Frame-sequencer timing.
- Resample to 48 kHz and feed `cpal` through a lock-free ring buffer.
- Volume, mute, and a simple low-pass filter.

**Done when:** music and sound effects play at the right pitch and speed, with no crackling during normal play.

**Rabbit hole warning:** "perfect" APU mixing is a trap. Get it close, then move on.

---

### M7 — Accuracy, debugger, polish (2–4 weeks)

- Run `mgba-emu/suite` and `jsmolka/gba-tests`; fix failures by category.
- Debugger:
  - Registers, CPSR, and disassembly around PC
  - Breakpoints and step / step-frame
  - Memory hex viewer and editor
  - Tile, palette, background-map, and OAM viewers
  - Layer toggles
  - DMA, timer, IRQ, and PPU state
- Save states with multiple slots.
- Rewind buffer (optional, but great for debugging).
- Frame limiter, fast-forward, pause.
- CI: `fmt`, `clippy -D warnings`, tests, headless boot smoke test.

**Done when:** the definition of done below is true.

---

## First three commits

Each commit should build, test, and be understandable on its own.

### Commit 1 — `chore: scaffold workspace and framebuffer window`

- Create the workspace and both crates.
- Add `winit`, `pixels`, `clap`, `anyhow`, `thiserror`.
- Open a 240×160 framebuffer scaled 4x.
- Draw color bars.
- Add a 59.7275 Hz frame loop and an FPS counter in the title.
- Esc quits; `S` saves a screenshot.

**Done when:** `cargo run` shows the bars at a stable frame rate, and `cargo test` passes.

### Commit 2 — `feat(core): bus, memory map, and cartridge loader`

- `Bus` with region dispatch and read/write helpers.
- `Cartridge::from_rom(&[u8])`.
- Parse title, game code, maker code, ROM size, and header complement check.
- Map ROM at `0x08000000`.
- `--info` prints the header.
- Unit tests with a synthetic 192-byte header.

**Done when:** `cargo run -- --info rom.gba` prints sane metadata, and the header tests pass.

### Commit 3 — `feat(cpu): ARM register file and data-processing instructions`

- `Cpu` state: 16 registers, CPSR, current mode.
- ARM fetch/decode for data-processing instructions.
- Barrel shifter and condition flags (N, Z, C, V).
- `MOV`, `MVN`, `ADD`, `ADC`, `SUB`, `SBC`, `RSB`, `CMP`, `CMN`, `AND`, `ORR`, `EOR`, `BIC`, `TST`, `TEQ`.
- A `TestBus` backed by 64 KB of RAM.
- Table-driven tests for results and flags.

**Done when:** the opcode tests pass and `cargo clippy -- -D warnings` is clean.

Commit 4 is branches; commit 5 is load/store; commit 6 is Thumb. Keep the commits small enough that you can bisect a CPU bug later.

---

## Schedule

Treat this as a **4–6 month evenings project**, or **12–16 focused weeks** if you can put real hours in.

| Week | Focus | Demo |
|---|---|---|
| 1 | M0 + M1: CHIP-8, window, frame loop | Color bars at 60 fps |
| 2–4 | M2: ARM + Thumb CPU, tests | `armwrestler` passing |
| 5–6 | M3: bus, cart, BIOS HLE, DMA, timers, IRQ | Homebrew ROM reaches its loop |
| 7–9 | M4: mode 3 → 4 → 0–2 → sprites → effects | First title screen |
| 10 | M5: input, saves, first game | Ten minutes of gameplay |
| 11 | M6: audio | Music and SFX |
| 12–14 | M7: test suites, debugger, save states | Debugger screenshot |
| 15–16 | Buffer, polish, demo video | The tweet |

If a milestone slips, cut scope, not sleep. The project is supposed to be fun.

---

## Test strategy

- **CPU unit tests:** table-driven, one test per instruction family and flag edge case.
- **CPU fuzzing:** `FuzzARM` plus `proptest` for random instruction streams.
- **PPU golden tests:** run a test ROM for N frames, hash the framebuffer, compare against a known-good image.
- **Integration tests:** boot test ROMs headlessly in CI; no window, no audio device.
- **Compatibility tests:** keep a small list of games and a checklist of what works.
- **Benchmarks:** cycles per frame, frame time, audio callback time.

CI should run: `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`, and a headless boot smoke test.

---

## Hardware cheat sheet

| Thing | Value |
|---|---|
| CPU | ARM7TDMI, 16.78 MHz |
| Screen | 240×160, BGR555 |
| Frame rate | 59.7275 Hz (280,896 cycles/frame) |
| BIOS | 16 KB at `0x00000000` |
| EWRAM | 256 KB at `0x02000000` |
| IWRAM | 32 KB at `0x03000000` |
| I/O | `0x04000000` |
| Palette | 1 KB at `0x05000000` |
| VRAM | 96 KB at `0x06000000` |
| OAM | 1 KB at `0x07000000` |
| ROM | up to 32 MB at `0x08000000` |
| Save | up to 128 KB at `0x0E000000` |
| DMA | 4 channels |
| Timers | 4, with cascade |
| Backgrounds | 4 layers |
| Sprites | 128, up to 64×64 |
| APU | 4 GB channels + 2 DMA sound channels |
| PPU modes | 0–5 |

If doing the DMG side quest: 4.194304 MHz, 160×144, 4 shades, 70,224 cycles/frame, MBC1/2/3/5.

---

## Resources

**Documentation**

- GBATEK — the GBA hardware bible: <https://problemkaputt.de/gbatek.htm>
- TONC — GBA programming, graphics, and hardware: <https://coranac.com/tonc/text/toc.htm>
- Pan Docs — original Game Boy reference: <https://gbdev.io/pandocs/>
- ARM7TDMI datasheet (ARM DDI 0210)
- gbadev forum and Discord — search before you suffer

**Test ROMs**

- `mgba-emu/suite` — <https://github.com/mgba-emu/suite>
- `jsmolka/gba-tests` — <https://github.com/jsmolka/gba-tests>
- `destoer/armwrestler` — <https://github.com/destoer/armwrestler>
- `DenSinH/FuzzARM` — <https://github.com/DenSinH/FuzzARM>
- AGS Aging Cartridge — hardware stress tests

**Reference emulators**

- mGBA (C) — the compatibility gold standard
- NanoBoyAdvance (Rust) — excellent reference for GBA accuracy
- SkyEmu (Rust) — useful for debugging workflows
- VBA-M — old but readable

Read for understanding, not for copy-paste. Keep your implementation yours.

---

## Crates

| Purpose | Crate |
|---|---|
| Window and input | `winit` |
| Framebuffer | `pixels` (or `softbuffer`) |
| Audio output | `cpal` |
| Lock-free audio queue | `rtrb` |
| CLI | `clap` |
| Errors | `thiserror` (core), `anyhow` (frontend) |
| Logging | `tracing`, `tracing-subscriber` |
| Debugger UI | `egui`, `egui-winit` |
| Save states | `serde`, `bincode` |
| Bitflags | `bitflags` |
| Tests | `proptest`, `insta`, `criterion` |
| Gamepad (stretch) | `gilrs` |
| WASM (stretch) | `wasm-bindgen`, AudioWorklet, WebGL2 |

Keep `gba-core` dependencies tiny. No async runtime. No `tokio`. The emulator is a loop, not a web service.

---

## Debugger wishlist

Build these in order after the first game boots:

- [x] Registers, CPSR, and mode
- [x] Disassembly around PC
- [x] Breakpoints, step, step-frame
- [x] Headless debugger CLI
- [x] Memory hex viewer and editor
- [x] Tile viewer (terminal)
- [x] Palette viewer
- [x] Background map viewer (register view)
- [x] OAM / sprite viewer
- [x] Layer toggles
- [x] PPU, DMA, timer, and IRQ state
- [x] Save-state slots
- [x] Rewind buffer

A good debugger turns a 3-hour bug into a 10-minute bug. But only after you have bugs worth debugging.

---

## Stretch goals

- Real BIOS support (user-supplied, never committed)
- GameShark / Action Replay cheats
- Link cable / multiplayer
- Netplay
- LCD shaders (grid, ghosting, color correction)
- WASM build in the browser
- Gamepad support
- ROM library UI with cover art
- Rewind and fast-forward
- Automated compatibility dashboard

---

## Risks and rabbit holes

| Risk | Mitigation |
|---|---|
| PPU timing consumes you | Scanline renderer first; dot timing only when a game needs it |
| APU mixing perfectionism | Get pitch and speed right; refine later |
| Save-type detection | Signature scan + manual CLI override |
| BIOS legal/distribution issues | HLE first; real BIOS stays local |
| Scope creep | No link cable, netplay, or shaders until v1 |
| Accuracy obsession | Compatibility-driven; test ROMs, not perfection |
| Burnout | Demo every milestone; keep a known-good branch |
| "Just one more refactor" | Ship the first game before restructuring |

---

## Definition of done (v1)

- [ ] A legally dumped commercial game boots and plays at full speed.
- [ ] Audio works: music and SFX at the right pitch and speed.
- [x] Saves persist across restarts (SRAM/Flash/EEPROM).
- [ ] Save states work across runs (in-memory slots and rewind are implemented).
- [ ] `armwrestler`, `FuzzARM`, and most of `gba-tests` pass (`gba-tests` does).
- [ ] A meaningful slice of `mgba-emu/suite` passes.
- [ ] Stable 59.73 fps with no audio crackling.
- [ ] Input latency under two frames.
- [x] Debugger is usable for CPU and PPU bugs.
- [x] No copyrighted ROMs or BIOS in the repo.
- [ ] A demo video that makes people say "you wrote that?"

---

## Current status and next work

v0.2 and v0.3 are implemented, and the v0.4 frontend polish adds gamepad
support, shaders, config with keyboard presets, per-key remapping and volume,
cubic audio resampling, a cover-aware ROM library scanner, and a full browser
arcade with AudioWorklet sound, touch controls, IndexedDB saves, and WebGL
shaders. The remaining accuracy work is tracked in [`ROADMAP.md`](ROADMAP.md).

Run the validation and smoke suites:

```bash
./tools/validate.sh
./tools/smoke.sh
```

Use `./tools/soak.sh <rom.gba> 216000` for a manual one-hour emulated soak.

Remaining accuracy work is tracked in [`ROADMAP.md`](ROADMAP.md): dot-level PPU
effects, exact SOUNDBIAS PWM, and a commercial-game soak.

---

*The project is a marathon disguised as a sprint. The trick is to make every week demoable.*
