# Configuration

Settings come from three places, in this order: environment variables, then `~/.config/melodias-studio/config.toml`, then the built-in defaults. `XDG_CONFIG_HOME` is respected and `MELODIAS_CONFIG` overrides the file path.

Start from the commented example:

```bash
mkdir -p ~/.config/melodias-studio
cp config.example.toml ~/.config/melodias-studio/config.toml
```

## Environment variables

| Variable | Default | Meaning |
| --- | --- | --- |
| `AUDIO_URL` | `http://192.168.88.38:8090` | `audiocpp_server` |
| `AUDIO_MODEL` | `ace-step` | model id registered on the server |
| `SD_URL` | `http://192.168.88.38:1234` | `sd-server` |
| `LLM_URL` | `http://192.168.88.38:8080` | `llama-server` |
| `LLM_MODEL` | `local` | model name the server expects |
| `SSH_HOST` | `root@192.168.88.38` | target for the reference and lifecycle commands; empty runs locally |
| `REMOTE_REF_DIR` | `/models/ref` | remote folder for references |
| `OUT_DIR` | `output` | local output |
| `MELODIAS_DB` | `history.db` next to `config.toml` | SQLite file behind `melodias history` |
| `GPU_LABEL` | `local models` | label in the header and the splash |
| `AUDIO_STYLE` | `8-bit chiptune` | fixed medium added to every audio prompt |
| `AUDIO_DETAIL` | empty | run-specific extras (BPM, instruments) appended verbatim to the audio prompt |
| `IMAGE_STYLE` | `8-bit pixel art` | style pinned in the image prompt system message |
| `IMAGE_MODEL` | `MOSS-Image-SFT-UD-Q4_K_XL` | image model label in the YouTube description |
| `CREATIVE_MODEL` | `Qwen3-4B-Instruct-2507` | LLM model label in the YouTube description |
| `IMAGE_NEGATIVE_PROMPT` | `glowing eyes, neon eyes, empty eyes, oversaturated eyes, text, watermark, signature, extra fingers, deformed hands` | negative prompt sent to sd-server |
| `DURATION_SEC` | `210` | take length |
| `STEPS` | `32` | diffusion steps |
| `GUIDANCE` | `5` | CFG |
| `SAMPLER` | `heun` | `euler` or `heun` |
| `WIDTH` / `HEIGHT` | `1280` / `720` | thumbnail size |
| `VIDEO_WIDTH` / `VIDEO_HEIGHT` | `1920` / `1080` | final video canvas |
| `VIDEO_RENDERER` | `remotion` | `remotion` or `ffmpeg`; ffmpeg is explicit only |
| `VIDEO_PARTICLES` | `both` | `both`, `bokeh` or `sparks` in the Remotion composition |
| `VIDEO_PROJECT` | `video` | Remotion project path, relative to the working directory |
| `JUDGE_ENABLED` | `true` | runs the judge when the provider has a key |
| `JUDGE_PROVIDER` | `auto` | `opencode`, `openrouter` or `auto` (OpenCode when its key is set) |
| `OPENCODE_API_KEY` | empty | OpenCode Go key; the default backend |
| `OPENROUTER_API_KEY` | empty | alternative backend; export it in the shell that runs `melodias` |
| `PIXABAY_API_KEY` | empty | key for `melodias refs fetch --source pixabay`; `pixabay.key` also works |
| `JUDGE_MODEL` | `gemini-4-argon` | OpenCode Go model, native Gemini API |
| `JUDGE_OMNI_MODEL` | `nvidia/nemotron-3-nano-omni-30b-a3b-reasoning:free` | OpenRouter perception model |
| `JUDGE_REASONER_MODEL` | `nvidia/nemotron-3-ultra-550b-a55b:free` | OpenRouter text-only final choice |
| `JUDGE_IMAGE_PX` | `768` | JPEG size sent to the judge |
| `JUDGE_AUDIO_SEC` | `15` | clip length sent to the judge |
| `JUDGE_TIMEOUT_SEC` | `300` | per-call timeout |
| `IMAGE_STEPS` | `20` | diffusion steps for the thumbnail |
| `IMAGE_SEED` | random | fixed seed to reproduce a thumbnail |
| `IMAGE_REFERENCES` | empty | optional folder of reference thumbnails, grouped by concept |
| `IMAGE_REFERENCE_COUNT` | `3` | references sent per image generation |
| `IMAGE_REFERENCE_CONCEPT` | empty | force a concept subfolder instead of matching the vibe |
| `CANDIDATES` | `3` | candidates per modality |
| `SCORE_ROUNDS` | `2` | deterministic retry rounds; `0` disables the loop |
| `AUDIO_DRAFT_SEC` | `60` | length of each audio draft |
| `AUDIO_DRAFT_STEPS` | `16` | steps for the audio drafts |
| `AUDIO_DRAFT_SAMPLER` | `euler` | sampler for the audio drafts |
| `REFERENCE_SEC` | `60` | length of the timbre reference, clamped to 5..120 |
| `AUDIO_REFERENCE` | empty | WAV used as the ACE-Step reference instead of a window of the source video |
| `FADE_IN_SEC` | `1.5` | master fade-in |
| `FADE_OUT_SEC` | `3.0` | master fade-out |
| `LOUDNESS_LUFS` | `-15` | master loudness target |

