# melodias-studio

Rust 1.85+ (edition 2024) TUI + CLI (binary `melodias`). A source URL and a vibe produce `output/<slug>/` with `thumbnail.png`, `audio.mp3`, `video.mp4`, `meta.json`, `candidates/` and `work/`. The reference only conditions timbre: the thumbnail and track are new material. No args opens the TUI, `<url> <vibe>` runs headless, and the remaining subcommands inspect or rework runs, history, references, or a single track.

## Commands

- Gate: `make check` runs `cargo fmt --check`, `cargo build`, `cargo clippy --all-targets -- -D warnings`, then `cargo test`, in that order. There is no CI. Run it before finishing; do not reformat unrelated files.
- Single test: `cargo test <name>`. The suite needs no services, GPU or network. Tests live in `src/tests/<module>.rs` and are wired with `#[path = "..."] mod tests;`; `src/chip/*` and `src/refs/*` point at `../tests/`.
- `make run` (TUI); `make generate URL='...' VIBE='...'` (release headless, log `output/run-<timestamp>.log`); `make log` tails the newest; `make install` copies the release binary to `~/.local/bin/melodias`.
- `make image|judge|score|render|video DIR=output/<slug>`, `make history [ARGS='findings']`, `make dashboard`, `make backfill [ARGS=--replace]`, `make refs ARGS=...`. They use `cargo run --release`, so the first one builds release.
- `make backfill` needs `ffmpeg`, ImageMagick and `tesseract`; `make refs` needs ImageMagick for images inside packs; `make deploy` and `make status` need SSH to the GPU host; the rest runs locally.
- `make dashboard` serves read-only charts on `http://127.0.0.1:7373/`.
- TUI screenshots are generated, not hand-captured: `DUMP_TUI_HTML_DIR=/tmp/tui cargo test draws_without_leaving_the_terminal`; `PRINT_TUI=1` prints the buffer as ASCII.
- Start config from `config.example.toml` at `$XDG_CONFIG_HOME/melodias-studio/config.toml` (`MELODIAS_CONFIG` overrides the path).

## Wiring

- `src/pipeline.rs` owns the 11-stage order in `STAGES` and `run()`. The TUI and headless reporter share the `Msg` channel and answer the review through the `Sender<Selection>` from `pipeline::spawn`. Adding a stage touches `STAGES`, `run()`, and the hardcoded `1/11` assertions in `src/tests/main.rs`. The retry loop lives in the image pool and audio drafts stages.
- Entry points: `src/main.rs` dispatches TUI/headless/subcommands and is the headless reporter; `src/app.rs` is the TUI; `src/config.rs` handles precedence; `src/clients.rs` speaks HTTP to llama-server, sd-server and audiocpp; `src/media.rs` shells out to `yt-dlp`, `ffmpeg`, `ffprobe`, `ssh` and `scp`.
- Features: `src/judge.rs` (cloud review), `src/score.rs` (deterministic `score-v1` rubric behind `melodias score`, writes `work/score.json`), `src/retry.rs` (patch table and finding picker), `src/history.rs` (SQLite schema v2 and read views), `src/dashboard.rs` (server), `src/video.rs` (Remotion/ffmpeg), `src/references.rs` (reference selection), `src/refs/` (reference library), `src/backfill.rs`, `src/chip/`.
- Config precedence is env var > `config.toml` > defaults. The judge is enabled by default but no-ops without a key; `auto` prefers `OPENCODE_API_KEY`, then `OPENROUTER_API_KEY`. A failed judge is logged and does not abort the run.
- The default judge is one `generateContent` call to `gemini-4-argon` through OpenCode Go. It sees 768 px JPEGs, hears 15 s MP3s, receives the deterministic findings, and returns a verdict plus up to five `must_fix` codes in `work/judge/verdict.json` and `meta.json`. `JUDGE_PROVIDER=openrouter` keeps the older two-stage path.
- Headless accepts the judge suggestion, or the best-scored pair when the judge is off. The TUI starts the same way and still allows `1-9`/`Tab`/`Enter` overrides.
- Child process output goes to the run log through `media::set_child_log` (TUI: `output/run-<unix>.log`, headless: `MELODIAS_LOG`). Use `attach_output`/`attach_stderr` for new subprocesses so they do not fight the TUI.
- `docs/architecture/arquitectura.html` is generated from `docs/architecture/arquitectura.json` with the archify skill (command in `docs/architecture/README.md`); edit the JSON, not the HTML. `.archify/` is gitignored.

