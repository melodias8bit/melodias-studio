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
  `DUMP_TUI_HTML_DIR=... cargo test draws_without_leaving_the_terminal`) plus
  six pool stills (`docs/assets/gallery-*.webp`, winners and losers from four
  runs, 960 px WebP). The dashboard keeps its own screenshots in
  `dashboard/README.md`.
- Copy: the stage list in the page mirrors `STAGES` in `src/pipeline.rs`; the
  targets in the commands table mirror the `Makefile`.
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
  sources are `docs/assets/demo-thumbnail.png`, `docs/assets/demo-audio.mp3`
  and `docs/assets/demo-video.mp4`; `scripts/prepare-site.sh` stages them into
  `site/assets/img/` and `site/assets/media/`. Current run:
  `output/epic-8-bit-orchestral-finale`, vibe "epic 8-bit orchestral finale"
  (total 69.25, the highest recent score for this vibe).
  Refresh them with:

  ```bash
  slug=epic-8-bit-orchestral-finale
  cp output/$slug/thumbnail.png docs/assets/demo-thumbnail.png
  ffmpeg -y -ss 45 -t 30 -i output/$slug/audio.mp3 \
    -af "afade=t=in:st=0:d=1,afade=t=out:st=29:d=1" \
    -b:a 96k docs/assets/demo-audio.mp3
  ffmpeg -y -ss 45 -t 30 -i output/$slug/video.mp4 \
    -vf scale=1280:-2 -c:v libx264 -crf 30 -preset slow \
    -c:a aac -b:a 96k -movflags +faststart docs/assets/demo-video.mp4
  ls -lh docs/assets/demo-*
  ```

  If the still is over 1.5 MB, convert it to WebP instead of copying it:
  `ffmpeg -y -i output/$slug/thumbnail.png -vf scale=1280:-2 -c:v libwebp
  -quality 85 docs/assets/demo-thumbnail.webp`, and point the page poster at
  the `.webp` file. Keep the still under 1 MB, the audio under 1 MB and the
  clip under 4 MB so the whole page stays under about 12 MB.
  Follow [docs/runbooks/site-demo.md](../docs/runbooks/site-demo.md)
  on the machine with the generated runs for the full checklist.