The judge is optional: it runs when `JUDGE_ENABLED` is not `false` and the selected provider has a key. It compresses the candidates to 768 px JPEGs and 15 s mono clips, sends them to a model that can see and hear, and preselects the best pair in the review; `Enter` accepts it. If the key is missing, the API fails or the JSON is invalid, the run falls back to the manual pick and logs it.

The judge also receives the deterministic findings and returns up to five `must_fix` codes for what the rubric cannot measure. They are normalized to the vocabulary `eyes`, `composition`, `style`, `retro`, `coherence` or `other`, and kept in `work/judge/verdict.json` and `meta.json`. They do not change the editorial score yet.

`JUDGE_PROVIDER=auto` uses OpenCode Go when `OPENCODE_API_KEY` is set and OpenRouter otherwise. The OpenCode backend is a single `generateContent` call to `gemini-4-argon`, which accepts images and MP3 in the same request (about 20 s and $0.019 of list cost per run, drawn from the Go monthly allowance). The OpenRouter backend is two stages: the free omni model perceives, the free text model decides. OpenRouter requires at least **$0.50 of balance for audio inputs**, even on a free model; image-only requests work without balance.

Keys are read from the environment of the process that runs `melodias` (or from `opencode_api_key` / `openrouter_api_key` in this file). They do not belong in the `video/` Remotion project, which only renders local assets.

#### What the judge costs

Measured on real runs with `gemini-4-argon`; the gateway reports the debit in `usage.opencode.allowance`, and `work/judge/verdict.json` stores it:

| Pool | Input tokens | Output | Allowance | List price | Time |
| --- | --- | --- | --- | --- | --- |
| 3 images + 3 clips | 5,303 (3,300 image, 1,353 audio, 650 text) | 517 | $0.0316 | $0.0158 | ~30 s |
| 4 images + 3 clips | 6,384 | 632 | $0.0382 | $0.0191 | ~20 s |

Each extra image adds about 1,100 input tokens and one row per clip in the verdict; each extra 15 s clip adds about 450. The debit follows `(650 + 1100·images + 451·clips) × $4/M + (175 + 38·images·clips) × $20/M`.

Go includes $30 of Argon usage per month ($120 on Go Plus), so the default 3 + 3 pool is about 950 runs per month, 190 per five-hour window and 475 per week. Those caps are shared with every other Go model, so a month of daily coding spends more than the judge. The advertised list price is half the debit until November 30, 2026; from December 1 the two match, and the debit is the number that decides when requests start failing.

A run costs about $0.03 of the allowance for 20 to 30 s of judge time, next to roughly 36 minutes of GPU work.

