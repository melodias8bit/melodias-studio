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

mkdir -p "$img_out" "$video_out"

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

copy "$overlays/birds.mp4" "$video_out/birds.mp4"

if [ "$status" -ne 0 ]; then
  printf 'site: staging failed\n' >&2
  exit 1
fi

printf 'site: staged 5 images and 1 overlay\n'
