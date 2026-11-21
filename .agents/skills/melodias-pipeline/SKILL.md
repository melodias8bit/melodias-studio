---
name: melodias-pipeline
description: End-to-end map of the melodias-studio generation run, covering the 11 stages, the Msg channel, review selection, deterministic retries, score-v1 targets and meta.json. Use this whenever you change stage order or add a stage, debug a failed or partial run, touch src/pipeline.rs, src/retry.rs or src/score.rs, adjust retry budgets or patches, explain why a run stopped early, or need to know which services are up at each point.
---

# Melodias pipeline

The pipeline turns a source URL plus a vibe into a complete run directory. It is a single linear
function with one human decision in the middle. Any error aborts the run, and a run is never
resumed; the output directory keeps whatever finished before the failure.

## Control flow

1. `main` parses args. No args opens the TUI, `<url> <vibe>` runs headless, and the remaining
   subcommands act on finished runs.
2. `pipeline::spawn(cfg, url, vibe, tx)` starts the work on a thread and returns the
   `Sender<Selection>` used to answer the review.
3. `pipeline::run` sends progress through `Msg` and returns `Outcome`.
4. `step(tx, idx, f)` wraps each stage: it sends `Stage(idx, Running)`, then `Ok` or `Failed`, and
   logs the error. A failed stage aborts `run`.

`Msg` variants:

- `Stage(usize, State)` with `State::{Pending, Running, Ok, Failed}`
- `Log(String)`
- `Choose(ReviewRequest)` carrying the pool, the judge suggestion and the best-scored default
- `Done(Result<Outcome, String>)`

The TUI and the headless reporter consume the same channel. Headless accepts the judge
suggestion, or `default` when the judge did not answer. The TUI starts on the same selection and
allows `1-9`, `Tab` and `Enter` overrides.

Before stage 1, `run` calls `media::preflight()` (yt-dlp, ffmpeg, ffprobe and scp on PATH) and
`video::preflight()` (Remotion project, pnpm and overlays when the renderer is Remotion). Both
fail early on purpose so a missing tool does not waste GPU time.

## The 11 stages

`STAGES` in `src/pipeline.rs` is the single source of order and labels. The TUI sizes its state
list from `STAGES.len()`, so do not hardcode the count outside the test assertions.

| # | Stage | What happens |
| --- | --- | --- |
| 1 | Image prompt | Starts `llm_start_cmd`, waits for llama-server, asks for the thumbnail prompt, keeps `image_negative_prompt` as the negative side |
| 2 | Prepare image | Stops the LLM, starts `sd_start_cmd`, waits for sd-server |
| 3 | Image pool | Generates `CANDIDATES` images, scores each one, runs the image retry rounds, keeps the grown pool |
| 4 | Download | `yt-dlp` downloads the source audio into `work/` |
| 5 | Reference | Picks a window of the source, or normalizes `AUDIO_REFERENCE` when set |
| 6 | Upload | Pushes the reference to the GPU host with `scp` and returns the remote path |
| 7 | Audio drafts | Stops sd-server and the LLM, starts `audio_start_cmd`, waits for ACE-Step, generates short drafts, scores them, runs the audio retry round |
| 8 | Review | Runs the optional judge, sends `Msg::Choose`, then blocks on `decisions.recv()` |
| 9 | Final take | Generates the full-length track with the selected draft seed, then stops the audio server even if generation failed |
| 10 | Master + save | Fade in/out, loudness normalization, 320 kbps MP3, copies `thumbnail.png` and `audio.mp3` into the run root |
| 11 | Video | Renders `video.mp4`, writes `meta.json`, and only then records history |

The audio server stays loaded from stage 7 through stage 9 so the review does not pay a reload.
Everything else is stopped as soon as its consumer is done.

## Retry loop

The retry table is fixed data in `src/retry.rs`, not a model. A finding code selects prompt text
that is appended before the next round. The caller owns the rounds, the budget and the
generation.

- Budget per side: `cfg.score_rounds.min(IMAGE_ROUNDS)` with `IMAGE_ROUNDS = 2`, and
  `cfg.score_rounds.min(AUDIO_ROUNDS)` with `AUDIO_ROUNDS = 1`. `SCORE_ROUNDS` defaults to 2 and
  `0` disables the loop.
- A round only starts when the best candidate is below `score::TARGET_TECHNICAL` (85).
- The chosen finding must lose at least `retry::MIN_LOSS` (4 points). The audio side also
  requires 8 points of its 50-point rubric, which is why the code checks
  `(100 - score) / 2 >= 8` instead of a strict high filter.
- The same patch key is never applied twice. `AUD_HIGHS` and `AUD_CENTROID` share `AUD_TIMBRE`,
  so fixing one blocks the other.
- The pool stops at `MAX_POOL = 9` per side so the review keys `1-9` still reach every candidate.
- The loop stops for one of four reasons, recorded in `meta.retry.stop`: target reached, no
  patchable finding, pool full, or budget exhausted. A run that misses the target after the loop
  is `accepted_by_limit`, not a failure.

Retry rounds run while the relevant service is still up: image rounds before stage 3 ends, audio
rounds inside stage 7. Do not move a round past its service stop without moving the stop too.

## Adding or reordering a stage

1. Edit `STAGES` and `run()` in `src/pipeline.rs`.
2. Update the hardcoded `1/11` assertions in `src/tests/main.rs` if the count or label changed.
3. Check whether the stage starts or stops a service, and whether the failure path must release
   it. `run` only guarantees the audio server stop after the final take.
4. Check the retry placement: image budgets belong before sd-server stops, audio budgets before
   the audio server stops.
5. Run `make check`.

Do not reorder `Review`, `Final take` or `Master + save` without checking `meta.json`, history,
and the subcommands that read `meta.image.selected` and `meta.audio.draft.selected`.

## Output contract

```text
output/<slug>/
  thumbnail.png          selected image, copied in stage 10
  audio.mp3              mastered track, copied in stage 10
  video.mp4              renderer output, stage 11
  video-ffmpeg.mp4       previous video kept on Remotion re-render
  meta.json              written after the video, before history
  candidates/            image-NN.png and audio-NN.wav
  work/                  source, reference, take, master, score.json, judge/, video temp
```

`meta.json` keys that other code depends on:

- `slug`, `source_url`, `vibe`, `image_prompt`, `seed`
- `image.size`, `image.steps`, `image.negative_prompt`, `image.candidates[]`, `image.selected`,
  `image.file`
- `audio.model`, `audio.prompt`, `audio.negative_prompt`, `audio.draft`, `audio.final`,
  `audio.master`, `audio.reference`, `audio.file`
- `video.file`, `video.size`, `video.renderer`, `video.particles`
- `retry.rounds`, `retry.image_rounds`, `retry.audio_rounds`, `retry.stop`,
  `retry.patches[]`
- `judge`

Candidate entries keep `file`, `seed`, `prompt`, `negative_prompt` and `references`. That is what
`melodias image` replays, so a patch applied during a retry is reproducible without the config
file.

## Verification

- Full gate: `make check`.
- Stage label test: `cargo test writes_a_clean_log` asserts the `1/11` labels in
  `src/tests/main.rs`.
- TUI layout and snapshot test: `cargo test draws_without_leaving_the_terminal`.
- Score a finished run without a GPU: `make score DIR=output/<slug>` or
  `cargo run --release -- score output/<slug>`.
- Read the stop reason after a short run: `jq '.retry.stop' output/<slug>/meta.json`.