## Gotchas

Runtime and config:

- Empty environment variables are ignored (`config.rs::env` filters them), so `SSH_HOST=` does not switch to local mode: it falls back to the file and then to `root@192.168.88.38`. Disable SSH and lifecycle commands in `config.toml` (`ssh_host = ""`, `*_cmd = ""`).
- `media::preflight` requires `yt-dlp`, `ffmpeg`, `ffprobe` and `scp` on `PATH`, including `scp` when `SSH_HOST` is empty. `video::preflight` adds `pnpm`, `VIDEO_PROJECT/node_modules` and the overlays selected by `VIDEO_PARTICLES`.
- ACE-Step only reads the nested `options` object: `duration_sec`, `reference_audio`, `sampler_mode` and `negative_prompt` are silently dropped at the top level of `request`.
- A thumbnail size mismatch is only a warning; `media::png_size` validates the PNG header, not the pixels, so trust the log or `meta.json` for the real size.
- The run is not resumable and a failed stage aborts it. `output/` and `target/` are gitignored; generated `docs/` images and HTML are committed. Candidates and drafts live in `output/<slug>/candidates/`.
- Remotion is the default renderer and takes about 12 minutes. There is no silent fallback: `VIDEO_RENDERER=ffmpeg` or `melodias video` is the explicit fast path. `VIDEO_PARTICLES` selects `both` (default), `bokeh` or `sparks`; the composition always adds the birds layer.
- `melodias render` resolves `VIDEO_PROJECT` from the current directory (run it from the repo root), copies the still and track into ignored `video/public/current/`, writes ignored `video/props.json`, renders the `showwaves` waveform overlay, writes `work/video-remotion.mp4`, then moves it to `video.mp4`, keeping a previous file as `video-ffmpeg.mp4`.
- `AUDIO_REFERENCE` replaces the source window as the ACE-Step reference; `ffmpeg` normalizes it to 48 kHz stereo PCM and cuts it to `REFERENCE_SEC` before it is pushed to the GPU host.
- `melodias judge` only prints the verdict and writes `work/judge/verdict.json`; it does not update `meta.json`. OpenCode Go needs `x-opencode-session` and a non-generic User-Agent, otherwise 400 `MissingSessionID`. OpenRouter audio input needs at least $0.50 of balance.

Scoring and history:

- `SCORE_ROUNDS` defaults to 2 and caps the deterministic retry rounds per side (budgets: 2 image, 1 audio); `0` disables the loop. Rounds run while the relevant GPU service is up, image before audio.
- The loop acts on `medium` or worse (`delta <= -4`), not only `high`: audio metrics top out at 8 points, so a strict high filter would never reach the audio patches. The pool stops at 9 per side so `1-9` still selects everything in the review.
- The same patch key is never applied twice; `AUD_HIGHS` and `AUD_CENTROID` share `AUD_TIMBRE`. `patches.prompt_before` and `prompt_after` hold the positive and negative prompt as a JSON object.
- History schema v2 adds `runs.outcome` and `runs.outcome_reason`. `open()` migrates old files on the next write; readers tolerate the missing columns. A run that misses the target after the loop is `accepted_by_limit`, not a failure. Backfill writes `below_target` for legacy rows.
- The history write happens after the video and re-scores the run with `ffmpeg` (about 20 s more). It is best effort: a failure is one log line and the run stays valid. The write also refreshes `work/score.json`.
- `melodias history` never creates or migrates the database; on a machine without the file it prints a hint and exits 0. `make dashboard` opens the database read-only (with a read-write fallback for a WAL edge case), never migrates it, and reads it on every request.
- The dashboard has no Node or build step; its HTML, CSS and JS in `dashboard/` are `include_str!`, so editing them needs a rebuild. A database without the expected tables answers 500 and logs the SQL error; a missing file renders an empty state.
- `melodias backfill` writes through the same `history::insert` as the pipeline, so findings attach to the run with a null `candidate_id` like the normal write.
- `melodias image` regenerates from the selected candidate's stored `prompt`, `negative_prompt`, seed and references, falling back to the global prompt when the candidate has none. A patched retry is reproducible without the config file.
- `rusqlite` is pinned to 0.32 with `bundled` because 0.43 needs Rust 1.89. Keep Rust 1.85 as the floor until the repo moves.

