# Adversary review and video assembly

Status: in progress. Phases 1, 2 and the cloud judge are implemented, and so is the assembly with ffmpeg and Remotion. The pipeline renders with Remotion by default; ffmpeg is the explicit fast path. The local hard gates are implemented as a first set (type, blank, text, duration, peak, leading silence, decode); palette distance, band balance, stable tempo and vocal detection stay unmeasured. The deterministic retry loop is specified in RFC 0002 and implemented. The judge preselects the pair and the TUI keeps the manual override.

## Why

Today the pipeline generates three thumbnails and three audio drafts and pauses for a manual pick. That works, but the choice is still a human decision with no written rationale. The next step is to put a judge on top of the pool: local hard gates first, then a cloud reviewer that sees the images, hears the clips and picks the pair that best matches the brief.

The proposal is to generate a small pool of candidates per modality, let local critics pick the best one inside a fixed number of rounds, optionally ask a stronger cloud judge for a second opinion, and then assemble the final video from the chosen still, an audio-reactive effect and the mastered track.

## Goals

- Improve the average quality without a human in the loop.
- Stay within roughly 40 minutes per video, assembly included.
- Bound the review with an explicit round limit and a score threshold.
- Keep the local-only mode working: the cloud judge is optional.
- Record every candidate, score and decision in a manifest.
- Assemble a publishable video from the chosen pieces.

## Non-goals

- Training or fine-tuning models.
- Automatic publishing to YouTube.
- Reviewing with raw media on the cloud when the user disables it.

## Flow

```
brief (vibe + fixed rules)
        │
        ├─ image pool (N candidates) ──► hard gates ──┐
        │                                             │
        ├─ audio pool (N candidates) ──► hard gates ──┤
        │                                  ▲          │
        │                        retry the weak side  │
        │                        while rounds remain   │
        └──────────────────────────────────┴──────────┘
                                               │
                               compressed evidence (JPEG + clips)
                                               │
                          omni perception (sees + hears)
                                               │
                          Nemotron Ultra judge (reasoning, text)
                                               │
                          decider: strict choice or argmax
                                               │
                          assembly: Remotion or ffmpeg
                                               │
                                            final/
```

The pools run sequentially because a single GPU cannot hold the image model and ACE-Step at the same time. The image pool still runs first, as it does now. The judge only sees compressed evidence, not the full run.

## Candidate pools

- `CANDIDATES`, default 3, for both pools. Implemented.
- Every candidate keeps its seed and parameters in `manifest.json`.
- Image seeds and steps are sent through `sd_cpp_extra_args`; the same seed and steps reproduce the image.
- The TUI pauses on a review screen and the chosen pair drives the final take. The headless mode picks the first candidate.
- The candidate data lives in `meta.json`.
- Each pool loads its model once and generates all candidates before unloading it.
- Candidates are not regenerated at full steps; the same parameters are used for the whole pool.

## Local critics

Two layers, cheapest first.

### Hard gates

Deterministic checks that need no model. Implemented in `src/score.rs` (`image_gates`, `audio_gates`) and enforced per candidate in the pool (`src/pipeline.rs`): a gated candidate is discarded before any soft score counts, and the final pair still has to pass them in `score_run`:

- Image: valid PNG (`GATE_DECODE` as a scoring error), exact size and aspect (`GATE_TYPE`: 16:9 within 1 %, at least 1280x720), not blank (`GATE_BLANK`: luma std below 0.02), no clear text (`GATE_TEXT`: 3 or more OCR words; one or two stay soft as `IMG_TEXT` because illustrated art invents junk tokens).
- Audio: duration tolerance (`GATE_DURATION`: within 2 s), no clipping (`GATE_PEAK`: true peak above 0 dBFS), no leading silence (`GATE_LEADING`: over 1.5 s of silence at the start; the tail is excluded because the master fade leaves an intentional near-silence).
- Decode failures in the pool log a warning and leave the candidate unscored, which excludes it from the best pick.

Still unmeasured, by design: palette distance to the channel palette (the unique-color count stays soft as `IMG_PALETTE` because a hard cap would reject every current run), band balance against the reference, stable tempo, intro ramp and outro decay beyond what `AUD_BUILD` and the master fades cover, and vocal detection for the instrumental rule (source separation is the reliable option but adds a model and time). The judge `must_fix` codes stay the counterweight for eyes, composition and style.

