# Project page demo media

The project page in `site/` ships with UI screenshots only: no generated track
or clip is committed. This runbook is for the machine that holds the recent
runs under `output/`, so the page can show one real example of the product.

## Agent prompt

Paste this into an agent with shell access on that machine, from the repository
root:

````text
You are on the machine that holds the recent melodias-studio runs under output/.

Task: add one real generated run to the static project page in site/ and open a
PR against main.

Start
  git fetch origin
  git switch site/landing-page    # or main once the page PR is merged

Read site/README.md and scripts/prepare-site.sh first.

1. Pick a run
   - ls -1 output
   - Prefer a run that met the targets (80 total, 85 technical, 70 editorial).
     melodias score output/<slug> prints them, and work/score.json holds the
     same numbers. If none met the target, take the highest total.
   - Check that output/<slug>/ has thumbnail.png, audio.mp3 and video.mp4.
   - Read the vibe with: jq -r '.vibe' output/<slug>/meta.json
     If jq is missing, use: python3 -c "import json;print(json.load(open('output/<slug>/meta.json'))['vibe'])"

2. Prepare three files under docs/assets (committed, they travel to CI)
   - Still, from the run thumbnail:
       cp output/<slug>/thumbnail.png docs/assets/demo-thumbnail.png
     If that file is over 1.5 MB, convert it instead:
       ffmpeg -y -i output/<slug>/thumbnail.png -vf scale=1280:-2 \
         -c:v libwebp -quality 85 docs/assets/demo-thumbnail.webp
   - 30 s of audio with short fades. Start at 45 s so the intro does not eat the
     clip; adjust -ss if the track peaks elsewhere:
       ffmpeg -y -ss 45 -t 30 -i output/<slug>/audio.mp3 \
         -af "afade=t=in:st=0:d=1,afade=t=out:st=29:d=1" \
         -b:a 96k docs/assets/demo-audio.mp3
   - 30 s of video with its audio, 720p and faststart for the web:
       ffmpeg -y -ss 45 -t 30 -i output/<slug>/video.mp4 \
         -vf scale=1280:-2 -c:v libx264 -crf 27 -preset slow \
         -c:a aac -b:a 96k -movflags +faststart docs/assets/demo-video.mp4
   - Target under 1 MB for the still, 1 MB for the audio and 4 MB for the video.
     ls -lh docs/assets/demo-* and retry with a lower bitrate or a shorter clip
     if one is over.

3. Stage and wire the page
   - Extend scripts/prepare-site.sh: copy demo-audio.mp3 and demo-video.mp4 into
     site/assets/media/ (create it with mkdir -p), and the still into
     site/assets/img/.
   - Add a "From a real run" block to site/index.html, after the gallery or
     under the hero. Include the still, an audio element and a video element:
       <audio controls preload="metadata" src="assets/media/demo-audio.mp3"></audio>
       <video controls playsinline preload="none"
              poster="assets/img/demo-thumbnail.png"
              src="assets/media/demo-video.mp4"></video>
     If you converted the still to WebP, point the poster at
     demo-thumbnail.webp instead. Caption it with the run slug and the vibe, for
     example "output/epic-8-bit-artcore, epic 8-bit artcore & orchestra".
   - Add only the CSS the block needs to site/assets/css/main.css. Keep the NES
     look: two columns that collapse to one under 760 px, hard borders, no
     external player library.
   - Update site/README.md with the three demo files and the ffmpeg recipe so
     the next person can refresh them.

4. Verify and ship
   - make site, then open http://127.0.0.1:4000/ at 1440 px and 390 px. The
     audio and the video must play, the page must not scroll horizontally, and
     the console must be clean.
   - git status must not list anything under site/assets/.
   - Run make check if cargo is available. The page itself does not need it.
   - Commit on a branch named site/demo-<slug>, push it, and open a PR against
     main. Put a screenshot of the new block and a link to the generated run in
     the PR description.

Constraints
  - Keep the code and copy in English.
  - Never commit site/assets/ or output/.
  - Do not touch the pipeline, src/ or the Makefile.
  - Keep the whole page under about 12 MB: hero, screenshots, overlay and demo.
````

## Done looks like

- The page shows one real run with a still, 30 seconds of audio and 30 seconds
  of video, and the caption names the run.
- `scripts/prepare-site.sh` stages the demo files, so `make site` and the Pages
  workflow behave the same way.
- `site/assets` stays out of git and the page still works at 390 px.
