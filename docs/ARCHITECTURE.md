# How Cinnabar works

A guided tour of the emulator, from cartridge to framebuffer. Read this first if
you want a mental model before diving into the source.

## The short version

Cinnabar is a Game Boy Advance emulator written in Rust. It recreates the
console's chips as Rust structs, then runs them in a loop fast enough to look
like a real console: about 60 frames per second, each frame 240×160 pixels with
stereo sound.

A real GBA is several chips sharing one memory bus. Cinnabar gives each chip the
same job in software, and a conductor called `System` lets them take turns.

## The pieces at a glance

| Piece | What it does on real hardware | Where it lives |
|---|---|---|
| CPU | Fetches and executes ARM or Thumb instructions | `crates/cinnabar-core/src/cpu/` |
| Bus | Routes every memory read/write to the right chip | `bus.rs` |
| Cartridge | Reads the ROM header and detects the save chip | `cart.rs` |
| PPU | Draws backgrounds, sprites, windows, and effects | `ppu.rs` |
| APU | Produces the four GB sound channels plus DMA audio | `apu.rs` |
| DMA | Bulk-copies memory without the CPU | `dma.rs` |
| Timers | Count cycles and raise interrupts | `timer.rs` |
| IRQ | The interrupt controller ("tap the CPU on the shoulder") | `irq.rs` |
| Keypad | Button state and keypad interrupts | `keypad.rs` |
| BIOS HLE | Fakes the useful firmware calls | `bios.rs` |
| Save media | SRAM, Flash, or EEPROM behind the cartridge | `save.rs` |
| System | Owns everything and runs one frame | `system.rs` |

The **bus** is the piece that makes the whole thing click. The CPU never touches
hardware directly. It asks the bus for an address, and the bus looks at the top
byte of that address and forwards the request:

- `0x02` EWRAM, `0x03` IWRAM: work RAM
- `0x04` I/O registers (PPU, APU, DMA, timers, keypad)
- `0x05` palette, `0x06` VRAM, `0x07` sprite memory
- `0x08..0x0D` cartridge ROM
- `0x0E`/`0x0F` save chip

That is why `bus.rs` is the center of the core: it is the switchboard.

## One frame, step by step

The core is deliberately headless. It owns no window and no audio device. The
entire machine is driven by one call: `System::run_frame()`.

```text
frontend tick
  -> System::run_frame()
       repeat System::step():
         -> Cpu::step()        execute one ARM or Thumb instruction
         -> Bus::step(cycles)  advance timers, APU, PPU, DMA, IRQs
       until the PPU finishes 228 scanlines
  <- 240x160 framebuffer + audio samples
  -> window or canvas, and the speaker
```

In a little more detail:

1. `run_frame` clears the "frame done" flag and loops.
2. `Cpu::step` runs one instruction and reports how many cycles it took. ARM
   instructions are 4 bytes and Thumb instructions are 2 bytes. `r[15]` always
   points at the next instruction, with the usual pipeline bias of +4 or +2.
3. `Bus::step` advances the peripherals by those cycles. It batches work in
   32-cycle chunks for speed, then steps the timers, APU, PPU, and any DMA that
   the PPU's HBlank/VBlank events triggered.
4. If an interrupt is pending and enabled, the CPU takes it before the next
   instruction.
5. If the game executed `HALT`, the system idles until an interrupt or VBlank
   wakes it. That is how games save battery.
6. After 160 visible lines and 68 VBlank lines, the PPU sets `frame_ready`.
   `run_frame` clones the framebuffer and drains the audio samples.
7. The frontend presents those pixels and schedules the next frame roughly
   16.74 ms later. The native app does this on a fixed cadence and can run four
   frames per tick in fast-forward.

Because a frame is exactly 280,896 CPU cycles at 16.78 MHz, the core is
deterministic. Given the same inputs, it produces the same frames. That is what
makes headless screenshots, tests, and rewinding possible.

## The four crates

- **`cinnabar-core`**: the brain. Pure logic, no I/O, no filesystem. It is fully
  driven by `run_frame`, `set_keys`, and the save-state API, which makes it easy
  to test.
- **`cinnabar-frontend`**: the native shell. `winit` for the window,
  `softbuffer` for pixels, `cpal` for audio, plus keyboard/gamepad input, config,
  screenshots, the debugger, headless mode, and save files.