### Soft score

- Image to text alignment with a small CLIP model.
- Audio to text alignment with a small CLAP model.
- Channel style: palette, composition, energy envelope.
- A local text LLM receives the metrics plus the brief and returns a ranking with one line of reasoning per candidate.
- A small vision model can caption the image when the LLM needs more than metrics.

Start with one critic. If two critics are desired, use a technical one and a creative one, then a local arbiter on the text model.

## Selection versus merging

Default to selecting the best candidate. Blending three images or splicing arbitrary audio sections usually creates visible or audible seams.

Merging is a later experiment: choose the best intro, body and outro only when tempo and key match, crossfade on bar boundaries, and reject the result if the seam measures above a threshold.

## Iteration policy

- `REVIEW_ROUNDS`, default 2.
- `REVIEW_THRESHOLD`, for example 0.75 on a normalized score.
- Each round regenerates only the modality below the threshold.
- Rejected candidates are deleted, but their scores stay in the manifest.
- When the rounds run out, take the best candidate, mark it as accepted by limit, and continue. The run never loops forever.

## Cloud judge

Optional. It runs when a provider key is present and replaces the manual pick with a review over compressed evidence, so the request stays small and cheap.

### Default backend: OpenCode Go with Gemini 4 Argon

`gemini-4-argon` is part of the OpenCode Go plan and accepts images and audio in the same native `generateContent` request (`https://opencode.ai/inference/go/google/v1beta`). One call sees the thumbnails, hears the clips and returns the whole verdict, so the two-stage split below is not needed:

- Request: system brief, 768 px JPEGs, 15 s mono MP3s, `responseMimeType: application/json`.
- Cost: about 6.4k input tokens and 0.02 USD of list cost per run, drawn from the Go monthly allowance. Roughly 20 s of wall time.
- Validation: on the first real run it ranked the repaired thumbnail (natural green eyes) first and picked the strongest clip, which matches the manual correction.
- The gateway requires `x-opencode-session` and an explicit user agent, or it answers 400.

The key is `OPENCODE_API_KEY`. `JUDGE_PROVIDER=auto` picks this backend when the key is set and falls back to OpenRouter otherwise.

### Alternative backend: OpenRouter, two stages

### Perception: the omni model

`nvidia/nemotron-3-nano-omni-30b-a3b-reasoning:free` accepts text, image, audio and video (256k context, free at the time of writing). Send the brief, the fixed rules, the three thumbnails and three short clips:

- Images: 768 px JPEG, around 100 to 200 KB each. Not the full 1280x720 PNG.
- Audio: 15 s mono MP3 at 64 kbps, around 120 KB each. Not the 60 s WAV.
- Ask for strict JSON with one score and one reason per image, one per audio, the coherence of each image/audio pair, and the best pair.

The omni model is the part that actually sees the eyes, the palette and the composition, and that hears whether the track feels like the image.

### Reasoning: the judge

`nvidia/nemotron-3-ultra-550b-a55b:free` is text only (1M context, free). It receives the brief, the local metrics and the omni verdict, and returns the final decision:

```json
{
  "verdict": "approve",
  "image": 2,
  "audio": 1,
  "score": 86,
  "must_fix": [],
  "nice_to_have": ["warmer highlight"],
  "reason": "one or two sentences"
}
```

The Ultra is the arbiter, not the perception layer. It never sees the raw media.

The judge is implemented in `src/judge.rs`. It runs before the review, writes `work/judge/verdict.json` (provider, model, token usage, both reports, the axes and the selection), and returns a suggestion that preselects the TUI review. Without a provider key the judge is skipped and the manual pick still works. The editorial score is the weighted sum of the five axes (weights in RFC 0002); legacy verdicts without axes fall back to the single opaque score.

### Decider

`pplx-decider-v1.3` is not available on OpenRouter: the URL returns 404 and the model list only has the Sonar family (text and image, paid). Use the Ultra with a strict choice, or a deterministic argmax over the omni scores. A paid Sonar model is only worth it for a third opinion, not for the default path.

### Why two models

- The Ultra is blind and deaf. It cannot rank images or audio by itself.
- The omni model sees and hears, but it is smaller; it is good at describing and scoring, not at the final arbitration.
- Two calls per round are cheap and under the free limits. Cache the verdict in `meta.json`.

