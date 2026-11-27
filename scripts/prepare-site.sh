#!/usr/bin/env bash
# Stages the artwork the project page serves: the screenshots in docs/assets
# plus the birds overlay the renderer uses. The copies are gitignored so the
# page cannot drift from the current tree.
# Run through `make site` or by .github/workflows/pages.yml.
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
images="$root/docs/assets"
overlays="$root/video/public/overlays"
img_out="$root/site/assets/img"
video_out="$root/site/assets/video"
media_out="$root/site/assets/media"

mkdir -p "$img_out" "$video_out" "$media_out"

status=0

copy() {
  if [ ! -f "$1" ]; then
    printf 'site: missing %s\n' "${1#"$root"/}" >&2
    status=1
    return
  fi
  cp "$1" "$2"
}

for name in banner.png tui.png tui-review.png dashboard-home.png dashboard-detail.png; do
  copy "$images/$name" "$img_out/$name"
done

for name in gallery-battle-01.webp gallery-battle-02.webp gallery-battle-03.webp gallery-battle-04.webp gallery-battle-05.webp gallery-battle-06.webp gallery-finale-01.webp gallery-finale-02.webp gallery-finale-03.webp gallery-finale-04.webp gallery-finale-05.webp gallery-finale-06.webp gallery-artcore-01.webp gallery-violin-01.webp gallery-violin-02.webp gallery-violin-03.webp gallery-violin-04.webp gallery-kawaii-01.webp gallery-kawaii-02.webp gallery-kawaii-03.webp gallery-kawaii-04.webp gallery-kawaii-05.webp gallery-kawaii-06.webp gallery-kawaii-07.webp gallery-kawaii-08.webp gallery-kawaii-09.webp gallery-village-01.webp gallery-village-02.webp gallery-village-03.webp gallery-village-04.webp gallery-village-05.webp gallery-village-06.webp; do
  copy "$images/$name" "$img_out/$name"
done

copy "$overlays/birds.mp4" "$video_out/birds.mp4"

if [ -f "$images/demo-thumbnail.webp" ]; then
  copy "$images/demo-thumbnail.webp" "$img_out/demo-thumbnail.webp"
elif [ -f "$images/demo-thumbnail.png" ]; then
  copy "$images/demo-thumbnail.png" "$img_out/demo-thumbnail.png"
else
  printf 'site: missing %s\n' "docs/assets/demo-thumbnail.{webp,png}" >&2
  status=1
fi

copy "$images/demo-audio.mp3" "$media_out/demo-audio.mp3"
copy "$images/demo-video.mp4" "$media_out/demo-video.mp4"

if [ "$status" -ne 0 ]; then
  printf 'site: staging failed\n' >&2
  exit 1
fi

printf 'site: staged 5 images, 32 gallery stills and 1 overlay plus the demo still, audio and video\n'
