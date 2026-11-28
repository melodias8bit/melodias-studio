# melodias-studio site

Static HTML for the GitHub Pages project page. No Jekyll, no Ruby, no build
step: one `index.html`, one stylesheet and one script.

## Preview locally

The page reads the screenshots from `docs/assets` and the ambient overlay from
`video/public/overlays`, so they are copied into `site/assets` first. The copies
are gitignored and staged by the same script the workflow runs.

```bash
make site
```

That stages the artwork and serves <http://127.0.0.1:4000/>. To stage without
serving:

```bash
bash scripts/prepare-site.sh
```

After changing `index.html`, `main.css` or `main.js`, refresh the browser. There
is nothing to rebuild.

## Deploy

`.github/workflows/pages.yml` stages the artwork, uploads `site/` as the Pages
artifact and deploys it on every push to `main` that touches the page or its
assets. Enable Pages in the repository settings with **GitHub Actions** as the
source; no branch build is needed.

The page is served from `https://melodias8bit.github.io/melodias-studio/`, which
is what `_config`-less relative asset paths assume. If the repository moves or a
custom domain is added, update the canonical and Open Graph URLs in
`site/index.html`.

## What to update

- Gallery: `docs/assets/tui-review.png` (regenerate with
  `DUMP_TUI_HTML_DIR=... cargo test draws_without_leaving_the_terminal`) feeds
  the TUI tab, while the creations component reads the 32 pool stills
  (`docs/assets/gallery-*.webp`, winners and losers from six runs, 960 px
  WebP): `gallery-battle-01..06` (winner 01), `gallery-finale-01..06`
  (winner 04), `gallery-artcore-01` (single take), `gallery-violin-01..04`
  (winner 04), `gallery-kawaii-01..09` (winner 04) and
  `gallery-village-01..06` (winner 04). The component filters by
  run, toggles winners only and opens a lightbox, so it scales past hundreds
  of stills without endless scroll. Regenerate one pool with:
  `ffmpeg -y -i output/<slug>/candidates/image-NN.png -vf scale=960:-2
  -c:v libwebp -quality 82 docs/assets/gallery-<prefix>-NN.webp`, then add one
  `figure.creation` per still, its filter button and count plus the `All`
  total in `site/index.html`, and the files in `scripts/prepare-site.sh`. The
  dashboard keeps its own screenshots in `dashboard/README.md`.
- Demo tracklist: `More from the channel` renders `site/videos.json` (newest
  upload first with its YouTube duration) and keeps the static list as the
  offline fallback. Refresh it from the live channel with
  `make site-videos` (needs `yt-dlp`; `SITE_VIDEOS_COUNT` overrides how many
  uploads are listed); the file is committed and the demo caption links the
  featured run.
- Pipeline diagram: the SVG inside the pipeline section is extracted from
  `site/pipeline.html`, an Archify workflow generated from `src/pipeline.rs`
  (`STAGES` plus the retry and review branches). The SVG carries the Archify
  dark variables scoped to `.diagram-svg` in `main.css`. Regenerate with the
  archify skill from `.archify/workflow-pipeline-*/candidate.json`, or author
  a fresh candidate and run `node bin/archify.mjs finalize workflow
  <candidate> site/pipeline.html --repo-root <repo> --quality showcase
  --json`, then re-extract the inline `<svg>` into `index.html`. Crop its
  `viewBox` height to 542 to drop the built-in legend and keep the HTML
  `.diagram-legend` below the frame instead: same hues, readable size. The
  standalone `site/pipeline.html` stays as the interactive source but is not
  linked from the landing.
- Copy: the diagram replaces the old stage list, so a `STAGES` change in
  `src/pipeline.rs` means regenerating the diagram. Per-run work (image,
  judge, score, youtube, render, video, history, export, dashboard, backfill,
  refs) lives in the `Makefile`, not on the landing.
- Tabs: the TUI panel shows `docs/assets/tui.png`, the headless panel the
  `melodias '<url>' '<vibe>'` command. Keep both honest when the CLI changes.
- FAQ: answers summarize `docs/setup.md`, `docs/usage.md` and
  `docs/configuration.md`. Fix the docs first, then the answer.
- Hero 3D: `site/assets/js/hero-3d.js` renders one Three.js particle wave from a
  CDN import (no build step, no bundler). It pauses off screen and renders a
  static frame with `prefers-reduced-motion`. Keep it to one canvas and no
  scroll hijacking.
- Theme: the page follows [DESIGN.md](DESIGN.md) (Mercury: onyx canvas,
  graphite cards, one cobalt action, Inter only, no shadows). Keep new styles
  inside that system.
- Artwork: `banner.png` is the README banner.
- Demo media: the page shows one real run from `output/`. The committed
  sources are `docs/assets/demo-thumbnail.webp`, `docs/assets/demo-audio.mp3`,
  `docs/assets/demo-video.mp4`, `docs/assets/demo-short.mp4` and
  `docs/assets/demo-short-poster.webp`; `scripts/prepare-site.sh` stages them
  into `site/assets/img/` and `site/assets/media/`. Current run:
  `output/euphoric-kawaii-edm-showdown-sparkling-s`, vibe "euphoric kawaii EDM
  showdown, sparkling synth leads over pounding festival bass".
  Refresh them with:

  ```bash
  slug=euphoric-kawaii-edm-showdown-sparkling-s
  ffmpeg -y -i output/$slug/thumbnail.png -vf scale=1280:-2 \
    -c:v libwebp -quality 85 docs/assets/demo-thumbnail.webp
  ffmpeg -y -ss 45 -t 30 -i output/$slug/audio.mp3 \
    -af "afade=t=in:st=0:d=1,afade=t=out:st=29:d=1" \
    -b:a 96k docs/assets/demo-audio.mp3
  ffmpeg -y -ss 45 -t 30 -i output/$slug/video.mp4 \
    -vf scale=1280:-2 -c:v libx264 -crf 27 -preset slow \
    -c:a aac -b:a 96k -movflags +faststart docs/assets/demo-video.mp4
  make short DIR=output/$slug ARGS="--sec 30 --start 45"
  ffmpeg -y -i output/$slug/short.mp4 \
    -vf scale=720:1280 -c:v libx264 -crf 30 -preset slow \
    -c:a aac -b:a 64k -movflags +faststart docs/assets/demo-short.mp4
  ffmpeg -y -ss 5 -i docs/assets/demo-short.mp4 -frames:v 1 \
    -vf scale=540:-2 -c:v libwebp -quality 82 docs/assets/demo-short-poster.webp
  ls -lh docs/assets/demo-*
  ```

  Keep the still under 1 MB, the audio under 1 MB, the horizontal clip
  under 4 MB and the vertical clip under 2 MB so the whole page stays
  under about 13 MB.
  Follow [docs/runbooks/site-demo.md](../docs/runbooks/site-demo.md)
  on the machine with the generated runs for the full checklist.