### Rules

- One revision per run. If `must_fix` is not empty and rounds remain, regenerate only the weak side.
- Fail-open: if the API fails, returns invalid JSON or hits a rate limit, continue with the local pick and log it.
- The judge never writes files; it only returns JSON.
- Free model IDs change and disappear, as `pplx-decider` did. Keep them in config and verify at runtime.
- Privacy: the images and clips go to a third party. The source URL is not sent. Enable only when that is acceptable.

## Assembly

Two renderers, same inputs (chosen still and mastered track):

- **ffmpeg**, `media::assemble_video`: fast (~2 min), deterministic, used only when requested with `VIDEO_RENDERER=ffmpeg` or `melodias video <dir>`. Slow zoom, moving sprite particles, waveform band and fades.
- **Remotion**, the `video/` project: the pipeline default (~12 min at 1080p30). Uses the particle and bird clips in `assets/videos/` as screen overlays, the same `showwaves` line as the fast renderer (rendered by ffmpeg as a screen overlay), a slow zoom, the channel label and fades. `VIDEO_PARTICLES=both|bokeh|sparks` selects the particle layers. Run `melodias render <dir>` to force it on a finished run.

Both write `video.mp4`. Remotion is the pipeline default; `VIDEO_RENDERER=ffmpeg` or `melodias video` keeps the fast cut, and a previous video is kept as `video-ffmpeg.mp4` when re-rendering.

The assembly agent that picks a preset from the vibe is still pending.

## Data model

```
output/<slug>/
├── meta.json          # the manifest: brief, config, seeds, candidates, scores, judge verdict
├── candidates/
│   ├── image-01.png
│   ├── image-02.png
│   ├── audio-01.wav
│   └── audio-02.wav
├── thumbnail.png
├── audio.mp3
├── video.mp4
├── video-ffmpeg.mp4   # only when Remotion rendered over the ffmpeg version
└── work/
```

`meta.json` records the brief, resolved config, seeds, hard gate results, soft scores, rounds used, judge verdict and the chosen paths.

## Cost control and runtime

More candidates only make sense if a round stays affordable. The costs come from three choices: the image request now sends steps (20 by default), the audio request uses 32 steps with heun, which evaluates the model twice per step, and every candidate pays the full duration.

Use two tiers:

| Tier | Image | Audio |
| --- | --- | --- |
| Pool | 1280x720, 20 steps | 60 s, 16 steps, euler |
| Final | the pool winner, no regeneration | configured duration, 32 steps, heun |

Why each choice:

- **Image drafts did not hold up.** A/B at 1280x720 with the same seed on the P40: 12 steps took 116 s and 20 steps 285 s, but the 12-step image was visibly softer and even changed the outfit. SSIM between the two was 0.24. Use 20 steps for every image candidate; the saved time is not worth the quality loss.
- **Image resolution is the last lever.** 1024x576 saves more time but changes the composition. If it is used, the critic must rank the draft and the final must be reviewed again, which defeats part of the saving.
- **Short audio drafts rank style, not the arc.** A 60 s draft at 16 euler steps should be enough to compare timbre, energy and prompt adherence. The full 210 s take is generated once, at 32 heun steps, and passes the hard gates for intro, outro and duration.
- **Keep the model loaded across the pool.** One load per modality, all candidates inside it.
- **Turbo for audio drafts is optional.** ACE-Step Turbo caps at 8 steps but requires switching the DiT and reloading, so it only pays off if the reload is cheaper than the saved steps. Measure before adopting it.

The target budget is roughly 40 minutes per video, assembly included. N=3 is the default at about 36 minutes; N=2 is the faster option.

Measured wall time, warm model:

| Step | Today | N=2 | N=3 |
| --- | --- | --- | --- |
| Image pool | ~5 min | ~10 min | ~15 min |
| Audio pool | ~14 min | ~4 min + ~14 min final | ~6 min + ~14 min final |
| Assembly (ffmpeg) | ~2 min | ~2 min | ~2 min |
| Assembly (Remotion) | ~12 min | ~12 min | ~12 min |
| Total, ffmpeg | ~20 min | ~29 min | ~36 min |
| Total, Remotion | ~30 min | ~46 min | ~50 min |

