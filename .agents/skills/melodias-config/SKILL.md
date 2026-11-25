---
name: melodias-config
description: Configuration and environment reference for melodias-studio, including precedence, file and database paths, service endpoints, SSH and local mode, the judge keys, output directories and the reference library. Use this whenever a service is unreachable, a setting seems ignored, empty environment variables do not take effect, the judge does not run, the history database is in the wrong place, or you need to add a config field and keep env > file > default behavior.
---

# Melodias configuration

`src/config.rs` is the source of truth. Every field has a built-in default, the file is optional,
and environment variables win over the file. Docs can lag the struct, so confirm a key in
`src/config.rs` before documenting or using it.

## Precedence and paths

For every scalar value: environment variable, then `config.toml`, then the hardcoded default.
`pick` handles strings, `pick_parsed` handles numbers and booleans parsed from env.

Empty environment variables are ignored. `env()` filters them out, so `SSH_HOST=` does not mean
"local mode"; it falls through to the file and then to the default `root@192.168.88.38`. To
disable SSH, set `ssh_host = ""` in `config.toml` and set the lifecycle commands to `""` there
too.

Config file resolution:

1. `MELODIAS_CONFIG` if set.
2. `$XDG_CONFIG_HOME/melodias-studio/config.toml`.
3. `~/.config/melodias-studio/config.toml`.
4. `./melodias-studio/config.toml` when neither XDG nor HOME is set.

Start from `config.example.toml` and uncomment only what changes. The file can hold API keys, so
it is user-only and must never be committed.

Database resolution: `MELODIAS_DB`, then `db_path` from the file, then `history.db` next to the
config file. `MELODIAS_DB` is the only way to move the database without editing the file.

`OUT_DIR` changes where runs are written (default `output`). Both the environment variable and
the `out_dir` file key work, in the usual env > file > default order; a relative path is
resolved against the current directory. The Makefile has separate `LOG_DIR` and `PREFIX`
variables that affect make targets, not the binary.

## High-impact settings

| Variable | File key | Default | Notes |
| --- | --- | --- | --- |
| `LLM_URL` | `llm_url` | `http://192.168.88.38:8080` | OpenAI-compatible chat server |
| `LLM_MODEL` | `llm_model` | `local` | Model name sent to the LLM |
| `SD_URL` | `sd_url` | `http://192.168.88.38:1234` | stable-diffusion.cpp |
| `AUDIO_URL` | `audio_url` | `http://192.168.88.38:8090` | ACE-Step server |
| `AUDIO_MODEL` | `audio_model` | `ace-step` | Model name sent to the audio server |
| `SSH_HOST` | `ssh_host` | `root@192.168.88.38` | Empty in the file means local |
| `REMOTE_REF_DIR` | `remote_ref_dir` | `/models/ref` | Where the reference is pushed |
| `*_START_CMD`, `*_STOP_CMD` | same | `/home/ubuntu/bin/serve-*.sh start\|stop` | Run over SSH, or locally without SSH |
| `OUT_DIR` | `out_dir` | `output` | Run directory root |
| `MELODIAS_DB` | `db_path` | next to config | History database |
| `DURATION_SEC` | `duration_sec` | `210` | Final track length |
| `STEPS`, `GUIDANCE`, `SAMPLER` | same | `32`, `5.0`, `heun` | Final audio generation |
| `WIDTH`, `HEIGHT` | same | `1280`, `720` | Thumbnail size |
| `IMAGE_STEPS` | `image_steps` | `20` | sd-server sampling steps |
| `AUDIO_STYLE`, `IMAGE_STYLE` | same | `8-bit chiptune`, `8-bit pixel art` | Fixed medium and look |
| `AUDIO_DETAIL` | `audio_detail` | empty | Run extras appended verbatim to the audio prompt |
| `AUDIO_NEGATIVE_PROMPT` | `audio_negative_prompt` | detailed list | Voices out, production tight, on every audio request |
| `AUDIO_DRAFT_SEC`, `AUDIO_DRAFT_STEPS` | same | `60`, `16` | Short review drafts |
| `AUDIO_DRAFT_SAMPLER` | `audio_draft_sampler` | `euler` | Draft sampler |
| `CANDIDATES` | `candidates` | `3` | Per modality; pool capped at 9 |
| `SCORE_ROUNDS` | `score_rounds` | `2` | `0` disables retries |
| `FADE_IN_SEC`, `FADE_OUT_SEC` | same | `1.5`, `3.0` | Mastering fades |
| `LOUDNESS_LUFS` | `loudness_lufs` | `-15.0` | Master loudness target |
| `VIDEO_RENDERER` | `video_renderer` | `remotion` | `remotion` or `ffmpeg` |
| `VIDEO_PARTICLES` | `video_particles` | `both` | `both`, `bokeh` or `sparks` |
| `VIDEO_WIDTH`, `VIDEO_HEIGHT` | same | `1920`, `1080` | Render size |
| `VIDEO_PROJECT` | `video_project` | `video` | Resolved against the current directory |
| `IMAGE_REFERENCES` | `image_references` | empty | Optional concept folder |
| `IMAGE_REFERENCE_COUNT` | `image_reference_count` | `3` | References sent per generation |
| `IMAGE_REFERENCE_CONCEPT` | `image_reference_concept` | empty | Forces a concept folder |
| `IMAGE_SEED` | `image_seed` | random | Reproducible image batch |
| `AUDIO_REFERENCE` | `audio_reference` | empty | Replaces the source window |
| `REFERENCE_SEC` | `reference_sec` | `60` | Reference length after normalization |
| `PIXABAY_API_KEY` | `pixabay_api_key` | empty | Key for `melodias refs fetch --source pixabay` |
| `JUDGE_ENABLED` | `judge_enabled` | `true` | No key means no-op |
| `JUDGE_PROVIDER` | `judge_provider` | `auto` | `opencode`, `openrouter` or `auto` |
| `JUDGE_MODEL` | `judge_model` | `gemini-4-argon` | OpenCode Go model |
| `JUDGE_IMAGE_PX`, `JUDGE_AUDIO_SEC` | same | `768`, `15` | Evidence compressed before the judge |
| `JUDGE_TIMEOUT_SEC` | `judge_timeout_sec` | `300` | Judge request timeout |

