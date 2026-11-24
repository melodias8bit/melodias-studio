# Setup

## Requirements

- Rust 1.85+ (`rustup default stable`) to build from source.
- `yt-dlp`, `ffmpeg` and `ffprobe` on `PATH`. `yt-dlp` pulls the timbre reference; the thumbnail and the track are generated from scratch.
- Three HTTP services:
  - a chat completions server for the thumbnail prompt (llama.cpp or any OpenAI-compatible endpoint)
  - a stable-diffusion.cpp server for the image (OpenAI images API)
  - an audio.cpp server with the ACE-Step family for the music (`/v1/tasks/run`)
- `ssh` and `scp` only when the services run on another host or when the lifecycle commands need them.
- Node 24+ and pnpm only for the produced Remotion video.

No particular GPU or model is required.

## Install

```bash
make install
melodias
```

`make install` builds the release binary and copies it to `~/.local/bin/melodias`. Override `BINDIR` or `PREFIX` if you keep binaries elsewhere, and make sure that folder is on `PATH`. `make uninstall` removes it. `cargo run` still works for development.

To run everything on this machine, set `ssh_host = ""` in `config.toml` and empty the lifecycle commands. See [configuration.md](configuration.md).

## Example setup

The defaults describe one machine with a single 24 GB P40: `llama-server` with `Qwen3-4B-Instruct-2507` on `:8080`, `sd-server` with MOSS-Image on `:1234` and `audiocpp_server` with ACE-Step on `:8090`, all reached over SSH. The 4B model keeps ~3.4 GB busy, so the prompt step and `sd-server` (11.4 GB) can hold the GPU at the same time. The audio stage stops both `sd-server` and the LLM: ACE-Step with a timbre reference needs the whole card. `deploy/audio-server.json` enables `ace_step.mem_saver`, which returns the session to ~14.7 GB between requests. Use it as a reference, not as a requirement.

## Deploy the server scripts

The scripts in [`deploy/`](../deploy/) start, stop and report the three services. They are optional: if you start the servers yourself, leave the lifecycle commands empty.

`deploy/env.sh` holds the host layout: binary paths, model paths, ports, and the run and log folders. The `serve-*.sh` scripts source it, so a different machine only needs different values there. Edit the deployed copy, or export the variables before running a script.

```bash
make deploy
```

That copies `env.sh`, the scripts and `audio-server.json` to `REMOTE_BIN_DIR` and `REMOTE_CONFIG_DIR` (`/home/ubuntu/bin` and `/home/ubuntu` by default) on `SSH_HOST`, then fixes permissions. If you move them, update the lifecycle commands in your config so the app calls the new paths. Each script answers `start`, `stop` and `status`.
