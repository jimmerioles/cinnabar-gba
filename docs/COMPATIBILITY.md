# Compatibility

Cinnabar's automated suite covers the CPU, memory map, backup media, PPU test
ROMs, the built-in cartridges, and a Celeste Classic smoke test when the
homebrew ROM is present.

## Automated coverage

| Area | Coverage | Status |
|---|---|---|
| ARM instruction set | `gba-tests/arm` | passing |
| Thumb instruction set | `gba-tests/thumb` | passing |
| Memory map and mirrors | `gba-tests/memory` | passing |
| SRAM | `gba-tests/save/sram` | passing |
| Flash 64K | `gba-tests/save/flash64` | passing |
| Flash 128K | `gba-tests/save/flash128` | passing |
| No backup | `gba-tests/save/none` | passing |
| PPU modes 0-5 | `gba-tests/ppu`, built-in cartridges | passing |
| Audio | APU unit tests, Celeste Classic | passing |
| BIOS HLE | SWI unit/integration tests | passing |
| Frontend | headless smoke, self-test | passing |
| Browser arcade | WASM build, audio worklet, gamepad/touch, IndexedDB saves, WebGL shaders | passing (manual) |

Run the suite with:

```bash
./tools/validate.sh
```

The script fetches `gba-tests` into the gitignored `test-roms/` directory. It
also runs Celeste Classic when `test-roms/homebrew/celeste.gba` is present.

## Boot smoke tests

```bash
./tools/smoke.sh
```

This boots every bundled cartridge headlessly and checks that the frontend and
core agree on a sane frame. It runs in CI.

## Verified commercial titles

These were checked against legally dumped images; commercial ROMs are not part of
the repository. Put local copies in the gitignored `test-roms/commercial/`
directory.

| Title | Region | Save type | Coverage |
|---|---|---|---|
| The Legend of Zelda: The Minish Cap | Europe (BZMP) | EEPROM 8K | boot, intro, file select, new-file creation, save reload, gameplay, audio |
| Pokémon FireRed Version | US (BPRE) | Flash 128K | boot, flash-ID detection, New Game, save, overwrite, reload, gameplay, audio |

The `drive` example replays a scripted input sequence and writes one PNG per
step, which makes commercial boot and gameplay checks reproducible:

```bash
cargo run -p cinnabar-core --example drive -- \
  test-roms/commercial/minish-cap.gba /tmp/minish 900:0 5:8 120:0
```

Each argument is `<frames>:<keys-hex>`.

`audio_dump` writes a 16-bit stereo WAV for audio inspection:

```bash
cargo run -p cinnabar-core --example audio_dump -- \
  test-roms/commercial/pokemon-firered.gba /tmp/pokemon.wav 900
```

## Long-run soak

Use a legally dumped ROM you own:

```bash
./tools/soak.sh /path/to/game.gba 216000   # about one hour of emulated time
```

The script runs headlessly, checks that the process survives, and writes a
final screenshot. A one-hour emulated run is a manual release check rather than
a CI job.

## Per-game checklist

Copy this template when testing a commercial ROM:

```text
Title:
ROM version / region:
Save type:
Boot:
Title screen:
Gameplay:
Audio:
Save and reload:
30-minute stability:
Known PPU/timing issues:
```

## Known limitations

- BIOS internals are HLE, not a cycle-accurate BIOS.
- PPU timing is scanline-based. Dot-level effects and exact HBlank/DMA timing
  are still approximate.
- Flash and EEPROM command handling covers the common AMD/bit-banged paths;
  unusual command sequences may still differ.
- EEPROM size is inferred from the first DMA transfer (6-bit or 14-bit address).
- SOUNDBIAS amplitude resolution is modelled; the exact PWM output stage is not.
- No link cable, netplay, or save-state files outside the debugger slots yet.
- The browser arcade targets modern browsers with WebGL2 and AudioWorklet. It
  falls back to a 2D canvas and silent playback when those are unavailable.
