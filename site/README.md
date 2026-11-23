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

- Screenshots: regenerate them in `docs/assets` (the TUI ones come from
  `DUMP_TUI_HTML_DIR=... cargo test draws_without_leaving_the_terminal`) and the
  page picks them up on the next stage.
- Copy: the stage list in the page mirrors `STAGES` in `src/pipeline.rs`; the
  targets in the commands table mirror the `Makefile`.
- Artwork: `banner.png` is the README banner. Keep the character credit in the
  footer while it shows Frieren.
- Demo media: the page ships with UI screenshots only; no generated track or
  video is committed. Follow [docs/runbooks/site-demo.md](../docs/runbooks/site-demo.md)
  on the machine with the generated runs to add a still, a 30 s clip and a
  30 s excerpt.