The channel medium is a hard rule: the audio prompt always gets `AUDIO_STYLE`, even when the vibe does not mention it, and the image system message always pins `IMAGE_STYLE`. The vibe adds mood and references, not the medium. The LLM writes the arrangement content from the vibe in the same loaded session as the thumbnail prompt, under the fixed section skeleton; when that call fails, the local keyword template stands in (calm words like `chill`, `lullaby` or `peaceful` switch to a slow arrangement, anything else keeps the driving default). `compose_audio_prompt` wraps the vibe with the full arrangement arc (motif first, development, dynamic break, climactic return, decisive ending) and the production bar, so a short vibe still gets a detailed prompt, and the detailed `AUDIO_NEGATIVE_PROMPT` goes on every audio request, reference or not.

`AUDIO_REFERENCE` replaces the window taken from the source video as the ACE-Step reference. Point it at a WAV you already have, for example a ChipForge render, a Wario Synth export or a MIDI-driven chip bounce. The file is normalized to 48 kHz stereo PCM and cut to `REFERENCE_SEC`, then pushed to the GPU host like the normal reference. The rest of the pipeline is unchanged, so the score and the retry rounds still apply.

An empty environment variable is ignored, so to run locally use `ssh_host = ""` in `config.toml` instead of `SSH_HOST=`.

The pipeline renders with Remotion by default. Before the GPU stages it checks `VIDEO_PROJECT`, `pnpm` and the overlays selected by `VIDEO_PARTICLES`; if something is missing the run fails there with the command to fix it. It never falls back to ffmpeg on its own. `VIDEO_RENDERER=ffmpeg` or `melodias video <dir>` is the explicit fast path.

## Reference library

`IMAGE_REFERENCES` points at a folder of reference thumbnails. It is only a path: a normal folder or a mounted SMB share. The app does not mount it, copy it into the repository, or handle credentials.

```text
references/
  battle/     ...
  forest/     ...
  castle/     ...
```

The subfolder whose name (letters and digits only) is contained in the vibe wins, longest match first. For `8bit jrpg battle theme`, `jrpg-battle` beats `battle`. `IMAGE_REFERENCE_CONCEPT=battle` forces one. If no subfolder matches, the images directly in the root are used; if there are none, the run continues without references.

`IMAGE_REFERENCE_COUNT` (3 by default) references are sent per generation through the sd-server image API (`extra_images`). Each reference costs conditioning time, so 2 to 4 is the practical range. The retry rounds rotate through the library, and `meta.json` records the references used by every image candidate. `melodias image` reuses the selected candidate's references.

Use images you have the rights to, for example your own art or CC0 packs. The app never downloads or scrapes anything.

### Building a library

`melodias refs fetch --source NAME` fills the folder. A source only discovers candidates; the app downloads, normalizes, deduplicates and writes the manifest. Adding a site means one module that implements `Source` in `src/refs/`, plus one line in the registry.

```bash
melodias refs fetch --source pixabay --count 40
melodias refs fetch --source pixabay --concept forest --count 40
melodias refs fetch --source opengameart --concept castle --count 20
melodias refs fetch --source opengameart --concept battle --count 5 --dry-run
```

The defaults come from the source: 18 concept folders (`battle`, `forest`, `castle`, `jrpg`, `boss`, `dungeon`, `cave`, `village`, `desert`, `snow`, `volcano`, `space`, `sky`, `underwater`, `swamp`, `graveyard`, `temple`, `city`), each with a query. The list and the per-source queries live in `src/refs/mod.rs`; adding a folder is one line there. `--concept` and `--query` override them (a custom `--query` needs exactly one `--concept`). `--out` falls back to `IMAGE_REFERENCES`, then `image_references` in `config.toml`, then `~/.local/share/melodias-studio/references`. `make refs ARGS="fetch ..."` wraps the command.

