# Cinnabar GBA, explained

A 3:13 house-style explainer film about the emulator, made with the
[`explainer-kit`](../../../explainer-kit) pipeline.

| | |
|---|---|
| Runtime | 3:13 (193.22 s) |
| Format | 1920 × 1080, 30 fps, H.264 High, AAC 192k |
| Voice | Kokoro `af_heart`, speed 0.88, no fallback |
| Mix | -16.0 LUFS integrated, -3.2 dBFS true peak |
| Music | House `tech` score, 100 BPM, A major, seed 1, MIT |
| Chapters | 10 |

The film follows one 240 × 160 frame: the Cinnabar logo cartridge, captured
from the headless core. It builds the machine around that frame, then returns to
it at the end.

The deliverable is `build/film.mp4`, with `build/poster.jpg` as the thumbnail and
`build/script.md` as the companion script. `preview.mp4` is the 23-second trailer
embedded in the project README. The README player uses a GitHub user-attachment
URL because README video tags do not accept repo-hosted MP4s. The MP4 is a
release asset, not a Git commit. `poster.jpg` and every source file are committed.

## Source

| File | Role |
|---|---|
| `script.json` | narration, chapter titles and shot notes |
| `scenes.js` | the argument, drawn with the kit's canvas primitives |
| `assets.js` | names for the real captures |
| `assets/` | Cinnabar logo, Celeste Classic, browser arcade |
| `preview.mp4` | 23 s trailer used by the README player |
| `config.json` | title, accent, chrome, voice, bed, verify frame |
| `music.m4a`, `music.json` | the house bed and its provenance |
| `golden/frame.png` | the frame `verify` compares |
| `poster.jpg` | the committed thumbnail |

## Rebuild

From the project root:

```bash
KIT=~/Work/jimmerioles/explainer-kit
node $KIT/bin/explainer.mjs narrate
node $KIT/bin/explainer.mjs golden
node $KIT/bin/explainer.mjs render
node $KIT/bin/explainer.mjs mix
node $KIT/bin/explainer.mjs master
node $KIT/bin/explainer.mjs verify
```

`narrate` caches every sentence by a hash of its text and voice settings. If a
sentence changes, remove `build/voice/` before narrating so the per-index WAVs
are rebuilt from the cache.

The kit version is pinned to 1.1.0 in `config.json`. The final gate report is
`build/report.json`; all twelve checks pass.

## Captures

The GBA frames are real 240 × 160 screenshots from the headless core:

- `logo` demo cartridge, frame 6, for the object and the scanline chapter.
- Celeste Classic, frame 500, for the proof chapter.
- The deployed browser arcade, cropped to the console.

The validation chapter shows the real `tools/validate.sh` output as type, not as
a screenshot.

Celeste Classic is an open-source homebrew port fetched by
`tools/fetch-celeste.sh`; it is not bundled. The film contains no commercial
game footage and no Nintendo BIOS.

## Legal

Cinnabar GBA is MIT licensed. Game Boy Advance is a trademark of Nintendo.
This film is an independent project explainer and is not affiliated with or
endorsed by Nintendo.
