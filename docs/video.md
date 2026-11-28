# Video

## Two renderers

Both take the chosen `thumbnail.png` and the mastered `audio.mp3` and write `video.mp4`.

### ffmpeg (fast, explicit)

`media::assemble_video` builds a 1080p30 video in about two minutes. Slow zoom, soft sprite particles, waveform band and fades. It is deterministic and needs no Node.

```bash
melodias video output/<slug>
# or
VIDEO_RENDERER=ffmpeg make generate URL='...' VIBE='...'
```

It is used only when requested. The pipeline does not fall back to it.

### Remotion (default)

The `video/` project renders a richer composition in about twelve minutes:

- Slow zoom on the still.
- Bokeh and spark overlays from `assets/videos/`, screen blended and selectable with `VIDEO_PARTICLES` (`both` by default, `bokeh` or `sparks`).
- A bird layer clipped to the sky.
- The same `showwaves` waveform line as the fast renderer, rendered by ffmpeg as a screen-blended overlay.
- A subtle channel label and fades.

The pipeline uses this renderer at the end of every run. If `video/node_modules` or `pnpm` is missing, the run fails in the preflight, before the GPU work, with the command to fix it. There is no silent fallback.

```bash
cd video
pnpm install
cd ..
melodias render output/<slug>
```

`melodias render` copies the still and track into `video/public/current/`, writes `video/props.json` with the duration, channel and particles, renders the waveform overlay from the audio, and runs `pnpm exec remotion render` with four workers. The previous `video.mp4` is kept as `video-ffmpeg.mp4`.

## Short (TikTok vertical)

`melodias short` cuts a 1080x1920 TikTok/Shorts version out of a finished run, with ffmpeg only and no LLMs. It picks the loudest window with `ffmpeg ebur128` unless `--start` is given, then assembles slow zoom, dark bands, waveform and channel titles.

```bash
melodias short output/<slug>
# or
make short DIR=output/<slug>
make short DIR=output/<slug> ARGS="--sec 32 --start 103 --platform tiktok"
```

It writes `short.mp4` plus `work/short-audio.mp3` and records `video.short` with `start_sec` in `meta.json`. It also writes `short.txt` with paste-ready English captions: TikTok and YouTube Shorts sections by default, or a single section with `--platform tiktok|shorts|reels`. The caption reuses the stored YouTube title, one context line from the vibe and at most five discovery hashtags, so the long YouTube description is never pasted as is.

The 1080p overlay clips live in `video/public/overlays/` and are committed. The 4K masters in `assets/videos/` are local only and ignored by git.

## Setup notes

- Node 24+ and pnpm are required by default; only the explicit ffmpeg path avoids them.
- `pnpm-workspace.yaml` allows esbuild's build script, which pnpm blocks by default.
- The first render downloads a Headless Shell for Remotion.
- Remotion writes a ~120 MB file for a three minute track, compared to ~42 MB for the ffmpeg version.

## Why a sprite for the ffmpeg path

`drawbox` positions are evaluated once, which produced static squares. The overlay filter with `eval=frame` evaluates the position on every frame, so the particles move. The sprite is a 128x128 radial alpha generated with ffmpeg `geq`, tinted per particle with `colorchannelmixer`.

## Options for richer effects

| Option | What it gives | Cost |
| --- | --- | --- |
| **Remotion** (implemented) | Animated titles, transitions, branded intros and outros, audio-reactive scenes, any CSS/SVG effect | Node project, ~12 min render |
| **Motion Canvas / Revideo** (TS) | Canvas animation with a scene graph | Another rendering pipeline |
| **Kdenlive, Shotcut, DaVinci** | Full manual editing for one-offs | Manual, not reproducible |
| **Blender, MLT, VapourSynth** | Scripted compositing, 3D scenes | Heavy, complex setup |

## Open items

- The assembly agent that picks a preset from the vibe is still a design item.
- Title and channel overlays are fixed; they can move to `config.toml` when the video gets more settings.
