# Usage

## TUI

![melodias-studio TUI](assets/tui.png)

```bash
melodias
```

The reference URL is optional in the product vision but required by the current build. It is used only to condition timbre.

- `Tab` switches between the URL and the vibe.
- `Enter` moves to the next field, then starts the run.
- `q` quits, only once the run finished.

When the pools are ready the run pauses on a review screen. Pick one image and one audio draft with `1-9`, switch sections with `Tab`, and continue with `Enter`. The candidates live under `output/<slug>/candidates/`; open them to compare.

![Review screen](assets/tui-review.png)

Each stage shows its state, the progress bar tracks how many are done, and the log prints every step, including the prompt the LLM wrote and the seed of the take. A splash with the version appears for a moment on start.

## Headless run

Passing the reference URL and the vibe as arguments runs the same pipeline without the full screen UI. Each stage is printed as its own colored group with an icon, and tool output from `yt-dlp`, `ffmpeg` and `ssh` goes to the run log instead of the screen:

```bash
MELODIAS_LOG=output/run.log melodias 'https://www.youtube.com/watch?v=...' 'epic 8-bit orchestral finale'
```

The headless mode auto-selects the judge suggestion, or the best-scored pair when the judge is off, so use the TUI when you want to choose. The log receives the same lines as plain text, so it can be read later without escape codes. Quote the URL: a YouTube link copied from a playlist carries `&list=` and `&index=`, which the shell would otherwise split. The exit code is non-zero when a stage fails.

The image stage reports its own duration and the size of the PNG it got back, and warns when the server returned something other than the requested size.

## Video

See [video.md](video.md) for how the renderer works and which options exist for richer effects.

Every finished run leaves `video.mp4` next to the audio. If you have a run from before the video stage, assemble it without touching the GPU:

```bash
melodias video output/<slug>
# or
make video DIR=output/<slug>
```

The video is the chosen still with a slow zoom, glowing pixel particles, the audio waveform and edge fades, at `VIDEO_WIDTH` x `VIDEO_HEIGHT` (1920x1080 by default).

### Judge

Before the review, the run sends the three thumbnails and three 15 s clips to a multimodal model and preselects the best pair. The review screen opens with that image and audio selected and a yellow line with the score and reason. Press `Enter` to accept, or pick another pair.

The default backend is OpenCode Go with `gemini-4-argon`, which sees the images and hears the clips in a single call:

```bash
export OPENCODE_API_KEY="..."   # the key from OpenCode Console › Go
melodias
```

With `OPENCODE_API_KEY` absent it uses OpenRouter instead, but OpenRouter requires at least $0.50 in balance for audio inputs, even on the free omni model:

```bash
export OPENROUTER_API_KEY="sk-or-..."
JUDGE_PROVIDER=openrouter melodias
```

The keys can also live in `~/.config/melodias-studio/config.toml`. If neither is set, the judge is skipped and the manual review still works. `JUDGE_ENABLED=false` turns it off explicitly.

To review a finished run without regenerating anything:

```bash
melodias judge output/<slug>
# or
make judge DIR=output/<slug>
```

### Score

`melodias score <dir>` applies the deterministic rubric to a finished run without a GPU or a cloud call. It scores every thumbnail in the pool, every draft and the final pair, prints the per-candidate scores with the worst findings, and writes `work/score.json`.

```bash
melodias score output/<slug>
melodias score output/<slug> --json
# or
make score DIR=output/<slug>
```

The image side is palette, contrast at 168 px, text, glow and saturation. The audio side is loudness, true peak, spectral centroid, energy above 5 kHz, build, silence and length. The run score is 60 % technical and 40 % editorial (the judge verdict, when it exists). A run is accepted at 80 total, 85 technical and 70 editorial; without a verdict, the total equals the technical score.

The text check uses `tesseract`. Without it, `IMG_TEXT` is skipped and the image side is renormalized over the remaining metrics.

