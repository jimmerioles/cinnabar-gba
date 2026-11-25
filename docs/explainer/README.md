# Cinnabar GBA, explained

A 3:13 house-style explainer film about the emulator, made with the
[`explainer-kit`](https://github.com/jimmerioles/explainer-kit) pipeline.

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

`build/film.mp4` is the edit master, `build/poster.jpg` is the thumbnail, and
`build/script.md` is the companion script. The MP4 is a release asset, not a Git
commit. `poster.jpg` and every source file are committed.

## Poster

The poster is a 16:9 crop of the browser arcade capture,
`docs/web-arcade.png`. It keeps the arcade cabinet, demo screen, and controls,
and it is also the video's first frame. Rebuild it from the project root:

```bash
ffmpeg -y -v error -i docs/web-arcade.png \
  -vf "crop=1440:810:0:540,scale=1920:1080:flags=lanczos" \
  -frames:v 1 -q:v 2 docs/explainer/poster.jpg
```

Copy it to `docs/explainer/build/poster.jpg` if the explainer kit expects the
build copy.

## Publish master

GitHub renders a bare user-attachment URL as a video player. It strips `poster`,
`autoplay`, `loop`, and `style` from hand-written `<video>` tags, so the opening
frame has to be the poster. The publish master holds the browser arcade still
for 2.5 s, crossfades over 0.5 s, then continues the film from 2.5 s. Audio is
copied untouched, so the duration, narration, music, and chapter timestamps do
not change.

Build the publish master from the project root:

```bash
POSTER=docs/explainer/poster.jpg
SOURCE=docs/explainer/build/film.mp4
OUT=docs/explainer/build/film-poster.mp4
FPS=30
OFFSET=2.5
XFADE=0.5
POSTER_DURATION=$(awk -v a="$OFFSET" -v b="$XFADE" 'BEGIN { print a+b }')

ffmpeg -y -v error \
  -loop 1 -framerate "$FPS" -t "$POSTER_DURATION" -i "$POSTER" \
  -i "$SOURCE" \
  -filter_complex "\
[0:v]scale=1920:1080:flags=lanczos,fps=$FPS,format=yuv420p,setsar=1,settb=AVTB,trim=duration=$POSTER_DURATION,setpts=PTS-STARTPTS[poster];\
[1:v]trim=start=$OFFSET,setpts=PTS-STARTPTS,fps=$FPS,format=yuv420p,setsar=1,settb=AVTB[film];\
[poster][film]xfade=transition=fade:duration=$XFADE:offset=$OFFSET[v]" \
  -map "[v]" -map 1:a:0 -map_metadata 1 -map_chapters 1 -dn \
  -c:v libx264 -preset slow -crf 18 -pix_fmt yuv420p \
  -profile:v high -level 4.1 \
  -g $((FPS*2)) -keyint_min $((FPS*2)) -sc_threshold 0 \
  -c:a copy -movflags +faststart \
  "$OUT"
```

Upload the 1080p publish master with `gh issue create --attach` and put the bare
`https://github.com/user-attachments/assets/...` URL on its own line in the
README. The CLI accepts the roughly 38 MB file even though the web editor caps
video uploads at 10 MB. Do not wrap the URL in a link or a `<video>` tag, and do
not downscale the film to fit the web editor. After the temporary issue is
deleted, check that the asset still resolves with a 206 response and a
`video/mp4` content type.

## Source

| File | Role |
|---|---|
| `script.json` | narration, chapter titles and shot notes |
| `scenes.js` | the argument, drawn with the kit's canvas primitives |
| `assets.js` | names for the real captures |
| `assets/` | Cinnabar logo, Celeste Classic, browser arcade |
| `config.json` | title, accent, chrome, voice, bed, verify frame |
| `music.m4a`, `music.json` | the house bed and its provenance |
| `golden/frame.png` | the frame `verify` compares |
| `poster.jpg` | 16:9 browser arcade still, thumbnail and first frame |

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