- **`cinnabar-wasm`**: a thin `wasm-bindgen` bridge. JavaScript owns the canvas,
  audio worklet, input, and browser storage; Rust only moves frames, samples,
  and save bytes across the boundary.
- **`cinnabar-testrom`**: small original GBA ROMs written in ARM assembly,
  assembled with `llvm-mc`, and checked into the repo. They boot without any
  commercial ROM and drive the demos and integration tests.

The dependency arrow only ever points one way: frontend and wasm depend on core,
never the reverse. That is why the same emulator can run in a desktop window and
in a browser tab.

## Why it does not need a BIOS file

A real GBA starts by running Nintendo's BIOS. Cinnabar uses **HLE** (high-level
emulation): when the game calls a BIOS function with a `SWI` instruction,
`bios.rs` implements the useful ones directly in Rust. That includes
decompression (LZ77, RLE, Huffman), `CpuSet`/`CpuFastSet`, division, square
root, affine matrix setup, and the interrupt-wait calls. Then it jumps straight
into the cartridge. That is why the repo ships no copyrighted firmware.

## Saves and save states

Two different things, often confused:

- **Battery saves** mimic the cartridge's SRAM, Flash, or EEPROM chip. The save
  type is detected from the ROM header, and the bytes are written to
  `<rom>.sav` next to the ROM (or wherever `--save` points). In the browser they
  go into IndexedDB instead.
- **Save states** are snapshots of the whole mutable machine: CPU, RAM, PPU,
  APU, DMA, timers, interrupts, and save media. They are serialized with
  `serde` and `bincode`, stamped with a magic value and version, and checked
  against the cartridge so you cannot load a state into the wrong game. The
  debugger uses these for rewinding.

## Ways to run it

```bash
make demo       # bundled logo cartridge, native window
make selftest   # no ROM, no window: verifies the core wiring
make debug      # headless debugger with breakpoints and stepping
make serve      # build the WASM core and serve the browser arcade
```

You can also point it at a legally dumped ROM:

```bash
cargo run -p cinnabar-frontend -- /path/to/game.gba
cargo run -p cinnabar-frontend -- /path/to/game.gba --headless --frames 600 --screenshot shot.png
cargo run -p cinnabar-frontend -- /path/to/game.gba --info
```

## Where to look first

If you want to build a mental map, read in this order:

1. **`crates/cinnabar-core/src/system.rs`**: about 100 lines, and the whole
   heartbeat is there.
2. **`crates/cinnabar-core/src/bus.rs`**: the memory map and peripheral
   scheduling.
3. **`crates/cinnabar-core/src/cpu/mod.rs`**: registers, modes, and the step
   function.
4. **`crates/cinnabar-core/src/ppu.rs`**, starting at `render_line`: how one
   scanline becomes pixels.
5. **`crates/cinnabar-frontend/src/app.rs`**, starting at `about_to_wait`: how a
   real window pumps the core.

And if you want to change something, the mapping is roughly:

| You want to change... | Edit... |
|---|---|
| Frame timing or the run loop | `crates/cinnabar-core/src/system.rs`, `crates/cinnabar-frontend/src/app.rs` |
| An instruction bug | `crates/cinnabar-core/src/cpu/arm.rs` or `thumb.rs` |
| Graphics behavior | `crates/cinnabar-core/src/ppu.rs` |
| Sound | `crates/cinnabar-core/src/apu.rs` |
| A game that will not save | `crates/cinnabar-core/src/save.rs`, `crates/cinnabar-core/src/cart.rs` |
| The desktop experience | `crates/cinnabar-frontend/src/` |
| The browser arcade | `crates/cinnabar-wasm/src/lib.rs`, `web/` |

The safety nets while exploring are `make test` and `make selftest`. The
hand-written test ROMs in `crates/cinnabar-testrom/roms/` are also readable
assembly if you want to see exactly what the emulator is being asked to do.

## See also

- [`PLAN.md`](../PLAN.md): the original design plan and milestones
- [`ROADMAP.md`](../ROADMAP.md): what shipped and what comes next
- [`docs/COMPATIBILITY.md`](COMPATIBILITY.md): test coverage and verified titles
- [`README.md`](../README.md): usage, controls, and configuration
