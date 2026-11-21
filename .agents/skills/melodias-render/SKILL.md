---
name: melodias-render
description: Render, re-render or diagnose the video of a finished melodias-studio run, including the Remotion default, the explicit ffmpeg fast path, particles and overlay requirements, meta.json video fields and the render commands. Use this whenever a run has no video, the Remotion render fails or takes too long, video.mp4 or video-ffmpeg.mp4 looks wrong, overlays or particles are missing, or you need to choose between make render, make video and make image.
---

# Melodias video rendering

There are two renderers and no silent fallback. Remotion is the default because it produces the
channel look; ffmpeg is an explicit fast path. If Remotion is selected and its project is missing,
the stage fails with the fix command instead of quietly rendering with ffmpeg. Keep that
behavior: a silent fallback would make two runs look different for no visible reason.

Renderer selection comes from `VIDEO_RENDERER` (`remotion` or `ffmpeg`, default `remotion`).
`melodias render` forces Remotion regardless of the setting, and `melodias video` forces ffmpeg.

## Commands

Run these from the repository root. `VIDEO_PROJECT` is resolved against the current directory,
so a render started from elsewhere looks for the wrong project.

| Need | Command | Renderer |
| --- | --- | --- |
| Re-render with the channel look | `make render DIR=output/<slug>` | Remotion, always |
| Fast re-render while iterating | `make video DIR=output/<slug>` | ffmpeg, always |
| New thumbnail and then a video | `make image DIR=output/<slug>` | configured renderer |
| Same without make | `cargo run --release -- render output/<slug>` | Remotion |
| Same without make | `cargo run --release -- video output/<slug>` | ffmpeg |

`melodias render` and `melodias video` accept either a real path or a path relative to `OUT_DIR`.
Both require `thumbnail.png` and `audio.mp3` in the run directory and exit with code 2 if either
is missing.

## Remotion path

`video::render` (pipeline stage 11) and `video::render_remotion` (`melodias render`) share the
same steps:

1. `check_remotion` verifies `VIDEO_PROJECT/node_modules`, `pnpm` on PATH, and every overlay
   required by `VIDEO_PARTICLES`.
2. Copies the still and track into `video/public/current/` as `thumbnail.png` and `audio.mp3`.
3. Runs `media::waveform_overlay`, which writes `waveform.mp4` next to them.
4. Writes `video/props.json` with `durationSec`, `channel: "melodias8bit"` and `particles`.
5. Runs:
   `pnpm exec remotion render src/index.ts MelodiasVideo <output> --props=video/props.json --concurrency=4`
6. Renders to `work/video-remotion.mp4` first. On success it renames that file to `video.mp4`.
   On failure it deletes the temp file and returns the error.

A full Remotion render takes about 12 minutes. The composition always adds the birds layer; the
particles layer is the only part selected by `VIDEO_PARTICLES`.

Overlay requirements in `video/public/overlays/`:

| `VIDEO_PARTICLES` | Files |
| --- | --- |
| `both` (default) | `particles-bokeh.mp4`, `particles-sparks.mp4`, `birds.mp4` |
| `bokeh` | `particles-bokeh.mp4`, `birds.mp4` |
| `sparks` | `particles-sparks.mp4`, `birds.mp4` |

`both` is also used when the value is empty. Anything else fails with a message naming the
accepted values. The overlays are assets, not generated files; do not create placeholders to get
past the check.

`video/public/current/` and `video/props.json` are gitignored. They are overwritten on every
render, so never edit them by hand or read them as a source of truth.

## ffmpeg path

`media::assemble_video` builds the fast video directly:

- Even width and height, minimum 320x240, default 1920x1080.
- Still image with a slow `zoompan` (1.00 to 1.08), 30 fps.
- A dark band at the bottom and a `showwaves` waveform in `0x56d4dd` at 55% opacity.
- Particle sprite overlay chosen by `VIDEO_PARTICLES`.
- Fade in from 0 s for 1.5 s and fade out for the last 3 s.
- `libx264`, `veryfast`, CRF 21, AAC 192k, `+faststart`, trimmed to the audio duration.

The track must be at least 5 seconds long. The output is written directly to `video.mp4`,
overwriting the previous file. Unlike the Remotion path, the ffmpeg subcommand does not update
`meta.json`, so after `make video` the `video.renderer` field can still say `remotion`. Update it
if a later step depends on that field.

## File layout after a render

```text
output/<slug>/
  video.mp4            current render
  video-ffmpeg.mp4     previous video, kept when Remotion replaces one
  work/
    video-remotion.mp4 temp target; removed on pipeline failure, may remain after `melodias render` fails
    waveform.mp4       Remotion input overlay
video/
  public/current/      thumbnail, audio and waveform copied in
  props.json           durationSec, channel, particles
```

## Diagnosing a failure

| Message | Cause | Fix |
| --- | --- | --- |
| `the Remotion project is not installed` | `node_modules` missing | `cd video && pnpm install` |
| `pnpm is not on PATH` | pnpm missing | install pnpm or use `VIDEO_RENDERER=ffmpeg` |
| `missing Remotion overlay` | particle overlay not in `public/overlays/` | add the asset or pick another `VIDEO_PARTICLES` |
| `VIDEO_PARTICLES must be both, bokeh or sparks` | invalid value | fix the config or environment |
| `VIDEO_RENDERER must be remotion or ffmpeg` | invalid value | fix the config or environment |
| `the track is ... too short` | audio under 5 s | regenerate the track |
| `the Remotion renderer failed` | pnpm or Remotion error | read the run log; the child output is attached |

Preflight runs before the GPU stages, so a missing project or overlay fails before any image or
audio work. If a run fails at stage 11 after a long GPU session, the failure is usually in the
Remotion command itself, not in the preflight.

Child process output goes to the run log through `media::attach_output`. In the TUI the log is
`output/run-<unix>.log`; headless uses `MELODIAS_LOG`. Do not replace `attach_output` with
inherited stdout or the TUI will be corrupted.

## Verification

- Check the file exists and is playable: `ffprobe -v error -show_entries format=duration -of csv=p=0 output/<slug>/video.mp4`.
- Check the resolution, renderer and particles:
  `jq '.video' output/<slug>/meta.json`.
- After a Remotion re-render, confirm the previous file was kept as `video-ffmpeg.mp4`.
- Run `make check` after touching `src/video.rs` or `src/media.rs`; there is no video test that
  exercises a real render, so the gate is the compiler plus unit tests.