The default is Remotion, so the real budget is closer to 50 minutes; `VIDEO_RENDERER=ffmpeg` keeps the ~36 minute budget. The judge calls are free.

One quality question remains:

- Does a 60 s draft rank the same take that a full 210 s run would produce?

If the answer is no, the budget cannot afford two full takes. The fallback is one full take with no audio pool, or an explicitly longer opt-in run.

## Configuration sketch

```
CANDIDATES=3
IMAGE_STEPS=20
IMAGE_SEED=
AUDIO_STEPS=32
AUDIO_DRAFT_STEPS=16
AUDIO_DRAFT_SEC=60
AUDIO_DRAFT_SAMPLER=euler
REVIEW_ROUNDS=2
REVIEW_THRESHOLD=0.75
JUDGE_ENABLED=false
JUDGE_PERCEPTION_MODEL=nvidia/nemotron-3-nano-omni-30b-a3b-reasoning:free
JUDGE_REASONER_MODEL=nvidia/nemotron-3-ultra-550b-a55b:free
JUDGE_DECIDER_MODEL=
OPENCODE_API_KEY=
OPENROUTER_API_KEY=
JUDGE_PROVIDER=auto
JUDGE_MODEL=gemini-4-argon
JUDGE_MAX_CALLS=4
JUDGE_IMAGE_MAX_PX=768
JUDGE_AUDIO_SEC=15
VIDEO_RENDERER=ffmpeg # or remotion
ASSEMBLY_PRESET=auto
```

The same keys belong in `config.toml` once the feature exists. Image seeds and steps are already sent, and the A/B settled the image tier: full steps for every candidate.

## Phased plan

1. Image steps and seed through `sd_cpp_extra_args`, plus the draft/final policy. Seed, steps and the image tier (full steps) are done; the audio draft policy is pending validation.
2. Manifest and candidate pools, with a manual pick in the TUI. No new models. Done: `meta.json` records the candidates and the selection.
3. Deterministic hard gates and local metrics. Done as a first set: `GATE_TYPE`, `GATE_BLANK`, `GATE_TEXT`, `GATE_DURATION`, `GATE_PEAK`, `GATE_LEADING`, plus decode errors; enforced per candidate in the pool and on the final pair. Palette distance, band balance, tempo and vocal detection stay open.
4. Omni perception + Ultra judge, with the strict JSON contract and fail-open. Done.
5. Review loop with rounds and thresholds; the TUI can still override the pick.
6. Video assembly: done. Remotion is the default and ffmpeg is the explicit fast path; the preset agent is pending.
7. Bar-aligned splicing, only if the seams measure clean.

## Risks and open questions

- Image throughput, to test later: batch the pool candidates in one request when sd-server supports it, or switch to a distilled few-step model and re-run the 12 vs 20 step A/B. Steps are sequential by nature and cannot be split; the saving comes from fuller GPU use or fewer steps at equal quality.

- Vocal detection for the instrumental rule. Source separation is the reliable option but adds a model and time.
- Free model availability is not guaranteed. `pplx-decider` already returned 404; the Nemotron IDs can change too.
- The 15 s clip may not represent the full three minute arc. The judge ranks timbre and energy; the final take still passes the hard gates.
- The Ultra judge is blind and deaf by design; if the omni model is unavailable, there is no perception layer.
- Rate limits on free models. Cap the calls per run and fail-open.
- OpenRouter charges the audio input path with a $0.50 minimum balance even on a free model. Images work without balance; audio is the one that needs credit.
- OpenCode Go is sold for coding-agent traffic, and this judge is media review. It is the user's own subscription and the volumes are tiny, but it is not the advertised use case.
- The Gemini request carries 4 images and 3 clips at once, around 870 KB of base64. It works today; a larger candidate pool would need chunking or a smaller audio bitrate.
- Privacy: images and clips leave the machine. Keep the judge opt-in and never send the source URL.
- Cost and time grow linearly with the candidates, and Remotion adds about 12 minutes on top.
- Judge reliability: strict schema, one revision, and fail-open to the local pick.
- Reproducibility: image seeds and steps are now sent; recheck that a server update keeps the same output for the same input.
- Blending audio across takes can sound worse than the best single take. Measure before shipping it.