References and optional commands:

- The reference library is optional and user supplied: `IMAGE_REFERENCES` is a normal folder or mounted SMB share grouped in concept subfolders. The app never mounts it, copies it into the repo, or handles credentials. The longest concept name contained in the vibe wins; `IMAGE_REFERENCE_CONCEPT` forces one; `IMAGE_REFERENCE_COUNT` defaults to 3 and retry rounds rotate the subset. Use own or CC0 images.
- Reference images go to sd-server `/sdapi/v1/txt2img` as `extra_images` (base64); the no-reference path keeps `/v1/images/generations` with `sd_cpp_extra_args`. The field is `extra_images`, not `ref_images` (that one is for video models). `meta.image.candidates[].references` records the files and `melodias image` reuses them.
- `melodias refs` builds that library: `src/refs/` owns the `Source` trait, download, normalization, dedupe and `<out>/sources.jsonl`; `pixabay.rs` and `opengameart.rs` are the sources, `import.rs` handles files/folders/zips, and `generate.rs` uses sd-server. Pack members are read with the `zip` crate, so `unzip` is not required. Pixabay reads `PIXABAY_API_KEY`, `pixabay_api_key` or `~/.config/melodias-studio/pixabay.key`; OpenGameArt is CC0 only and needs a browser user agent. `--out` falls back to `IMAGE_REFERENCES`, then `image_references`, then `~/.local/share/melodias-studio/references`.
- `CONCEPTS` in `src/refs/mod.rs` is the fixed vocabulary (18 folders, each with a Pixabay and OpenGameArt query). Packs are opt-in with `--packs` because their members are often body parts or single props, not scenes. `IMAGE_SEED` makes a generated batch reproducible.
- `melodias chip <url|file>` is the experimental NES cover command: yt-dlp, Demucs, Basic Pitch, the Rust arrangement in `src/chip/`, FamiStudio CLI and an ffmpeg stereo mix. It is not in the default pipeline. It needs `CHIP_VENV` with demucs and basic-pitch, `FAMISTUDIO_BIN` and `DOTNET_ROOT`; defaults are under `~/.local/share/melodias-studio/{venvs,tools}`. There are no Python scripts in the repo for it.

## Ops and docs

- Defaults point at one GPU box (`192.168.88.38`). `deploy/env.sh` holds host paths, ports and model paths; `serve-{llm,sd,audio}.sh` each answer `start|stop|status`; `make deploy` copies them plus `audio-server.json` to `REMOTE_BIN_DIR`/`REMOTE_CONFIG_DIR`.
- `docs/configuration.md` is the reference for every env var, config key and output format. `docs/setup.md` covers services, `docs/usage.md` covers flows, `docs/video.md` covers the renderer, and `dashboard/README.md` covers the charts.
- Docs lag the code: `docs/architecture/README.md` still draws the ffmpeg video, and `docs/roadmap.md` plus the RFC 0001 status still say video assembly or the retry loop is missing. Trust `src/` (`src/video.rs`, `src/pipeline.rs`) and update the docs when behavior is confirmed. The five editorial axes and their weights from RFC 0002 are still unimplemented.