`OPENCODE_API_KEY` and `OPENROUTER_API_KEY` also come from the file or the environment, and
`GPU_LABEL` only changes the header text. The full list lives in `docs/configuration.md`; when
the two disagree, `src/config.rs` wins.

Booleans accept `1`, `true`, `yes` and `on` (case-insensitive). Numbers parse with `FromStr`; an
unparseable environment value is discarded and the file or default wins. Do not assume a typo
will fail loudly.

## Local mode

To run every service on this machine, set these in the file, not as empty environment variables:

```toml
ssh_host = ""
llm_start_cmd = ""
llm_stop_cmd = ""
sd_start_cmd = ""
sd_stop_cmd = ""
audio_start_cmd = ""
audio_stop_cmd = ""
```

`media::preflight` still requires `yt-dlp`, `ffmpeg`, `ffprobe` and `scp` on PATH even when SSH
is disabled. `scp` is required unconditionally, so a missing scp fails before stage 1.

Without start commands the pipeline does not manage the servers. Start them yourself and make
sure the URLs answer before running.

## Judge

The judge is enabled by default and becomes a no-op without a key. `auto` prefers
`OPENCODE_API_KEY` (OpenCode Go) and then `OPENROUTER_API_KEY`. The default judge is one
`generateContent` call to `gemini-4-argon`; it sees 768 px JPEGs and 15 s MP3s plus the
deterministic findings, and returns a verdict with up to five `must_fix` codes in
`work/judge/verdict.json` and `meta.json`.

`JUDGE_PROVIDER=openrouter` keeps the older two-stage path with the free omni and reasoner
models. OpenRouter audio input needs enough balance, and OpenCode Go requires the
`x-opencode-session` header and a non-generic User-Agent; those live in `src/judge.rs`, not in
config. A failed judge is logged and does not abort the run.

`melodias judge` only prints the verdict and writes `work/judge/verdict.json`; it does not update
`meta.json`.

## Reference library

`IMAGE_REFERENCES` is a normal folder or a mounted share with concept subfolders. The app never
mounts it, copies it into the repo, or stores credentials. The longest concept name contained in
the vibe wins; `IMAGE_REFERENCE_CONCEPT` forces one. Retry rounds rotate the subset. `melodias
refs` fills the library, and its `--out` fallback order is `IMAGE_REFERENCES`, then
`image_references`, then `~/.local/share/melodias-studio/references`.

## Adding a config field

1. Add the field to `Config` and to `FileConfig` in `src/config.rs`.
2. Add the env name in `Config::from_env` with `pick` or `pick_parsed`, then the file value, then
   the default.
3. Update `config.example.toml` with a commented example and the default.
4. Update `docs/configuration.md`; it is the reference for every env var and key.
5. Run `make check`. Config tests live in `src/tests/config.rs` if the field has parsing logic.

Keep the file and environment names aligned. A new key that only works through the file, or an
env var that is not documented, is a bug.

## Verification

- Confirm the effective path: `MELODIAS_CONFIG=/tmp/x.toml cargo run -- --help` does not print
  config, so read `Config::config_path` or check the file directly.
- Confirm the history location: `cargo run --release -- history` prints a hint and exits 0 when
  the database does not exist. It never creates or migrates the database.
- Never print keys in logs or commits. If a key is exposed, rotate it.
