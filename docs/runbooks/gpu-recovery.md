# GPU recovery

The pipeline drives the GPU box over SSH and starts one model server at a time.
When a run dies in stage 0-6, work through this list before retrying.

## Services

Each model has a lifecycle script on the box, answering `start|stop|status`:

- `serve-llm.sh` (llama-server, image prompt and YouTube copy)
- `serve-sd.sh` (stable-diffusion.cpp, thumbnails)
- `serve-audio.sh` (ACE-Step, drafts and final take)

Check from the laptop:

```bash
make status
```

Or one service: `ssh $SSH_HOST /home/ubuntu/bin/serve-sd.sh status`.
A stale server looks running but refuses connections; `stop` then `start`
fixes it. The pipeline stops each server before starting the next, so a run
that aborts mid-stage can leave one up: stop it by hand.

## SSH and identity

- `SSH_HOST` defaults to `root@192.168.88.38`; `MELODIAS_CONFIG` and
  `config.toml` override it.
- Empty environment variables are ignored, so `SSH_HOST=` does not switch
  to local mode: it falls back to the file, then to the default. Disable SSH
  in `config.toml` (`ssh_host = ""`, lifecycle `*_cmd = ""`).
- `scp` must be on `PATH` even in local mode; `media::preflight` requires it.

## Disk

Runs are heavy: candidates plus `work/` reach hundreds of MB per run and
`output/` is gitignored, never cleaned. When the laptop or the box fills up,
delete finished `work/` dirs or whole old slugs (the history database keeps
the scores). Back up anything unpublished first.

## Judge keys

- No key, no judge: the run continues with the manual pick and logs it.
- `judge_enabled` defaults to true; without `OPENCODE_API_KEY` or
  `OPENROUTER_API_KEY` the judge is skipped, not an error.
- OpenRouter audio input needs at least $0.50 of balance even on a free model.