### Retry loop

The run scores the pool before the review and, when a side misses, regenerates only that side with a patched prompt. The patch is fixed data: `IMG_PALETTE` appends the limited-palette text, `IMG_GLOW` adds the neon negative and the natural-eye positive, and the audio codes append the chiptune, build or continuity text.

- The image side runs up to two rounds while sd-server is up; the audio side runs at most one when it lost eight points or more, independently of the image result.
- The same code never triggers twice, and the pool stops growing at nine candidates per side.
- `SCORE_ROUNDS` caps the rounds per side; set it to `0` to keep the phase 1 behavior and skip the loop.
- The review opens on the judge suggestion or, without one, on the best-scored pair. The log shows each patch, the scores after each round and why the loop stopped.
- A run that ends below the target is still saved and marked `accepted_by_limit`; `melodias history` and the dashboard show the outcome, and the run log says which side hit its budget.

### Reference library

To condition the thumbnails on a curated set, point `IMAGE_REFERENCES` at a folder (local or mounted SMB) and group the images in concept subfolders:

```bash
export IMAGE_REFERENCES=/mnt/owncloud/melodias-refs
melodias 'https://www.youtube.com/watch?v=...' 'epic 8bit battle theme'
```

The subfolder whose name matches the vibe (longest match) is used; `IMAGE_REFERENCE_CONCEPT=battle` forces one, and `IMAGE_REFERENCE_COUNT=3` changes how many go per generation. The references are sent as `extra_images` to sd-server, rotate between retry rounds, and are recorded per candidate in `meta.json`. The app never mounts the share or copies the images into the repository.