Common options: `--count N` files per concept (40), `--max-files-per-pack N` files taken from one zip (60), `--max-bytes N` for a single image (6000000), `--max-pack-bytes N` for a zip (40000000), `--size N` for images inside a zip (768), `--delay S` between downloads (0.2), `--tag TEXT` to keep only candidates whose tags contain TEXT (repeatable), `--packs` to also import the zip behind an entry, and `--dry-run`.

Packs are off by default. A pack usually mixes scenes with body parts, single props and sheets, so the default keeps one representative image per entry. `--packs` is for when you want the raw assets, and it still deduplicates and caps every pack.

Pixabay reads `PIXABAY_API_KEY`, then `pixabay_api_key` in `config.toml`, then `$XDG_CONFIG_HOME/melodias-studio/pixabay.key`:

```bash
install -m 600 /dev/null ~/.config/melodias-studio/pixabay.key
$EDITOR ~/.config/melodias-studio/pixabay.key
```

Its images are the 640 px rendition and it skips hits narrower than 400 px. `--tag pixel` drops some stock photos but is not a quality filter. Pixabay is a stock site, so expect single objects and UI icons next to scenes, and check for recognizable characters or brands before using a hit.

OpenGameArt searches only CC0 entries (license id 4, 2D art) and checks the license again on every art page. It sends a browser user agent and waits between pages. The first word of the query has to appear in the entry title or slug, because the search also matches descriptions and otherwise returns unrelated packs. Each entry offers one representative image, the original attachment when the preview points to one, otherwise the preview itself. Entries that are not CC0 are skipped and reported on stderr. With `--packs`, the zip behind an entry is imported too, normalized to PNG and capped.

`melodias refs generate` creates the references with the local sd-server instead of downloading them. They are yours, so no third-party license is involved, and they match the channel style:

```bash
melodias refs generate --concept volcano --count 5
melodias refs generate --concept boss --count 3 --out /tmp/refs
melodias refs generate --dry-run --concept city
```

It needs sd-server up, the same one the pipeline uses. Each concept has a scene prompt in `src/refs/generate.rs` and gets `IMAGE_STYLE` appended. Images are normalized to 768 px and recorded with `license: "generated"` and `author: "melodias-studio"`. `IMAGE_SEED` makes a batch reproducible; `--count` defaults to 3 and is capped at 50 per concept.

`melodias refs import` imports what you downloaded by hand, including `.zip` packs:

```bash
melodias refs import --concept castle --license CC0-1.0 \
  --author "Kenney" --source-url https://kenney.nl/assets/castle-kit \
  ~/Downloads/castle-kit.zip
```

It requires an explicit `--license`, normalizes to PNG with a 768 px longest side (`--size`), deduplicates by sha256, and names the files `<prefix>-<hash>.png` (`--prefix` defaults to `import`). Re-importing the same pack is harmless. It also accepts `--dry-run`.

Every accepted file is appended to `<out>/sources.jsonl`: concept, file, source, license, author, page URL, tags, archive member and hash. Keep that manifest with the library, it is the license record. Both commands need ImageMagick for the images that come from a pack; zip archives are read in process. Pixabay results use the Pixabay Content License: free use and adaptation, no attribution, no standalone redistribution, and no logos, brands or recognizable people.

## History

Every finished run scores itself and appends the result to a local SQLite database, again without a GPU or a cloud call. The file is `history.db` next to `config.toml` (`$XDG_CONFIG_HOME/melodias-studio/history.db` by default). `MELODIAS_DB` or the `db_path` key move it, for example into a checkout during development:

```bash
MELODIAS_DB=./local.db melodias 'https://www.youtube.com/watch?v=...' 'epic 8-bit orchestral finale'
```

The write happens after the video stage, with the score computed from the finished `thumbnail.png` and `audio.mp3`; `work/score.json` gets the same report. A failed write is a line in the run log and never aborts the run. The database uses WAL, one writer at a time, and is created with mode 0600. `.gitignore` covers `*.db`, `*.db-wal` and `*.db-shm` so a database inside the repo stays untracked.

Each `runs` row carries the `rubric` version, so a trend only compares runs scored with the same one. The pool candidates and the findings of the final pair are stored next to it.

`runs.outcome` is `accepted`, `accepted_by_limit` or `below_target` (the last one is what the backfill writes for runs made before the loop). `accepted_by_limit` means the run finished the retry budget without reaching the target; `outcome_reason` says which side stopped it. The applied prompt patches live in the `patches` table, with the positive and negative prompt as a JSON object.

Older databases are migrated on the next write: the outcome columns are added and existing accepted rows are mapped back. Readers such as `melodias history` and the dashboard tolerate a database that has not been migrated yet.

`melodias history` reads the file and never writes:

| Command | What it shows |
| --- | --- |
| `melodias history` | the last 20 runs |
| `melodias history --json` / `--csv` | the same rows for a script |
| `melodias history findings` | hits, average and total lost points per finding code |
| `melodias history trend --by day` | average technical and total per day, also `week` or `month` |

`make history ARGS=findings` is the shortcut. Runs made before the write existed are added with `make backfill`, which writes the same tables.

### Retry loop

The run scores each side before the review. If the best image is under 85, it applies the patch for the worst actionable finding and regenerates that side while sd-server is still up, up to two rounds. The audio side gets one round at most when it lost eight points or more of its 50; a weak image does not block it.

- `SCORE_ROUNDS` caps the rounds per side; the image budget is two and the audio budget is one.
- The same finding code never triggers twice. Codes that share a patch (`AUD_HIGHS` and `AUD_CENTROID`) count as one.
- The audio metrics top out at eight points, so their findings are `medium`, not `high`. The loop accepts medium or worse (a loss of four points or more); otherwise every audio patch would be unreachable.
- The pool caps at nine candidates per side, which is what the TUI can select with `1-9`.
- The review opens on the judge suggestion; without a judge it opens on the best-scored pair, not candidate 1.

The judge receives the deterministic findings and returns `must_fix` codes in `verdict.json` and `meta.json`, so it does not spend its answer repeating the rubric.

`melodias dashboard` serves a read-only page over the same file at `http://127.0.0.1:7373/`. It binds localhost only, opens the database for reading and polls it once a minute, so new runs appear without a reload. `--port` changes the address and `--limit` the run window (200 by default). The page is a Rust binary plus static files in `dashboard/`; there is no Node and no build step.

The run detail reads the stored documents next to the database: `meta.json`, `work/score.json` and `work/judge/verdict.json`, plus every image, audio and video under the run directory. The server finds the directory by slug under `OUT_DIR`, serves the files itself only inside that directory and supports HTTP byte ranges for the video. It never writes to the run directory.

## GPU lifecycle commands

They run between stages, over SSH when `SSH_HOST` is set. The defaults point at the `deploy/` scripts; override or empty them to manage the servers by hand:

| Variable | Default | When it runs |
| --- | --- | --- |
| `LLM_START_CMD` | `/home/ubuntu/bin/serve-llm.sh start` | before the thumbnail prompt |
| `LLM_STOP_CMD` | `/home/ubuntu/bin/serve-llm.sh stop` | before the audio stage, to free 3.4 GB |
| `SD_START_CMD` | `/home/ubuntu/bin/serve-sd.sh start` | before the thumbnail |
| `SD_STOP_CMD` | `/home/ubuntu/bin/serve-sd.sh stop` | before the audio stage |
| `AUDIO_START_CMD` | `/home/ubuntu/bin/serve-audio.sh start` | before the audio stage |
| `AUDIO_STOP_CMD` | `/home/ubuntu/bin/serve-audio.sh stop` | after the take, even if it failed |
| `AUDIO_NEGATIVE_PROMPT` | vocals, singing, rap, spoken words, choir, ... amateur production (full list in `config.example.toml`) | detailed steering text sent on every audio request, reference or not |

The app waits for `/health` on the service it just started, up to 180 s, before moving on. With `SSH_HOST` empty the reference is not copied and the audio server reads it from the local path.