`melodias refs fetch --source pixabay|opengameart` fills the folder and `melodias refs import` imports your own or CC0 packs; both write `sources.jsonl` with the license of every image. See [configuration.md](configuration.md#building-a-library).

### History

Every finished run records itself in a local SQLite database after the video stage, so the next command does not need the run directory. `melodias history` shows the last 20 runs with their outcome (`accepted`, `accepted_by_limit` or `below_target`); `findings` ranks the codes by the points they cost; `trend` averages by day, week or month. `--json` and `--csv` feed a script, and the whole view only reads:

```bash
melodias history
melodias history findings
melodias history trend --by week
melodias history --json
# or
make history ARGS=findings
```

The file is `history.db` next to `config.toml`, and `MELODIAS_DB` overrides it. Runs made before the write existed are added with `make backfill`. See [configuration.md](configuration.md#history) for the schema, permissions and failure behavior.

### Dashboard

`melodias dashboard` serves a read-only page at `http://127.0.0.1:7373/` with the score over runs, the findings that cost the most, the outcome mix and the run table. The home also shows totals for runs, acceptance, spend, time, findings, lost points, candidates and retry patches. A left sidebar filters the runs table by outcome and the findings chart by source, and links to the latest and best run. Clicking a run opens its detail: thumbnail, video and audio, the judge verdict, the candidates with their scores and prompts, the findings, the applied patches, every file saved under the run directory and the raw `meta.json`, `score.json` and `verdict.json`. The media is served by the same process with byte-range support, so the video can seek. It opens the database for reading only, never creates or migrates it, and refreshes the summary once a minute; the detail refreshes on demand. It binds localhost only.

```bash
melodias dashboard
melodias dashboard --port 8080 --limit 500
# or
make dashboard
```

The page is served by the binary from `dashboard/`, with no Node, no build step and no network. `--limit` sets how many runs the charts and the API read (200 by default). The files are looked up under `OUT_DIR` by slug, so the detail also works for a run nested one level deeper, like `output/retry-check/<slug>/`. The URL `?run=<id>` opens a run detail directly, and the media links use inline content disposition, so a `.wav` plays in the browser instead of downloading.

See [dashboard/README.md](../dashboard/README.md) for screenshots and the endpoint list.

### Produced video with Remotion

For the richer composition (bokeh overlays, birds, audio-reactive bars, channel label):

```bash
cd video && pnpm install && cd ..
melodias render output/<slug>
```

That takes about twelve minutes and keeps the previous ffmpeg video as `video-ffmpeg.mp4`. See [video.md](video.md).

### Regenerate the thumbnail

To replace the chosen image of a finished run, reuse its prompt and seed, and rebuild the video without touching the rest:

```bash
melodias image output/<slug>
# or
make image DIR=output/<slug>
```

It keeps the old file as `thumbnail-old.png`, appends the new image to `candidates/`, updates `meta.json` and rebuilds `video.mp4`. The current prompt is sanitized so the eyes stay natural and the configured image negative prompt is applied.

### Chip covers (experimental)

`melodias chip <url|file>` renders an NES-style cover without touching the main pipeline. It downloads the audio, separates stems with Demucs, transcribes melody and bass with Basic Pitch, writes a FamiStudio project, renders it, and mixes a stereo master with echo, arpeggios, vibrato and portamento.

```bash
melodias chip 'https://www.youtube.com/watch?v=...' --title "Battle theme"
melodias chip song.wav --out /tmp/chip --keep-work
```

Output: `output/chip/<slug>/` with `audio.wav`, `audio.mp3`, `preview.mp3`, `project.txt` and `meta.json`.

It needs three things outside the repo, all user-local:

```bash
# Demucs + Basic Pitch in an isolated virtualenv
uv venv ~/.local/share/melodias-studio/venvs/audio8bit --python 3.11
uv pip install --python ~/.local/share/melodias-studio/venvs/audio8bit/bin/python \
  demucs basic-pitch 'numpy<2' 'tensorflow==2.15.0' 'setuptools<81'

# FamiStudio 4.5.4 and a local .NET 8 runtime
mkdir -p ~/.local/share/melodias-studio/tools
# unzip FamiStudio454-LinuxAMD64.zip into tools/famistudio
# install .NET 8 with dotnet-install.sh into tools/dotnet
```

Override the paths with `CHIP_VENV`, `FAMISTUDIO_BIN` and `DOTNET_ROOT`. The project text opens in FamiStudio for manual edits, and the CLI can export NSF or ROM from it.

## Make shortcuts

| Target | What it does |
| --- | --- |
| `make run` | Opens the TUI |
| `make install` / `make uninstall` | Installs or removes `~/.local/bin/melodias` |
| `make generate URL='...' VIBE='...'` | Runs headless and writes `output/run-<timestamp>.log` |
| `make image DIR=output/<slug>` | Regenerates the thumbnail of a finished run |
| `make render DIR=output/<slug>` | Produces the Remotion video |
| `make video DIR=output/<slug>` | Assembles the fast ffmpeg video |
| `make log` | Prints the last 40 lines of the newest log |
| `make check` | `fmt --check`, build, `clippy -D warnings` and tests |
| `make status` | Service state on the host over SSH |
| `make deploy` | Copies `deploy/` to the host and fixes permissions |

`make generate` builds the release binary first, so the run does not pay debug overhead.

## Output

```
output/<slug>/
├── audio.mp3      # 3 min song, 320 kbps, -15 LUFS
├── thumbnail.png  # the chosen 1280x720 candidate
├── meta.json      # candidates, seeds, prompts, selection and source URL
├── candidates/    # N image candidates and N audio drafts
├── video.mp4      # 1920x1080, image + particles + waveform
├── video-ffmpeg.mp4  # only when Remotion rendered over the fast version
└── work/          # source, reference and final take
```

## Screenshots

The images in these docs come from the test backend, not from a manual capture. Regenerate them with:

```bash
DUMP_TUI_HTML_DIR=/tmp/tui cargo test draws_without_leaving_the_terminal
```

That writes `tui.html` and `tui-splash.html`; render them with a browser at the size you need.
